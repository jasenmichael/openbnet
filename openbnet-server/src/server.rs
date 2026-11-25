//! Accepts connections and ties the parts together.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use parking_lot::{Mutex, RwLock};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpSocket, TcpStream, UdpSocket};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use tokio::task::JoinSet;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::files::{self, Files};
use crate::hub::{Event, Hub};
use crate::store::{FinishedGame, Store};
use crate::text::{self, Style};
use crate::versions::Versions;
use crate::{bncs, http, irc, udp};

pub struct Server {
    config: RwLock<Arc<Config>>,
    /// The data folder, to read the settings again.
    data: Option<PathBuf>,
    pub store: Store,
    pub hub: Mutex<Hub>,
    pub files: Files,
    pub versions: Versions,
    pub udp: UdpSocket,
    /// Set when the server should stop.
    pub stop: CancellationToken,
    started: Instant,
}

/// The first byte a client sends says which protocol it speaks.
mod protocol {
    pub const GAME: u8 = 0x01;
    pub const FILES: u8 = 0x02;
    pub const BOT: u8 = 0x03;
    /// Someone at a terminal pressing return.
    pub const TELNET: u8 = 0x0d;
}

impl Server {
    pub fn config(&self) -> Arc<Config> {
        self.config.read().clone()
    }

    /// Reads the settings again. Ports, the database and the files directory need a restart.
    pub fn reload(&self) -> Result<()> {
        let path = self.data.as_ref().context("the server was started without a data folder")?;
        self.reconfigure(Config::load(path)?);
        Ok(())
    }

    pub fn reconfigure(&self, config: Config) {
        self.hub.lock().reconfigure(&config);
        *self.config.write() = Arc::new(config);
        tracing::info!("settings reloaded");
    }

    /// Warns everyone, then stops.
    pub async fn stop_gently(&self) {
        let secs = self.config().server.shutdown_secs;
        tracing::info!(secs, "stopping");
        if secs > 0 && self.hub.lock().user_count() > 0 {
            self.hub.lock().broadcast(&Event::info(format!("The server is shutting down in {secs} seconds.")));
            tokio::time::sleep(Duration::from_secs(secs)).await;
        }
        self.stop.cancel();
    }

    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    /// Milliseconds since the server started, for measuring latency.
    pub fn ticks(&self) -> u32 {
        self.started.elapsed().as_millis() as u32
    }
}

pub struct Running {
    pub server: Arc<Server>,
    pub address: SocketAddr,
    listener: TcpListener,
    pub irc: Option<TcpListener>,
    pub irc_tls: Option<(TcpListener, TlsAcceptor)>,
    pub http: Option<TcpListener>,
    finished: mpsc::UnboundedReceiver<FinishedGame>,
}

/// Opens the database and the ports.
pub async fn start(config: Config, data: Option<PathBuf>) -> Result<Running> {
    let store = Store::open(&config.database.url).await.context("opening the database")?;
    let listener =
        listen(config.server.listen).with_context(|| format!("listening on {}", config.server.listen))?;
    let address = listener.local_addr()?;
    let udp = UdpSocket::bind(address).await.with_context(|| format!("listening on UDP {address}"))?;
    let (sender, finished) = mpsc::unbounded_channel();
    let server = Arc::new(Server {
        hub: Mutex::new(Hub::new(&config, sender)),
        files: Files::new(&config.files),
        versions: Versions::load()?,
        store,
        udp,
        stop: CancellationToken::new(),
        started: Instant::now(),
        config: RwLock::new(Arc::new(config)),
        data,
    });
    for ban in server.store.ip_bans().await? {
        server.hub.lock().add_ban(ban);
    }
    for (channel, topic) in server.store.topics().await? {
        server.hub.lock().restore_topic(&channel, &topic);
    }
    let irc = listen_if(server.config().irc.listen)?;
    let irc_tls = listen_tls(&server.config().irc)?;
    let http = listen_if(server.config().http.listen)?;
    Ok(Running { server, address, listener, irc, irc_tls, http, finished })
}

