//! The HTTP and WebSocket API. docs/API.md describes it.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, DefaultBodyLimit, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use crate::auth::{self, LoginError, SignupError};
use crate::ban;
use crate::command;
use crate::hub::{Event, EventKind, Out, SessionId};
use crate::net::{self, Registration};
use crate::server::{Server, next_connection};
use crate::store::{IpBan, Role};
use crate::wire::product;
use crate::xsha1::Hash;

type Shared = State<Arc<Server>>;
type Reply = Result<Json<Value>, Failure>;

/// How long a client may take to send a request's headers, also between requests.
const HEADER_LIMIT: Duration = Duration::from_secs(15);
const REQUEST_LIMIT: Duration = Duration::from_secs(30);

pub async fn serve(server: Arc<Server>, listener: TcpListener) {
    let app = router(server.clone());
    let slots = Arc::new(Semaphore::new(server.config().http.max_connections.min(Semaphore::MAX_PERMITS)));
    loop {
        let (stream, peer) = next_connection(&listener).await;
        let Ok(slot) = slots.clone().try_acquire_owned() else {
            tracing::debug!(%peer, "API connection refused: too many");
            continue;
        };
        let app = TowerToHyperService::new(app.clone().layer(Extension(ConnectInfo(peer))));
        tokio::spawn(async move {
            let mut http = http1::Builder::new();
            http.timer(TokioTimer::new()).header_read_timeout(HEADER_LIMIT);
            if let Err(error) = http.serve_connection(TokioIo::new(stream), app).with_upgrades().await {
                tracing::debug!(%peer, %error, "API connection ended");
            }
            drop(slot);
        });
    }
}

pub fn router(server: Arc<Server>) -> Router {
    Router::new()
        .route("/ws", get(web_socket))
        .route("/api/health", get(health))
        .route("/api/status", get(status))
        .route("/api/channels", get(channels))
        .route("/api/games", get(games))
        .route("/api/accounts", post(create_account))
        .route("/api/password", post(change_password))
        .route("/api/admin/accounts/{name}", get(account).patch(update_account))
        .route("/api/admin/accounts/{name}/disconnect", post(disconnect))
        .route("/api/admin/bans", get(bans).post(add_ban).delete(remove_ban))
        .route("/api/admin/announce", post(announce))
        .route("/api/admin/reload", post(reload))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn(run_to_end))
        .with_state(server)
}

