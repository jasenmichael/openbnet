//! The binary protocol the games speak (Battle.net Chat Server).
//!
//! Behavior follows PvPGN's `bnetd/handle_bnet.cpp`, except where the decision records say otherwise.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::Result;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedReadHalf;

use crate::auth::{self, LoginError, MAX_NAME, SignupError};
use crate::command;
use crate::config::Ad;
use crate::files;
use crate::hub::{
    Event, EventKind, GameOffer, GameStatus, IRONMAN_GAME, LADDER_GAME, Listing, Lookup, Out, Outbox, SessionId,
};
use crate::net::Link;
use crate::server::Server;
use crate::store::{Account, Friend, LadderOrder, Outcome, Record};
use crate::udp;
use crate::versions::Verdict;
use crate::wire::{Reader, Tag, Writer, product};
use crate::xsha1::Hash;

mod id {
    pub const NULL: u8 = 0x00;
    pub const CLOSE_GAME: u8 = 0x02;
    pub const CLIENT_ID: u8 = 0x05;
    pub const START_VERSIONING: u8 = 0x06;
    pub const REPORT_VERSION: u8 = 0x07;
    pub const START_GAME_1: u8 = 0x08;
    pub const GAME_LIST: u8 = 0x09;
    pub const ENTER_CHAT: u8 = 0x0a;
    pub const CHANNEL_LIST: u8 = 0x0b;
    pub const JOIN_CHANNEL: u8 = 0x0c;
    pub const CHAT: u8 = 0x0e;
    pub const CHAT_EVENT: u8 = 0x0f;
    pub const LEAVE_CHAT: u8 = 0x10;
    pub const UDP_WORKS: u8 = 0x14;
    pub const AD: u8 = 0x15;
    pub const START_GAME_3: u8 = 0x1a;
    pub const GAME_ADDRESS: u8 = 0x1b;
    pub const START_GAME_4: u8 = 0x1c;
    pub const SERVER_TOKEN_2: u8 = 0x1d;
    pub const CLIENT_ID_2: u8 = 0x1e;
    pub const LEAVE_GAME: u8 = 0x1f;
    pub const JOIN_GAME: u8 = 0x22;
    pub const PING: u8 = 0x25;
    pub const READ_USER_DATA: u8 = 0x26;
    pub const WRITE_USER_DATA: u8 = 0x27;
    pub const SERVER_TOKEN: u8 = 0x28;
    pub const LOG_IN: u8 = 0x29;
    pub const CREATE_ACCOUNT: u8 = 0x2a;
    pub const GAME_RESULT: u8 = 0x2c;
    pub const ICON_FILE: u8 = 0x2d;
    pub const LADDER: u8 = 0x2e;
    pub const LADDER_SEARCH: u8 = 0x2f;
    pub const CD_KEY: u8 = 0x30;
    pub const CHANGE_PASSWORD: u8 = 0x31;
    pub const CHECK_MAP: u8 = 0x32;
    pub const FILE_TIME: u8 = 0x33;
    pub const REALMS: u8 = 0x34;
    pub const CD_KEY_2: u8 = 0x36;
    pub const LOG_IN_2: u8 = 0x3a;
    pub const CHECK_MAP_2: u8 = 0x3c;
    pub const CREATE_ACCOUNT_2: u8 = 0x3d;
    pub const REALMS_2: u8 = 0x40;
    pub const AD_LINK: u8 = 0x41;
    pub const CD_KEY_3: u8 = 0x42;
    pub const GAME_PORT: u8 = 0x45;
    pub const AUTH_INFO: u8 = 0x50;
    pub const AUTH_CHECK: u8 = 0x51;
    pub const FRIENDS: u8 = 0x65;
    pub const FRIEND: u8 = 0x66;
    pub const FRIEND_ADDED: u8 = 0x67;
    pub const FRIEND_REMOVED: u8 = 0x68;
}

/// The largest packet either side may send.
const MAX_PACKET: usize = 3072;
const MAX_GAME_NAME: usize = 32;
const MAX_GAME_INFO: usize = 128;
const MAX_CHAT: usize = 255;

/// Products this server can serve so far. Chat clients use other protocols.
pub const SUPPORTED: &[Tag] = &[
    product::WARCRAFT2,
    product::STARCRAFT,
    product::BROOD_WAR,
    product::STARCRAFT_SHAREWARE,
    product::STARCRAFT_JAPAN,
    product::DIABLO,
    product::DIABLO_SHAREWARE,
    product::DIABLO2,
    product::DIABLO2_EXPANSION,
];

pub async fn serve(server: Arc<Server>, stream: TcpStream, ip: Ipv4Addr) -> Result<()> {
    let (reader, writer) = stream.into_split();
    let link = Link::open(&server, ip.into(), Tag::default(), writer, Box::new(chat_event))?;
    udp::send_test(&server, SocketAddrV4::new(ip, 6112)).await;
    let mut connection = Connection {
        server: server.clone(),
        id: link.id,
        ip,
        outbox: link.outbox.clone(),
        stage: Stage::Connected,
        version_accepted: false,
    };
    let reading = async move { connection.read(reader).await };
    link.run(&server, reading).await
}