/// The usual queue of 128 overflows when many players reconnect at once, as after a restart.
fn listen(address: SocketAddr) -> std::io::Result<TcpListener> {
    let socket = if address.is_ipv4() { TcpSocket::new_v4()? } else { TcpSocket::new_v6()? };
    socket.set_reuseaddr(true)?;
    socket.bind(address)?;
    socket.listen(4096)
}

fn listen_if(address: Option<SocketAddr>) -> Result<Option<TcpListener>> {
    address.map(|address| listen(address).with_context(|| format!("listening on {address}"))).transpose()
}

fn listen_tls(config: &crate::config::Irc) -> Result<Option<(TcpListener, TlsAcceptor)>> {
    let Some(address) = config.tls_listen else { return Ok(None) };
    let certificate = config.certificate.as_ref().context("irc.tls_listen needs irc.certificate")?;
    let key = config.key.as_ref().context("irc.tls_listen needs irc.key")?;
    let certificates = CertificateDer::pem_file_iter(certificate)
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .with_context(|| format!("reading {}", certificate.display()))?;
    let key = PrivateKeyDer::from_pem_file(key).with_context(|| format!("reading {}", key.display()))?;
    let tls = ServerConfig::builder().with_no_client_auth().with_single_cert(certificates, key)?;
    let listener = listen(address).with_context(|| format!("listening on {address}"))?;
    Ok(Some((listener, TlsAcceptor::from(Arc::new(tls)))))
}

impl Running {
    /// Serves until told to stop, then finishes what players and games left to be stored.
    pub async fn run(self) -> Result<()> {
        let Running { server, listener, irc, irc_tls, http, finished, .. } = self;
        // One limit for every game and chat protocol, as each connection costs about the same.
        let gate = Arc::new(Gate::new(server.config().server.max_connections));
        let recording = tokio::spawn(record_games(server.clone(), finished));
        let mut serving = JoinSet::new();
        serving.spawn(udp::serve(server.clone()));
        {
            let shared = server.clone();
            serving.spawn(accept(server.clone(), listener, gate.clone(), move |stream, ip| {
                game_port(shared.clone(), stream, ip)
            }));
        }
        if let Some(listener) = irc {
            let shared = server.clone();
            serving.spawn(accept(server.clone(), listener, gate.clone(), move |stream, ip| {
                irc::serve(shared.clone(), stream, ip)
            }));
        }
        if let Some((listener, tls)) = irc_tls {
            let shared = server.clone();
            serving.spawn(accept(server.clone(), listener, gate.clone(), move |stream, ip| {
                let (server, tls) = (shared.clone(), tls.clone());
                async move {
                    let stream = tokio::time::timeout(Duration::from_secs(30), tls.accept(stream)).await??;
                    irc::serve(server, stream, ip).await
                }
            }));
        }
        if let Some(listener) = http {
            serving.spawn(http::serve(server.clone(), listener));
        }
        let result = tokio::select! {
            () = server.stop.cancelled() => Ok(()),
            ended = serving.join_next() => Err(anyhow::anyhow!("a part of the server stopped: {ended:?}")),
        };
        serving.shutdown().await;
        server.stop.cancel();
        server.hub.lock().close();
        // Let requests being handled finish, and results be stored, but not for ever.
        const PATIENCE: Duration = Duration::from_secs(5);
        let _ = tokio::time::timeout(PATIENCE, gate.empty()).await;
        let _ = tokio::time::timeout(PATIENCE, recording).await;
        let _ = tokio::time::timeout(PATIENCE, server.store.close()).await;
        result
    }
}

/// Counts connections, in all and by address.
struct Gate {
    max: usize,
    slots: Arc<Semaphore>,
    by_address: Arc<Mutex<HashMap<IpAddr, usize>>>,
}