/// Finishes a request even if its client goes away, so that no database query is abandoned.
async fn run_to_end(request: axum::extract::Request, next: Next) -> Response {
    match tokio::time::timeout(REQUEST_LIMIT, tokio::spawn(next.run(request))).await {
        Err(_) => StatusCode::REQUEST_TIMEOUT.into_response(),
        Ok(Ok(response)) => response,
        Ok(Err(error)) => {
            tracing::error!(%error, "API request failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// An error's status and text. Anything unexpected is logged, not shown.
struct Failure(StatusCode, &'static str);

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        tracing::error!(%error, "API request failed");
        Failure(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    }
}

/// Where a request comes from.
struct Client(IpAddr);

impl FromRequestParts<Arc<Server>> for Client {
    type Rejection = Failure;

    async fn from_request_parts(parts: &mut Parts, server: &Arc<Server>) -> Result<Self, Failure> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|info| info.0.ip());
        let peer = peer.ok_or(Failure(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
        // The last entry is the one our own proxy added. Earlier ones are the client's word.
        let forwarded = parts.headers.get_all("x-forwarded-for").iter().next_back();
        let last = forwarded.and_then(|value| value.to_str().ok()?.rsplit(',').next()?.trim().parse().ok());
        let ip = last.filter(|_| server.config().http.behind_proxy).unwrap_or(peer);
        if server.hub.lock().is_banned(ip) {
            return Err(Failure(StatusCode::FORBIDDEN, "banned"));
        }
        Ok(Client(ip))
    }
}

/// Proof that a request may use the administration API.
struct Admin;

impl FromRequestParts<Arc<Server>> for Admin {
    type Rejection = Failure;

    async fn from_request_parts(parts: &mut Parts, server: &Arc<Server>) -> Result<Self, Failure> {
        let Client(ip) = Client::from_request_parts(parts, server).await?;
        let refused = Failure(StatusCode::UNAUTHORIZED, "administration token needed");
        let token = server.config().http.admin_token.clone();
        if token.is_empty() || server.hub.lock().too_many_failures(ip) {
            return Err(refused);
        }
        // Comparing digests takes the same time wherever they differ.
        let matches = bearer(&parts.headers)
            .is_some_and(|given| Sha256::digest(given.as_bytes()) == Sha256::digest(token.as_bytes()));
        if !matches {
            server.hub.lock().note_failure(ip);
            return Err(refused);
        }
        Ok(Admin)
    }
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

async fn health(State(server): Shared) -> Reply {
    match tokio::time::timeout(Duration::from_secs(5), server.store.check()).await {
        Ok(Ok(())) => Ok(Json(json!({ "healthy": true }))),
        _ => Err(Failure(StatusCode::SERVICE_UNAVAILABLE, "the database doesn't answer")),
    }
}

async fn status(State(server): Shared) -> Json<Value> {
    let hub = server.hub.lock();
    Json(json!({
        "name": server.config().server.name,
        "version": env!("CARGO_PKG_VERSION"),
        "users": hub.user_count(),
        "games": hub.game_count(),
        "uptime_secs": server.uptime().as_secs(),
    }))
}

async fn channels(State(server): Shared) -> Json<Value> {
    let channels = server.hub.lock().public_channels();
    Json(channels.iter().map(|(name, users)| json!({ "name": text(name), "users": users })).collect())
}

async fn games(State(server): Shared) -> Json<Value> {
    let games = server.hub.lock().public_games();
    let show = |(product, name, players): &(crate::wire::Tag, Vec<u8>, usize)| json!({ "product": product.to_string(), "name": text(name), "players": players });
    Json(games.iter().map(show).collect())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    name: String,
    password: String,
}

fn usable(password: &str) -> Result<Hash, Failure> {
    if password.is_empty() || password.len() > auth::MAX_PASSWORD {
        return Err(Failure(StatusCode::BAD_REQUEST, "passwords are 1 to 64 bytes"));
    }
    Ok(Hash::of_password(password))
}

async fn create_account(
    State(server): Shared,
    Client(ip): Client,
    Json(new): Json<Credentials>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let password = usable(&new.password)?;
    match auth::sign_up(&server, ip, new.name.as_bytes(), password).await? {
        Ok(()) => Ok((StatusCode::CREATED, Json(json!({ "name": new.name })))),
        Err(SignupError::InvalidName) => Err(Failure(StatusCode::BAD_REQUEST, "name not allowed")),
        Err(SignupError::NameTaken) => Err(Failure(StatusCode::CONFLICT, "name taken")),
        Err(SignupError::NotAllowed) => Err(Failure(StatusCode::FORBIDDEN, "new accounts aren't allowed")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewPassword {
    name: String,
    password: String,
    new_password: String,
}

async fn change_password(State(server): Shared, Client(ip): Client, Json(change): Json<NewPassword>) -> Reply {
    let new = usable(&change.new_password)?;
    match auth::check_password(&server, ip, &change.name, &change.password).await? {
        Ok(account) => {
            server.store.set_password(account.id, new).await?;
            Ok(Json(json!({ "changed": true })))
        }
        Err(_) => Err(Failure(StatusCode::UNAUTHORIZED, "wrong name or password")),
    }
}

async fn account(State(server): Shared, _: Admin, Path(name): Path<String>) -> Reply {
    let account = server.store.account(&name).await?.ok_or(Failure(StatusCode::NOT_FOUND, "no such account"))?;
    Ok(Json(json!({
        "id": account.id,
        "name": account.name,
        "role": account.role.name(),
        "locked": account.locked,
        "muted": account.muted,
        "last_login_at": account.last_login_at,
        "online": server.hub.lock().is_online(account.id),
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountUpdate {
    role: Option<String>,
    locked: Option<bool>,
    muted: Option<bool>,
    password: Option<String>,
}

async fn update_account(
    State(server): Shared,
    _: Admin,
    Path(name): Path<String>,
    Json(update): Json<AccountUpdate>,
) -> Reply {
    let account = server.store.account(&name).await?.ok_or(Failure(StatusCode::NOT_FOUND, "no such account"))?;
    let role = update.role.as_deref().map(Role::parse);
    if role == Some(None) {
        return Err(Failure(StatusCode::BAD_REQUEST, "a role is player, moderator or admin"));
    }
    let password = update.password.as_deref().map(usable).transpose()?;
    if let Some(role) = role.flatten() {
        server.store.set_role(account.id, role).await?;
        server.hub.lock().set_role(account.id, role);
    }
    if let Some(muted) = update.muted {
        server.store.set_muted(account.id, muted).await?;
        server.hub.lock().set_muted(account.id, muted);
    }
    if let Some(password) = password {
        server.store.set_password(account.id, password).await?;
    }
    if let Some(locked) = update.locked {
        server.store.set_locked(account.id, locked).await?;
        if locked {
            server.hub.lock().disconnect_account(account.id);
        }
    }
    tracing::info!(account = account.name, "changed through the API");
    self::account(State(server), Admin, Path(name)).await
}

async fn disconnect(State(server): Shared, _: Admin, Path(name): Path<String>) -> Reply {
    let account = server.store.account(&name).await?.ok_or(Failure(StatusCode::NOT_FOUND, "no such account"))?;
    let was_online = server.hub.lock().disconnect_account(account.id);
    Ok(Json(json!({ "disconnected": was_online })))
}

fn show_ban(ban: &IpBan) -> Value {
    json!({ "network": ban.range.to_string(), "reason": ban.reason, "by": ban.by, "until": ban.until })
}

async fn bans(State(server): Shared, _: Admin) -> Json<Value> {
    Json(server.hub.lock().bans().iter().map(show_ban).collect())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewBan {
    /// An address, a range or a player.
    target: String,
    #[serde(default)]
    hours: u32,
    #[serde(default)]
    reason: String,
}

async fn add_ban(State(server): Shared, _: Admin, Json(new): Json<NewBan>) -> Reply {
    match ban::add(&server, None, &new.target, new.hours, &new.reason).await? {
        Ok(range) => Ok(Json(json!({ "network": range.to_string() }))),
        Err(why) => Err(Failure(StatusCode::BAD_REQUEST, why)),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OldBan {
    network: String,
}

async fn remove_ban(State(server): Shared, _: Admin, Json(old): Json<OldBan>) -> Reply {
    let range = old.network.parse().map_err(|_| Failure(StatusCode::BAD_REQUEST, "not an address or range"))?;
    Ok(Json(json!({ "removed": ban::remove(&server, range).await? })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Announcement {
    text: String,
}

async fn announce(State(server): Shared, _: Admin, Json(announcement): Json<Announcement>) -> Json<Value> {
    server.hub.lock().broadcast(&Event::info(format!("Announcement: {}", announcement.text)));
    Json(json!({ "sent": true }))
}

async fn reload(State(server): Shared, _: Admin) -> Reply {
    match server.reload() {
        Ok(()) => Ok(Json(json!({ "reloaded": true }))),
        Err(error) => {
            tracing::error!(error = format!("{error:#}"), "settings not reloaded");
            Err(Failure(StatusCode::CONFLICT, "the settings are invalid; see the server's log"))
        }
    }
}

// WebSocket chat

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Login { name: String, password: String },
    Join { channel: String },
    Say { text: String },
}

const MAX_MESSAGE: usize = 4096;
const MAX_TEXT: usize = 255;
const MAX_CHANNEL: usize = 31;

async fn web_socket(State(server): Shared, Client(ip): Client, upgrade: WebSocketUpgrade) -> Response {
    upgrade.max_message_size(MAX_MESSAGE).max_frame_size(MAX_MESSAGE).on_upgrade(move |socket| async move {
        if let Err(error) = chat(server, ip, socket).await {
            tracing::debug!(%ip, %error, "web socket ended");
        }
    })
}

async fn chat(server: Arc<Server>, ip: IpAddr, mut socket: WebSocket) -> Result<()> {
    let (registration, mut outgoing) = Registration::new(&server, ip, product::WEB)?;
    let close = registration.outbox.close.clone();
    let mut session = WebSession { server, id: registration.id, ip, logged_in: false };
    let result = session.run(&mut socket, &mut outgoing, &close).await;
    drop(registration);
    // What was already queued may say why the connection is closing.
    while let Ok(Out::Event(event)) = outgoing.try_recv() {
        send(&mut socket, &show(&event)).await?;
    }
    result
}

async fn send(socket: &mut WebSocket, value: &Value) -> Result<()> {
    const LIMIT: Duration = Duration::from_secs(30);
    tokio::time::timeout(LIMIT, socket.send(Message::text(value.to_string()))).await??;
    Ok(())
}

struct WebSession {
    server: Arc<Server>,
    id: SessionId,
    ip: IpAddr,
    logged_in: bool,
}

impl WebSession {
    async fn run(
        &mut self,
        socket: &mut WebSocket,
        outgoing: &mut tokio::sync::mpsc::Receiver<Out>,
        close: &tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let settings = self.server.config();
        let deadline = tokio::time::sleep(Duration::from_secs(settings.server.login_timeout_secs));
        tokio::pin!(deadline);
        let interval = Duration::from_secs(settings.server.ping_interval_secs.max(1));
        let mut pings = tokio::time::interval_at(tokio::time::Instant::now() + interval, interval);
        let mut silent_intervals = 0;
        loop {
            tokio::select! {
                biased;
                () = close.cancelled() => return Ok(()),
                out = outgoing.recv() => match out {
                    Some(Out::Event(event)) if event.kind == EventKind::Entered => {
                        send(socket, &json!({ "type": "entered", "channel": text(&event.text) })).await?;
                    }
                    Some(Out::Event(event)) => send(socket, &show(&event)).await?,
                    Some(Out::Raw(_)) => {}
                    None => return Ok(()),
                },
                message = socket.recv() => {
                    silent_intervals = 0;
                    match message {
                        Some(Ok(Message::Text(message))) => {
                            if let Some(reply) = self.handle(&message).await? {
                                send(socket, &reply).await?;
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => return Ok(()),
                        Some(Ok(_)) => {}
                        Some(Err(error)) => return Err(error.into()),
                    }
                }
                () = &mut deadline, if !self.logged_in => anyhow::bail!("did not log in in time"),
                _ = pings.tick() => {
                    silent_intervals += 1;
                    anyhow::ensure!(silent_intervals < 4, "stopped responding");
                    socket.send(Message::Ping(Vec::new().into())).await?;
                }
            }
        }
    }

    async fn handle(&mut self, message: &str) -> Result<Option<Value>> {
        let problem = |code: &str| Ok(Some(json!({ "type": "problem", "code": code })));
        let Ok(request) = serde_json::from_str::<Request>(message) else { return problem("bad_request") };
        match request {
            Request::Login { .. } if self.logged_in => problem("already_logged_in"),
            Request::Login { name, password } => {
                match auth::check_password(&self.server, self.ip, &name, &password).await? {
                    Err(LoginError::Locked) => problem("locked"),
                    Err(_) => problem("wrong_name_or_password"),
                    Ok(account) => {
                        let name = account.name.clone();
                        if !auth::enter(&self.server, self.id, self.ip, account, product::WEB).await? {
                            return Ok(None);
                        }
                        self.logged_in = true;
                        Ok(Some(json!({ "type": "logged_in", "name": name })))
                    }
                }
            }
            _ if !self.logged_in => problem("not_logged_in"),
            Request::Join { channel } if channel.is_empty() || channel.len() > MAX_CHANNEL => {
                problem("bad_channel")
            }
            Request::Join { channel } => {
                self.server.hub.lock().join_channel(self.id, channel.as_bytes());
                Ok(None)
            }
            Request::Say { text } if text.is_empty() || text.len() > MAX_TEXT => problem("bad_text"),
            Request::Say { text } => {
                command::say(&self.server, self.id, &net::one_line(text.as_bytes())).await?;
                Ok(None)
            }
        }
    }
}

fn show(event: &Event) -> Value {
    let kind = match event.kind {
        EventKind::ShowUser => "user",
        EventKind::Join => "join",
        EventKind::Leave => "leave",
        EventKind::Whisper => "whisper",
        EventKind::Talk => "talk",
        EventKind::Broadcast => "broadcast",
        EventKind::Channel => "channel",
        EventKind::UserFlags => "flags",
        EventKind::WhisperSent => "whisper_sent",
        EventKind::Info => "info",
        EventKind::Error => "error",
        EventKind::Emote => "emote",
        EventKind::Entered => "entered",
    };
    let mut value = json!({ "type": kind });
    match event.kind {
        EventKind::Info | EventKind::Error | EventKind::Broadcast => value["text"] = text(&event.text).into(),
        EventKind::Channel | EventKind::Entered => value["channel"] = text(&event.text).into(),
        EventKind::ShowUser | EventKind::Join | EventKind::UserFlags | EventKind::Leave => {
            value["name"] = event.name.clone().into();
            value["flags"] = event.flags.into();
            value["product"] = event.product.to_string().into();
        }
        _ => {
            value["name"] = event.name.clone().into();
            value["text"] = text(&event.text).into();
        }
    }
    value
}