fn packet(id: u8, body: Writer) -> Vec<u8> {
    let body = body.finish();
    let mut data = Vec::with_capacity(body.len() + 4);
    data.extend_from_slice(&[0xff, id]);
    data.extend_from_slice(&((body.len() + 4) as u16).to_le_bytes());
    data.extend_from_slice(&body);
    data
}

fn chat_event(event: &Event) -> Vec<u8> {
    if event.kind == EventKind::Entered {
        return Vec::new();
    }
    let mut text = event.text.as_slice();
    let is_message =
        !matches!(event.kind, EventKind::ShowUser | EventKind::Join | EventKind::Leave | EventKind::UserFlags);
    if is_message {
        // Clients mishandle empty messages.
        text = if text.is_empty() { b" " } else { &text[..text.len().min(MAX_CHAT)] };
    }
    packet(
        id::CHAT_EVENT,
        Writer::new()
            .u32(event.kind as u32)
            .u32(event.flags)
            .u32(event.latency)
            .u32(0)
            .u32(0xbaad_f00d)
            .u32(0xbaad_f00d)
            .cstr(&event.name)
            .cstr(text),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Connected,
    LoggedIn,
    /// Failed the version check. Nothing more is answered.
    Rejected,
}

struct Connection {
    server: Arc<Server>,
    id: SessionId,
    ip: Ipv4Addr,
    outbox: Outbox,
    stage: Stage,
    version_accepted: bool,
}

impl Connection {
    async fn read(&mut self, mut reader: OwnedReadHalf) -> Result<()> {
        let config = self.server.config();
        let settings = &config.server;
        let login_limit = Duration::from_secs(settings.login_timeout_secs);
        let ping_interval = Duration::from_secs(settings.ping_interval_secs.max(1));
        let deadline = tokio::time::sleep(login_limit);
        tokio::pin!(deadline);
        let mut pings = tokio::time::interval_at(tokio::time::Instant::now() + ping_interval, ping_interval);
        let mut buffer = Vec::with_capacity(MAX_PACKET);
        let mut silent_intervals = 0;
        loop {
            tokio::select! {
                () = self.outbox.close.cancelled() => return Ok(()),
                read = reader.read_buf(&mut buffer) => {
                    if read? == 0 {
                        return Ok(());
                    }
                    silent_intervals = 0;
                }
                () = &mut deadline, if self.stage != Stage::LoggedIn => {
                    // Password changes and sign-ups take a while, but not for ever.
                    anyhow::bail!("did not log in in time");
                }
                _ = pings.tick() => {
                    silent_intervals += 1;
                    anyhow::ensure!(silent_intervals < 4, "stopped responding");
                    self.ping();
                }
            }
            let mut used = 0;
            while let Some(header) = buffer[used..].first_chunk::<4>() {
                anyhow::ensure!(header[0] == 0xff, "not a game packet");
                let size = usize::from(u16::from_le_bytes([header[2], header[3]]));
                anyhow::ensure!((4..=MAX_PACKET).contains(&size), "bad packet size {size}");
                let Some(data) = buffer.get(used..used + size) else { break };
                let (id, body) = (data[1], data[4..].to_vec());
                used += size;
                if let Err(error) = self.handle(id, &body).await {
                    // Games send some packets in forms PvPGN also ignores, so this isn't fatal.
                    tracing::debug!(session = self.id, id, %error, "packet ignored");
                }
                if self.outbox.close.is_cancelled() {
                    return Ok(());
                }
            }
            buffer.drain(..used);
        }
    }

    fn send(&self, id: u8, body: Writer) {
        self.outbox.send(Out::Raw(packet(id, body)));
    }

    fn ping(&self) {
        self.send(id::PING, Writer::new().u32(self.server.ticks()));
    }

    async fn handle(&mut self, id: u8, body: &[u8]) -> Result<()> {
        let mut r = Reader::new(body);
        match self.stage {
            Stage::Rejected => Ok(()),
            Stage::Connected => self.handle_before_login(id, &mut r).await,
            Stage::LoggedIn => self.handle_after_login(id, &mut r).await,
        }
    }

    async fn handle_before_login(&mut self, id: u8, r: &mut Reader<'_>) -> Result<()> {
        match id {
            id::CLIENT_ID | id::CLIENT_ID_2 => self.client_id(id),
            id::START_VERSIONING => self.start_versioning(r).await?,
            id::REPORT_VERSION => self.report_version(r)?,
            id::AUTH_INFO => self.auth_info(r).await?,
            id::AUTH_CHECK => self.auth_check(r)?,
            id::CREATE_ACCOUNT | id::CREATE_ACCOUNT_2 => self.create_account(id, r).await?,
            id::CHANGE_PASSWORD => self.change_password(r).await?,
            id::LOG_IN | id::LOG_IN_2 => self.log_in(id, r).await?,
            id::CD_KEY => {
                r.skip(4)?;
                r.cstr(128)?;
                let owner = r.cstr(128)?;
                self.send(id::CD_KEY, Writer::new().u32(1).cstr(owner));
            }
            id::CD_KEY_2 => {
                r.skip(44)?;
                let owner = r.cstr(128)?;
                self.send(id::CD_KEY_2, Writer::new().u32(1).cstr(owner));
            }
            id::CD_KEY_3 => {
                r.skip(48)?;
                r.cstr(128)?;
                self.send(id::CD_KEY_3, Writer::new().u32(0).cstr(""));
            }
            _ => self.handle_any_time(id, r).await?,
        }
        Ok(())
    }

    async fn handle_any_time(&mut self, id: u8, r: &mut Reader<'_>) -> Result<()> {
        match id {
            id::NULL => self.send(id::NULL, Writer::new()),
            id::PING => {
                let then = r.u32()?;
                let now = self.server.ticks();
                if now >= then && then != 0 {
                    self.server.hub.lock().set_latency(self.id, now - then);
                }
            }
            id::UDP_WORKS => self.server.hub.lock().set_udp_works(self.id),
            id::ICON_FILE => {
                let name = "icons.bni";
                let modified = self.server.files.modified(name).await;
                self.send(id::ICON_FILE, Writer::new().u64(modified).cstr(name));
            }
            id::FILE_TIME => {
                let (kind, unknown, name) = (r.u32()?, r.u32()?, r.ascii(128)?);
                let modified = self.server.files.modified(name).await;
                self.send(id::FILE_TIME, Writer::new().u32(kind).u32(unknown).u64(modified).cstr(name));
            }
            id::READ_USER_DATA => self.read_user_data(r).await?,
            id::GAME_ADDRESS => {
                r.skip(2)?;
                let (port, ip) = (u16::from_be_bytes(r.bytes()?), Ipv4Addr::from(r.bytes::<4>()?));
                // The address is the client's word. Only its own connection's address is believed.
                if port >= 1024 && ip == self.ip {
                    self.with_session(|session| session.game_address = SocketAddrV4::new(ip, port));
                }
            }
            _ => tracing::trace!(session = self.id, id, "packet not handled"),
        }
        Ok(())
    }

    async fn handle_after_login(&mut self, id: u8, r: &mut Reader<'_>) -> Result<()> {
        match id {
            id::ENTER_CHAT => self.enter_chat(r)?,
            id::CHANNEL_LIST => {
                let list = self.server.hub.lock().channel_list(self.id);
                let body = list.iter().fold(Writer::new(), Writer::cstr);
                self.send(id::CHANNEL_LIST, body.cstr(""));
            }
            id::JOIN_CHANNEL => {
                r.skip(4)?;
                let name = r.cstr(31)?;
                if !name.is_empty() {
                    self.server.hub.lock().join_channel(self.id, name);
                }
            }
            id::CHAT => command::say(&self.server, self.id, r.cstr(MAX_CHAT)?).await?,
            id::LEAVE_CHAT => self.server.hub.lock().leave_channel(self.id),
            id::GAME_LIST => self.game_list(r)?,
            id::START_GAME_1 | id::START_GAME_3 | id::START_GAME_4 => self.start_game(id, r)?,
            id::CLOSE_GAME | id::LEAVE_GAME => self.server.hub.lock().leave_game(self.id),
            id::JOIN_GAME => {
                r.skip(8)?;
                let (name, password) = (r.cstr(MAX_GAME_NAME)?, r.cstr(MAX_GAME_NAME)?);
                self.server.hub.lock().join_game(self.id, name, password);
            }
            id::GAME_RESULT => self.game_result(r)?,
            id::CHECK_MAP | id::CHECK_MAP_2 => {
                let ladder = self.server.hub.lock().start_checked_game(self.id);
                let answer = match ladder {
                    None => 0,
                    Some(false) => 1,
                    Some(true) => 2,
                };
                self.send(id, Writer::new().u32(answer));
            }
            id::WRITE_USER_DATA => self.write_user_data(r).await?,
            id::GAME_PORT => {
                let port = r.u16()?;
                if port >= 1024 {
                    self.with_session(|session| session.game_address.set_port(port));
                }
            }
            id::LADDER => self.ladder(r).await?,
            id::LADDER_SEARCH => self.ladder_search(r).await?,
            // No closed realms yet.
            id::REALMS | id::REALMS_2 => self.send(id, Writer::new().u32(0).u32(0)),
            id::AD => self.ad(r).await?,
            id::AD_LINK => {
                let number = r.u32()?;
                let config = self.server.config();
                if let Some(ad) = (number as usize).checked_sub(1).and_then(|index| config.ads.get(index)) {
                    self.send(id::AD_LINK, Writer::new().u32(number).cstr(&ad.url));
                }
            }
            id::FRIENDS => self.friends().await?,
            id::FRIEND => self.friend(r).await?,
            _ => self.handle_any_time(id, r).await?,
        }
        Ok(())
    }

    fn with_session<T>(&self, change: impl FnOnce(&mut crate::hub::Session) -> T) -> Option<T> {
        self.server.hub.lock().session_mut(self.id).map(change)
    }

    // Before login

    fn client_id(&self, id: u8) {
        let reply = Writer::new().u32(1).u32(0xaa88_43d1).u32(0x001b_9dda).u32(0xab69_f79a);
        self.send(id::CLIENT_ID, reply);
        let Some(token) = self.with_session(|session| session.token) else { return };
        if id == id::CLIENT_ID_2 {
            self.send(id::SERVER_TOKEN_2, Writer::new().u32(self.id).u32(token));
        } else {
            self.send(id::SERVER_TOKEN, Writer::new().u32(token));
        }
    }

    fn set_game(&self, platform: Tag, game: Tag, version_byte: u32) -> bool {
        let config = self.server.config();
        let allowed = &config.server.products;
        if !SUPPORTED.contains(&game) || !(allowed.is_empty() || allowed.contains(&game.to_string())) {
            tracing::info!(session = self.id, product = %game, "product not allowed");
            self.outbox.close.cancel();
            return false;
        }
        self.with_session(|session| {
            session.platform = platform;
            session.product = game;
            session.version_byte = version_byte;
        });
        true
    }

    async fn start_versioning(&self, r: &mut Reader<'_>) -> Result<()> {
        let (platform, game, version) = (Tag(r.u32()?), Tag(r.u32()?), r.u32()?);
        if self.set_game(platform, game, version) {
            let (file, equation) = self.server.versions.check(game, platform, version);
            let modified = self.server.files.modified(file).await;
            self.send(id::START_VERSIONING, Writer::new().u64(modified).cstr(file).cstr(equation));
        }
        Ok(())
    }

    fn report_version(&mut self, r: &mut Reader<'_>) -> Result<()> {
        let claimed = (Tag(r.u32()?), Tag(r.u32()?), r.u32()?);
        let (version, checksum) = (r.u32()?, r.u32()?);
        let same = self.with_session(|s| (s.platform, s.product, s.version_byte)) == Some(claimed);
        let accepted = same && self.judge_version(version, checksum);
        self.send(id::REPORT_VERSION, Writer::new().u32(if accepted { 2 } else { 0 }).cstr("").cstr(""));
        Ok(())
    }

    async fn auth_info(&self, r: &mut Reader<'_>) -> Result<()> {
        r.skip(4)?;
        let (platform, game, version) = (Tag(r.u32()?), Tag(r.u32()?), r.u32()?);
        r.skip(8)?;
        let bias = r.u32()? as i32;
        if !self.set_game(platform, game, version) {
            return Ok(());
        }
        let Some(token) = self.with_session(|session| {
            session.time_bias_minutes = bias;
            session.token
        }) else {
            return Ok(());
        };
        self.ping();
        let (file, equation) = self.server.versions.check(game, platform, version);
        let modified = self.server.files.modified(file).await;
        let body = Writer::new().u32(0).u32(token).u32(self.id).u64(modified).cstr(file).cstr(equation);
        self.send(id::AUTH_INFO, body);
        Ok(())
    }

    fn auth_check(&mut self, r: &mut Reader<'_>) -> Result<()> {
        r.skip(4)?;
        let (version, checksum) = (r.u32()?, r.u32()?);
        let accepted = self.judge_version(version, checksum);
        self.send(id::AUTH_CHECK, Writer::new().u32(if accepted { 0 } else { 0x101 }).cstr(""));
        Ok(())
    }

    fn judge_version(&mut self, version: u32, checksum: u32) -> bool {
        let Some(key) = self.with_session(|s| (s.product, s.platform, s.version_byte)) else { return false };
        let verdict = self.server.versions.verdict(key, version, checksum);
        let strict = self.server.config().server.strict_versions;
        let (tag, accepted) = match verdict {
            Verdict::Known(tag) => (Some(tag.to_owned()), true),
            Verdict::Modified(tag) => (Some(tag.to_owned()), !strict),
            Verdict::Unknown => (None, !strict),
        };
        tracing::debug!(session = self.id, ?verdict, version, checksum, "version reported");
        self.with_session(|session| session.version_tag = tag);
        self.version_accepted = accepted;
        if !accepted {
            self.stage = Stage::Rejected;
        }
        accepted
    }

    async fn create_account(&self, id: u8, r: &mut Reader<'_>) -> Result<()> {
        let password = Hash::from_wire(r.bytes()?);
        let name = r.cstr(MAX_CHAT)?;
        // The second form of the request can say why it failed.
        let reply = |ok: bool, reason: u32| {
            let result = match (id, ok) {
                (id::CREATE_ACCOUNT, _) => u32::from(ok),
                (_, true) => 0,
                (_, false) => reason,
            };
            self.send(id, Writer::new().u32(result));
        };
        match auth::sign_up(&self.server, self.ip.into(), name, password).await? {
            Ok(()) => reply(true, 0),
            Err(SignupError::InvalidName) => reply(false, 2),
            Err(SignupError::NameTaken | SignupError::NotAllowed) => reply(false, 4),
        }
        Ok(())
    }

    async fn verify(
        &self,
        name: &str,
        client_token: u32,
        server_token: u32,
        proof: Hash,
    ) -> Result<std::result::Result<Account, LoginError>> {
        let token = self.with_session(|session| session.token);
        auth::check(&self.server, self.ip.into(), name, |stored| {
            token == Some(server_token) && stored.proof(client_token, server_token).matches(&proof)
        })
        .await
    }

    async fn change_password(&self, r: &mut Reader<'_>) -> Result<()> {
        let (client_token, server_token) = (r.u32()?, r.u32()?);
        let (proof, new) = (Hash::from_wire(r.bytes()?), Hash::from_wire(r.bytes()?));
        let name = r.ascii(MAX_NAME)?;
        let changed = match self.verify(name, client_token, server_token, proof).await? {
            Ok(account) if !self.server.hub.lock().is_online(account.id) => {
                self.server.store.set_password(account.id, new).await?;
                true
            }
            _ => false,
        };
        tracing::info!(session = self.id, name, changed, "password change requested");
        self.send(id::CHANGE_PASSWORD, Writer::new().u32(u32::from(changed)));
        Ok(())
    }

    async fn log_in(&mut self, id: u8, r: &mut Reader<'_>) -> Result<()> {
        let (client_token, server_token) = (r.u32()?, r.u32()?);
        let proof = Hash::from_wire(r.bytes()?);
        let name = r.ascii(MAX_NAME)?;
        let (game, version_byte) =
            self.with_session(|s| (s.product, s.version_byte)).unwrap_or((Tag::default(), 0));
        let mut outcome = self.verify(name, client_token, server_token, proof).await?;
        if self.server.config().server.strict_versions && !self.version_accepted {
            outcome = Err(LoginError::BadPassword);
        }
        let ok = match outcome {
            Ok(account) => {
                if !auth::enter(&self.server, self.id, self.ip.into(), account, game).await? {
                    return Ok(());
                }
                self.stage = Stage::LoggedIn;
                Ok(())
            }
            Err(error) => {
                tracing::info!(session = self.id, name, "login refused");
                Err(error)
            }
        };
        if id == id::LOG_IN {
            self.send(id, Writer::new().u32(u32::from(ok.is_ok())));
            return Ok(());
        }
        let body = match ok {
            Ok(()) => Writer::new().u32(0),
            Err(LoginError::NoAccount) => Writer::new().u32(1),
            Err(LoginError::Locked) if version_byte >= 0x0b => {
                Writer::new().u32(6).cstr("This account has been locked")
            }
            Err(LoginError::BadPassword | LoginError::Locked) => Writer::new().u32(2),
        };
        self.send(id, body);
        Ok(())
    }

    // After login

    fn enter_chat(&self, r: &mut Reader<'_>) -> Result<()> {
        r.cstr(MAX_CHAT)?;
        let statstring = r.cstr(MAX_CHAT)?;
        let Some(body) = self.with_session(|session| {
            // Only Diablo's statistics come from the client. The rest are the server's to state.
            if matches!(session.product, product::DIABLO | product::DIABLO_SHAREWARE)
                && statstring.starts_with(&session.product.reversed())
            {
                session.own_statstring = statstring.to_vec();
            }
            Writer::new().cstr(session.name()).cstr(session.statstring()).cstr(session.name())
        }) else {
            return Ok(());
        };
        self.send(id::ENTER_CHAT, body);
        Ok(())
    }

    fn game_list(&self, r: &mut Reader<'_>) -> Result<()> {
        let kind = r.u16()?;
        r.skip(14)?;
        let (name, password) = (r.cstr(MAX_GAME_NAME)?, r.cstr(MAX_GAME_NAME)?);
        let hub = self.server.hub.lock();
        if !name.is_empty() {
            let refused = |why| Writer::new().u32(0).u32(why);
            let body = match hub.look_up_game(self.id, name, password) {
                Lookup::NotFound => refused(0),
                Lookup::WrongPassword => refused(2),
                Lookup::Full => refused(3),
                Lookup::Started => refused(4),
                Lookup::Found(listing) => append_listing(Writer::new().u32(1).u32(0), &listing),
            };
            self.send(id::GAME_LIST, body);
            return Ok(());
        }
        let mut games = Writer::new();
        let mut count = 0;
        for listing in hub.list_games(self.id, kind) {
            const FIXED: usize = 4 + 32 + 3;
            let size = FIXED + listing.name.len() + listing.password.len() + listing.info.len();
            if 12 + games.len() + size > MAX_PACKET {
                break;
            }
            if count > 0 {
                games = games.u32(1);
            }
            games = append_listing(games, &listing);
            count += 1;
        }
        self.send(id::GAME_LIST, Writer::new().u32(count).u32(0).raw(games.finish()));
        Ok(())
    }

    fn start_game(&self, id: u8, r: &mut Reader<'_>) -> Result<()> {
        let state = if id == id::START_GAME_4 { u32::from(r.u16()?) } else { r.u32()? };
        r.skip(if id == id::START_GAME_4 { 6 } else { 4 })?;
        let (kind, option) = (r.u16()?, r.u16()?);
        r.skip(if id == id::START_GAME_3 { 12 } else { 8 })?;
        let (name, password) = (r.cstr(MAX_GAME_NAME)?, r.cstr(MAX_GAME_NAME)?);
        let info = r.cstr(MAX_GAME_INFO)?;
        let (status, private) = if id == id::START_GAME_4 {
            const KNOWN: u32 = 0x9f;
            anyhow::ensure!(state & !KNOWN == 0, "unknown game state {state:#x}");
            let status = match state {
                state if state & 0x08 != 0 => GameStatus::Started,
                state if state & 0x02 != 0 => GameStatus::Full,
                _ => GameStatus::Open,
            };
            (status, state & 0x01 != 0)
        } else {
            let status = match state & 0x0f {
                0x06 => GameStatus::Full,
                0x0e => GameStatus::Started,
                0x0c => GameStatus::Done,
                _ => GameStatus::Open,
            };
            (status, false)
        };
        let mut hub = self.server.hub.lock();
        if id == id::START_GAME_4 {
            hub.leave_channel(self.id);
        }
        let creating = hub.session(self.id).is_some_and(|session| session.game.is_none());
        // Ladder games carry a choice about disconnects.
        let disconnect_is_loss = id == id::START_GAME_4
            && matches!(kind, LADDER_GAME | IRONMAN_GAME)
            && (option == 2 || state & 0x10 != 0);
        let offer = GameOffer { name, password, info, kind, private, disconnect_is_loss, status };
        let has_game = hub.offer_game(self.id, &offer);
        drop(hub);
        // The three forms disagree on when they are answered and on which number means success.
        if id == id::START_GAME_4 {
            self.send(id, Writer::new().u32(u32::from(!has_game)));
            self.ping();
        } else if creating && status != GameStatus::Done {
            self.send(id, Writer::new().u32(u32::from(has_game)));
        }
        Ok(())
    }

    fn game_result(&self, r: &mut Reader<'_>) -> Result<()> {
        const MAX_PLAYERS: u32 = 8;
        r.skip(4)?;
        let count = r.u32()?;
        anyhow::ensure!(count <= MAX_PLAYERS, "too many players");
        let mut outcomes = Vec::new();
        for _ in 0..count {
            outcomes.push(match r.u32()? {
                1 => Some(Outcome::Win),
                2 => Some(Outcome::Loss),
                3 => Some(Outcome::Draw),
                4 => Some(Outcome::Disconnect),
                _ => None,
            });
        }
        let mut claims = Vec::new();
        for outcome in outcomes {
            claims.push((r.ascii(MAX_NAME)?, outcome));
        }
        self.server.hub.lock().report_game(self.id, &claims);
        Ok(())
    }

    async fn ladder(&self, r: &mut Reader<'_>) -> Result<()> {
        const MAX_ENTRIES: u32 = 20;
        let (tag, ladder, order, start, count) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?);
        let count = count.min(MAX_ENTRIES);
        let game = self.with_session(|session| session.product).unwrap_or_default();
        let kind = if ladder == 3 { 3 } else { 1 };
        let found = match ladder_order(order) {
            Some(order) => self.server.store.ladder(game, kind, order, start.into(), count.into()).await?,
            None => Vec::new(),
        };
        let mut body = Writer::new().u32(tag).u32(ladder).u32(order).u32(start).u32(count);
        for index in 0..count {
            let entry = found.get(index as usize);
            let record = entry.map(|(_, record)| record.clone()).unwrap_or_default();
            let last_game = match entry {
                Some(_) => files::filetime(UNIX_EPOCH + Duration::from_secs(record.last_game_at.max(0) as u64)),
                None => 0,
            };
            // The same twice: the current season, and the last one put on display. There is only one.
            for _ in 0..2 {
                body = body
                    .u32(record.wins as u32)
                    .u32(record.losses as u32)
                    .u32(record.disconnects as u32)
                    .u32(record.rating as u32)
                    .u32(record.rank.max(1) as u32 - 1);
            }
            body = body.u32(start.wrapping_add(index)).u32(0).u32(0).u32(record.high_rating as u32).u32(0).u32(0);
            body = body.u64(last_game).u64(last_game).cstr(entry.map_or(" ", |(name, _)| name));
        }
        self.send(id::LADDER, body);
        Ok(())
    }

    async fn ladder_search(&self, r: &mut Reader<'_>) -> Result<()> {
        r.skip(4)?;
        let (ladder, order) = (r.u32()?, r.u32()?);
        let name = r.ascii(MAX_NAME)?;
        let game = self.with_session(|session| session.product).unwrap_or_default();
        let kind = if ladder == 3 { 3 } else { 1 };
        let place = match ladder_order(order) {
            Some(order) => self.server.store.ladder_place(game, kind, order, name).await?,
            None => None,
        };
        self.send(id::LADDER_SEARCH, Writer::new().u32(place.map_or(u32::MAX, |place| place as u32)));
        Ok(())
    }

    /// Picks the banner after the one shown before.
    async fn ad(&self, r: &mut Reader<'_>) -> Result<()> {
        r.skip(8)?;
        let previous = r.u32()?;
        let game = self.with_session(|session| session.product).unwrap_or_default().to_string();
        let config = self.server.config();
        let suits = |ad: &Ad| ad.product.as_ref().is_none_or(|product| *product == game);
        let mut ads = config.ads.iter().zip(1u32..).filter(|(ad, _)| suits(ad)).collect::<Vec<_>>();
        ads.sort_by_key(|(_, number)| (*number <= previous, *number));
        let Some((ad, number)) = ads.first() else { return Ok(()) };
        let extension = match ad.file.rsplit_once('.').map(|(_, extension)| extension.to_ascii_lowercase()) {
            Some(extension) if extension == "pcx" => b".pcx",
            Some(extension) if extension == "smk" => b".smk",
            Some(extension) if extension == "mng" || extension == "png" => b".mng",
            _ => anyhow::bail!("unsupported banner {}", ad.file),
        };
        let modified = self.server.files.modified(&ad.file).await;
        self.send(id::AD, Writer::new().u32(*number).raw(extension).u64(modified).cstr(&ad.file).cstr(&ad.url));
        Ok(())
    }

    async fn my_friends(&self) -> Result<Vec<Friend>> {
        match self.with_session(|session| session.account.as_ref().map(|account| account.id)).flatten() {
            Some(account) => self.server.store.friends(account).await,
            None => Ok(Vec::new()),
        }
    }

    async fn friends(&self) -> Result<()> {
        let friends = self.my_friends().await?;
        let mut body = Writer::new().raw([friends.len() as u8]);
        for friend in &friends {
            body = append_friend(body, &self.server, friend);
        }
        self.send(id::FRIENDS, body);
        Ok(())
    }

    async fn friend(&self, r: &mut Reader<'_>) -> Result<()> {
        let [number] = r.bytes()?;
        let friends = self.my_friends().await?;
        let Some(friend) = friends.get(usize::from(number)) else { return Ok(()) };
        let presence = self.server.hub.lock().presence(friend.account);
        let body = Writer::new().raw([number]);
        let body = match presence {
            None => body.raw([0, 0]).u32(0).cstr(""),
            Some(p) => body.raw([p.status | u8::from(friend.mutual), p.location]).u32(p.product.0).cstr(&p.place),
        };
        self.send(id::FRIEND, body);
        Ok(())
    }

    async fn read_user_data(&self, r: &mut Reader<'_>) -> Result<()> {
        const MAX_NAMES: u32 = 8;
        const MAX_KEYS: u32 = 32;
        let (names, keys, request) = (r.u32()?, r.u32()?, r.u32()?);
        anyhow::ensure!(names <= MAX_NAMES && keys <= MAX_KEYS, "asked for too much");
        let names = (0..names).map(|_| r.ascii(MAX_CHAT)).collect::<Result<Vec<_>, _>>()?;
        let keys = (0..keys).map(|_| r.ascii(MAX_CHAT)).collect::<Result<Vec<_>, _>>()?;
        let mut body = Writer::new().u32(names.len() as u32).u32(keys.len() as u32).u32(request);
        let own_name = self.with_session(|session| session.name().to_owned()).unwrap_or_default();
        for name in &names {
            let account = match self.server.store.account(name).await? {
                Some(account) => Some(account),
                None => self.server.store.account(&own_name).await?,
            };
            let values = match account {
                Some(account) => self.user_values(&account, &keys).await?,
                None => vec![String::new(); keys.len()],
            };
            for value in values {
                if 4 + body.len() + value.len() < MAX_PACKET {
                    body = body.cstr(value);
                } else {
                    body = body.cstr("");
                }
            }
        }
        self.send(id::READ_USER_DATA, body);
        Ok(())
    }

    /// Looks up keys like `profile\sex` and `Record\W2BN\0\wins`. Everything else is private.
    async fn user_values(&self, account: &Account, keys: &[&str]) -> Result<Vec<String>> {
        let profile = self.server.store.profile(account.id).await?;
        let mut records: HashMap<Tag, HashMap<i64, Record>> = HashMap::new();
        let mut values = Vec::new();
        for key in keys.iter().filter(|key| !key.is_empty()) {
            let parts: Vec<&str> = key.split('\\').collect();
            values.push(match parts.as_slice() {
                [area, field] if area.eq_ignore_ascii_case("profile") => {
                    profile.get(&field.to_ascii_lowercase()).cloned().unwrap_or_default()
                }
                [area, game, ladder, field] if area.eq_ignore_ascii_case("record") => {
                    let (Some(game), Ok(ladder)) = (Tag::parse(game), ladder.parse::<i64>()) else {
                        values.push(String::new());
                        continue;
                    };
                    if let Entry::Vacant(entry) = records.entry(game) {
                        entry.insert(self.server.store.records(account.id, game).await?);
                    }
                    record_value(records[&game].get(&ladder).unwrap_or(&Record::default()), field)
                }
                _ => String::new(),
            });
        }
        Ok(values)
    }

    async fn write_user_data(&self, r: &mut Reader<'_>) -> Result<()> {
        const MAX_VALUE: usize = 512;
        let (names, keys) = (r.u32()?, r.u32()?);
        anyhow::ensure!(names == 1 && keys <= 32, "unexpected profile update");
        r.ascii(MAX_CHAT)?; // only a player's own profile can be changed, whatever name is given
        let keys = (0..keys).map(|_| r.ascii(MAX_CHAT)).collect::<Result<Vec<_>, _>>()?;
        let values = keys.iter().map(|_| r.cstr(MAX_VALUE)).collect::<Result<Vec<_>, _>>()?;
        let Some(Some(account)) = self.with_session(|s| s.account.as_ref().map(|a| a.id)) else {
            return Ok(());
        };
        for (key, value) in keys.iter().zip(values) {
            let Some((area, field)) = key.split_once('\\') else { continue };
            let field = field.to_ascii_lowercase();
            let known = ["sex", "age", "location", "description", "clanname"].contains(&field.as_str());
            if area.eq_ignore_ascii_case("profile") && known {
                self.server.store.set_profile(account, &field, &String::from_utf8_lossy(value)).await?;
            }
        }
        Ok(())
    }
}