struct Pass {
    ip: IpAddr,
    by_address: Arc<Mutex<HashMap<IpAddr, usize>>>,
    _slot: OwnedSemaphorePermit,
}

impl Gate {
    fn new(max: usize) -> Gate {
        let max = max.min(Semaphore::MAX_PERMITS);
        Gate { max, slots: Arc::new(Semaphore::new(max)), by_address: Arc::default() }
    }

    fn enter(&self, ip: IpAddr, max_per_address: usize) -> Option<Pass> {
        let slot = self.slots.clone().try_acquire_owned().ok()?;
        let mut by_address = self.by_address.lock();
        let count = by_address.entry(ip).or_default();
        if max_per_address > 0 && *count >= max_per_address {
            return None;
        }
        *count += 1;
        Some(Pass { ip, by_address: self.by_address.clone(), _slot: slot })
    }

    async fn empty(&self) {
        let all = u32::try_from(self.max).unwrap_or(u32::MAX);
        let _ = self.slots.acquire_many(all).await;
    }
}

impl Drop for Pass {
    fn drop(&mut self) {
        let mut by_address = self.by_address.lock();
        if let Some(count) = by_address.get_mut(&self.ip) {
            *count -= 1;
            if *count == 0 {
                by_address.remove(&self.ip);
            }
        }
    }
}

/// The next connection, however long that takes.
pub async fn next_connection(listener: &TcpListener) -> (TcpStream, SocketAddr) {
    loop {
        match listener.accept().await {
            Ok((stream, peer)) if stream.set_nodelay(true).is_ok() => return (stream, peer),
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "could not accept a connection");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn accept<F, Fut>(server: Arc<Server>, listener: TcpListener, gate: Arc<Gate>, serve: F)
where
    F: Fn(TcpStream, IpAddr) -> Fut,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    loop {
        let (stream, peer) = next_connection(&listener).await;
        let ip = peer.ip().to_canonical();
        if server.hub.lock().is_banned(ip) {
            continue;
        }
        let Some(pass) = gate.enter(ip, server.config().server.max_connections_per_ip) else {
            tracing::debug!(%peer, "refused: too many connections");
            continue;
        };
        let serving = serve(stream, ip);
        tokio::spawn(async move {
            if let Err(error) = serving.await {
                tracing::debug!(%peer, %error, "connection ended");
            }
            drop(pass);
        });
    }
}

async fn game_port(server: Arc<Server>, mut stream: TcpStream, ip: IpAddr) -> Result<()> {
    let config = server.config();
    let limit = Duration::from_secs(config.server.login_timeout_secs);
    let first = tokio::select! {
        first = tokio::time::timeout(limit, stream.read_u8()) => first??,
        () = server.stop.cancelled() => return Ok(()),
    };
    match (first, ip) {
        // Game hosts are sent to players as IPv4, so that is all games may use.
        (protocol::GAME, IpAddr::V4(ip)) => bncs::serve(server, stream, ip).await,
        (protocol::FILES, _) => tokio::time::timeout(limit, files::serve(&server.files, &mut stream)).await?,
        (protocol::BOT, _) if config.bots.enabled => text::serve(server, stream, ip, Style::Bot).await,
        (protocol::TELNET, _) if config.bots.enabled => text::serve(server, stream, ip, Style::Telnet).await,
        (other, _) => anyhow::bail!("protocol {other:#04x} not available"),
    }
}

async fn record_games(server: Arc<Server>, mut finished: mpsc::UnboundedReceiver<FinishedGame>) {
    while let Some(game) = finished.recv().await {
        if let Err(error) = record_game(&server, &game).await {
            tracing::error!(%error, name = game.name, "could not store a game's result");
        }
    }
}

async fn record_game(server: &Server, game: &FinishedGame) -> Result<()> {
    server.store.finish_game(game).await?;
    for (account, _) in &game.results {
        let records = server.store.records(*account, game.product).await?;
        server.hub.lock().set_records(*account, game.product, records);
    }
    Ok(())
}