fn ladder_order(number: u32) -> Option<LadderOrder> {
    match number {
        0 => Some(LadderOrder::Rating),
        2 => Some(LadderOrder::Wins),
        3 => Some(LadderOrder::Games),
        _ => None,
    }
}

fn append_friend(body: Writer, server: &Server, friend: &Friend) -> Writer {
    let body = body.cstr(&friend.name);
    match server.hub.lock().presence(friend.account) {
        None => body.raw([0, 0]).u32(0).cstr(""),
        Some(p) => body.raw([p.status | u8::from(friend.mutual), p.location]).u32(p.product.0).cstr(&p.place),
    }
}

/// Tells a game that its friends list changed. Other clients learn it from the command's answer.
pub fn friend_added(server: &Server, id: SessionId, friend: &Friend) {
    send_to_game(server, id, || packet(id::FRIEND_ADDED, append_friend(Writer::new(), server, friend)));
}

pub fn friend_removed(server: &Server, id: SessionId, number: u8) {
    send_to_game(server, id, || packet(id::FRIEND_REMOVED, Writer::new().raw([number])));
}

fn send_to_game(server: &Server, id: SessionId, packet: impl FnOnce() -> Vec<u8>) {
    let outbox = {
        let hub = server.hub.lock();
        hub.session(id).filter(|s| SUPPORTED.contains(&s.product)).map(|s| s.outbox.clone())
    };
    if let Some(outbox) = outbox {
        outbox.send(Out::Raw(packet()));
    }
}

fn append_listing(body: Writer, game: &Listing) -> Writer {
    let status = match game.status {
        GameStatus::Open => 0x04,
        GameStatus::Full => 0x06,
        GameStatus::Started => 0x0e,
        GameStatus::Done => 0x0c,
    };
    body.u16(game.kind)
        .u16(1)
        .u16(2)
        .raw(game.address.port().to_be_bytes())
        .raw(game.address.ip().octets())
        .u32(0)
        .u32(0)
        .u32(status)
        .u32(0x2b)
        .cstr(&game.name)
        .cstr(&game.password)
        .cstr(&game.info)
}

fn record_value(record: &Record, field: &str) -> String {
    match field.to_ascii_lowercase().as_str() {
        "wins" => record.wins.to_string(),
        "losses" => record.losses.to_string(),
        "draws" => record.draws.to_string(),
        "disconnects" => record.disconnects.to_string(),
        "rating" => record.rating.to_string(),
        "rank" => record.rank.to_string(),
        "high rating" => record.high_rating.to_string(),
        "last game result" if record.last_result.is_empty() => "NONE".into(),
        "last game result" => record.last_result.clone(),
        _ => String::new(),
    }
}
