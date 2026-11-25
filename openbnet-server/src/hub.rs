//! Who is connected, and the channels and games they are in.
//!
//! All of it sits behind one lock that is only held for short, non-blocking work. Database access happens
//! outside it.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::config::{self, Config};
use crate::range::Range;
use crate::store::{Account, AccountId, FinishedGame, IpBan, Outcome, Record, Role, unix_now};
use crate::wire::{Tag, product};

pub type SessionId = u32;
type ChannelId = u32;
type GameId = u32;

pub const VOID: &str = "The Void";

/// What chat clients are shown beside a name.
pub mod user_flags {
    pub const ADMIN: u32 = 0x01;
    pub const CHANNEL_OPERATOR: u32 = 0x02;
    pub const MODERATOR: u32 = 0x08;
    /// No working UDP, shown as a plug.
    pub const NO_UDP: u32 = 0x10;
}

mod channel_flags {
    pub const PUBLIC: u32 = 0x01;
    pub const MODERATED: u32 = 0x02;
    pub const SILENT: u32 = 0x08;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum EventKind {
    ShowUser = 0x01,
    Join = 0x02,
    Leave = 0x03,
    Whisper = 0x04,
    Talk = 0x05,
    Broadcast = 0x06,
    Channel = 0x07,
    UserFlags = 0x09,
    WhisperSent = 0x0a,
    Info = 0x12,
    Error = 0x13,
    Emote = 0x17,
    /// Everyone in a channel just entered has been shown. Games aren't sent this.
    Entered = 0x100,
}

/// Something shown in chat. Each protocol writes it in its own way.
#[derive(Debug, Clone)]
pub struct Event {
    pub kind: EventKind,
    pub flags: u32,
    pub latency: u32,
    pub name: String,
    /// What the person named is using.
    pub product: Tag,
    pub text: Vec<u8>,
}

impl Event {
    pub fn info(text: impl Into<Vec<u8>>) -> Event {
        Event::plain(EventKind::Info, text)
    }

    pub fn error(text: impl Into<Vec<u8>>) -> Event {
        Event::plain(EventKind::Error, text)
    }

    fn plain(kind: EventKind, text: impl Into<Vec<u8>>) -> Event {
        Event { kind, flags: 0, latency: 0, name: String::new(), product: Tag::default(), text: text.into() }
    }
}

pub enum Out {
    Event(Event),
    Raw(Vec<u8>),
}

/// The sending side of a connection. One that can't keep up is closed rather than buffered for ever.
#[derive(Clone)]
pub struct Outbox {
    pub queue: mpsc::Sender<Out>,
    pub close: CancellationToken,
}

impl Outbox {
    pub fn send(&self, out: Out) {
        if self.queue.try_send(out).is_err() {
            self.close.cancel();
        }
    }
}

pub struct Session {
    pub id: SessionId,
    /// The random number the game mixes into its password proof.
    pub token: u32,
    pub ip: IpAddr,
    pub outbox: Outbox,
    pub product: Tag,
    pub platform: Tag,
    pub version_byte: u32,
    pub version_tag: Option<String>,
    pub account: Option<Account>,
    pub records: HashMap<i64, Record>,
    /// What the client says about its character. Diablo sends this; other games' is computed.
    pub own_statstring: Vec<u8>,
    pub flags: u32,
    pub latency: u32,
    /// Where other players reach this player's games.
    pub game_address: SocketAddrV4,
    pub time_bias_minutes: i32,
    pub channel: Option<ChannelId>,
    pub last_channel: Option<Vec<u8>>,
    pub game: Option<GameId>,
    pub ignoring: HashSet<String>,
    pub away: Option<Vec<u8>>,
    pub busy: Option<Vec<u8>>,
    pub last_whisper_from: Option<String>,
    welcomed: bool,
    recent_lines: VecDeque<(Instant, usize)>,
}

impl Session {
    pub fn name(&self) -> &str {
        self.account.as_ref().map_or("", |account| &account.name)
    }

    pub fn role(&self) -> Role {
        self.account.as_ref().map_or(Role::Player, |account| account.role)
    }

    fn send(&self, event: Event) {
        self.outbox.send(Out::Event(event));
    }

    fn ignores(&self, name: &str) -> bool {
        self.ignoring.contains(&name.to_ascii_lowercase())
    }

    /// The text games turn into an icon and statistics.
    pub fn statstring(&self) -> Vec<u8> {
        if !self.own_statstring.is_empty() {
            return self.own_statstring.clone();
        }
        let mut out = self.product.reversed().to_vec();
        let record = |ladder| self.records.get(&ladder).cloned().unwrap_or_default();
        let (normal, ladder, ironman) = (record(0), record(1), record(3));
        match self.product {
            product::WARCRAFT2 => out.extend(
                format!(
                    " {} {} {} 0 0 {} {} {}",
                    ladder.rating,
                    ladder.rank,
                    normal.wins,
                    ladder.rating.max(ironman.rating),
                    ironman.rating,
                    ironman.rank
                )
                .bytes(),
            ),
            product::STARCRAFT | product::BROOD_WAR if self.version_byte > 0xc7 => {
                out.extend(
                    format!(" {} {} {} 0 0 {} 0 0 ", ladder.rating, ladder.rank, normal.wins, ladder.high_rating)
                        .bytes(),
                );
                out.extend(self.product.reversed());
            }
            product::STARCRAFT | product::BROOD_WAR | product::STARCRAFT_SHAREWARE => {
                out.extend(format!(" {} {} {} 0 0", ladder.rating, ladder.rank, normal.wins).bytes());
            }
            product::DIABLO | product::DIABLO_SHAREWARE => out.extend(b" 0 0 0 0 0 0 0 0 0"),
            _ => {}
        }
        out
    }
}

struct Channel {
    name: Vec<u8>,
    alias: Option<String>,
    /// The games it is offered to. Empty means all.
    products: Vec<Tag>,
    max: Option<usize>,
    permanent: bool,
    moderated: bool,
    staff_only: bool,
    silent: bool,
    topic: Vec<u8>,
    /// Newest first, the order PvPGN lists them in.
    members: Vec<SessionId>,
    operators: HashSet<SessionId>,
    banned: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Open,
    Full,
    Started,
    Done,
}

struct Player {
    account: AccountId,
    name: String,
    session: Option<SessionId>,
    reports: Option<Vec<(AccountId, Outcome)>>,
}

struct Game {
    name: Vec<u8>,
    password: Vec<u8>,
    info: Vec<u8>,
    product: Tag,
    version_tag: Option<String>,
    kind: u16,
    status: GameStatus,
    private: bool,
    disconnect_is_loss: bool,
    started_at: i64,
    address: SocketAddrV4,
    players: Vec<Player>,
}

impl Game {
    fn joinable(&self) -> bool {
        matches!(self.status, GameStatus::Open | GameStatus::Full)
    }
}

/// A game as shown in a game list.
#[derive(Debug, Clone)]
pub struct Listing {
    pub kind: u16,
    pub address: SocketAddrV4,
    pub status: GameStatus,
    pub name: Vec<u8>,
    pub password: Vec<u8>,
    pub info: Vec<u8>,
}

#[derive(Debug)]
pub enum Lookup {
    NotFound,
    WrongPassword,
    Full,
    Started,
    Found(Listing),
}

pub const LADDER_GAME: u16 = 9;
pub const IRONMAN_GAME: u16 = 0x10;
/// Which ladder a kind of game counts for. 0 means none. The Diablo games number their kinds differently.
fn ladder_of(product: Tag, kind: u16) -> i64 {
    match (product, kind) {
        (product::WARCRAFT2 | product::STARCRAFT | product::BROOD_WAR, LADDER_GAME) => 1,
        (product::WARCRAFT2, IRONMAN_GAME) => 3,
        _ => 0,
    }
}

const LADDER_WINS_NEEDED: i64 = 10;

pub struct GameOffer<'a> {
    pub name: &'a [u8],
    pub password: &'a [u8],
    pub info: &'a [u8],
    pub kind: u16,
    pub private: bool,
    pub disconnect_is_loss: bool,
    pub status: GameStatus,
}

/// How often each address did something lately.
#[derive(Default)]
struct Throttle(HashMap<IpAddr, VecDeque<Instant>>);

impl Throttle {
    /// Someone with IPv6 has a whole /64 to themselves.
    fn key(ip: IpAddr) -> IpAddr {
        match ip {
            IpAddr::V4(_) => ip,
            IpAddr::V6(ip) => IpAddr::V6((u128::from(ip) >> 64 << 64).into()),
        }
    }

    fn full(&mut self, ip: IpAddr, max: usize, window_secs: u64) -> bool {
        let window = Duration::from_secs(window_secs);
        let recent = |times: &VecDeque<Instant>| times.iter().filter(|time| time.elapsed() < window).count();
        max > 0 && self.0.get(&Throttle::key(ip)).is_some_and(|times| recent(times) >= max)
    }

    fn note(&mut self, ip: IpAddr, window_secs: u64) {
        const MAX_TIMES: usize = 1000;
        const MAX_ADDRESSES: usize = 10_000;
        let window = Duration::from_secs(window_secs);
        if self.0.len() >= MAX_ADDRESSES {
            self.0.retain(|_, times| times.back().is_some_and(|time| time.elapsed() < window));
        }
        let times = self.0.entry(Throttle::key(ip)).or_default();
        while times.len() >= MAX_TIMES || times.front().is_some_and(|time| time.elapsed() >= window) {
            times.pop_front();
        }
        times.push_back(Instant::now());
    }
}

/// What someone's friends may see, in the numbers the games use.
pub struct Presence {
    pub status: u8,
    pub location: u8,
    pub product: Tag,
    pub place: Vec<u8>,
}

/// Someone in a channel, as others may see them.
pub struct Member {
    pub name: String,
    pub flags: u32,
    pub product: Tag,
}

pub struct Hub {
    config: Config,
    sessions: HashMap<SessionId, Session>,
    by_account: HashMap<AccountId, SessionId>,
    channels: HashMap<ChannelId, Channel>,
    games: HashMap<GameId, Game>,
    next_id: u32,
    failures: Throttle,
    signups: Throttle,
    bans: Vec<IpBan>,
    /// None once the server is stopping.
    finished: Option<mpsc::UnboundedSender<FinishedGame>>,
}

impl Hub {
    pub fn new(config: &Config, finished: mpsc::UnboundedSender<FinishedGame>) -> Hub {
        let mut hub = Hub {
            config: config.clone(),
            sessions: HashMap::new(),
            by_account: HashMap::new(),
            channels: HashMap::new(),
            games: HashMap::new(),
            next_id: 1,
            failures: Throttle::default(),
            signups: Throttle::default(),
            bans: Vec::new(),
            finished: Some(finished),
        };
        for channel in &config.channels {
            hub.add_permanent(channel);
        }
        if hub.find_channel(VOID.as_bytes()).is_none() {
            let void = config::Channel {
                name: VOID.into(),
                alias: None,
                products: Vec::new(),
                max: None,
                moderated: false,
                staff_only: false,
                topic: String::new(),
            };
            hub.add_permanent(&void);
        }
        hub
    }

    fn add_permanent(&mut self, channel: &config::Channel) {
        let id = self.next_id();
        let is_void = channel.name.eq_ignore_ascii_case(VOID);
        self.channels.insert(
            id,
            Channel {
                name: channel.name.clone().into_bytes(),
                alias: channel.alias.clone(),
                products: channel.products.iter().filter_map(|product| Tag::parse(product)).collect(),
                max: channel.max,
                permanent: true,
                moderated: channel.moderated || is_void,
                staff_only: channel.staff_only,
                silent: is_void,
                topic: channel.topic.clone().into_bytes(),
                members: Vec::new(),
                operators: HashSet::new(),
                banned: HashSet::new(),
            },
        );
    }

    fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    // Sessions

    /// Registers a connection, or refuses it if the server is full.
    pub fn connect(&mut self, ip: IpAddr, product: Tag, outbox: Outbox) -> Option<SessionId> {
        let limits = &self.config.server;
        let same_ip = self.sessions.values().filter(|s| s.ip == ip).count();
        if self.finished.is_none()
            || self.is_banned(ip)
            || self.sessions.len() >= limits.max_connections
            || (limits.max_connections_per_ip > 0 && same_ip >= limits.max_connections_per_ip)
        {
            return None;
        }
        let id = self.next_id();
        self.sessions.insert(
            id,
            Session {
                id,
                token: rand::random(),
                ip,
                outbox,
                product,
                platform: Tag::default(),
                version_byte: 0,
                version_tag: None,
                account: None,
                records: HashMap::new(),
                own_statstring: Vec::new(),
                flags: user_flags::NO_UDP,
                latency: 0,
                game_address: SocketAddrV4::new(
                    match ip {
                        IpAddr::V4(ip) => ip,
                        // Only games host, and they connect over IPv4.
                        IpAddr::V6(_) => Ipv4Addr::UNSPECIFIED,
                    },
                    6112,
                ),
                time_bias_minutes: 0,
                channel: None,
                last_channel: None,
                game: None,
                ignoring: HashSet::new(),
                away: None,
                busy: None,
                last_whisper_from: None,
                welcomed: false,
                recent_lines: VecDeque::new(),
            },
        );
        Some(id)
    }

    /// Ends every session and refuses new ones. Games still running are judged by the reports they have.
    pub fn close(&mut self) {
        let ids: Vec<SessionId> = self.sessions.keys().copied().collect();
        for id in ids {
            self.disconnect(id);
        }
        self.finished = None;
    }

    pub fn is_banned(&self, ip: IpAddr) -> bool {
        let now = unix_now();
        !ip.is_loopback()
            && self.bans.iter().any(|ban| ban.range.contains(ip) && (ban.until == 0 || ban.until > now))
    }

    pub fn bans(&self) -> &[IpBan] {
        &self.bans
    }

    /// Adds a ban and ends the sessions it covers.
    pub fn add_ban(&mut self, ban: IpBan) {
        self.bans.retain(|old| old.range != ban.range);
        self.bans.push(ban);
        let ids: Vec<SessionId> = self
            .sessions
            .values()
            .filter(|session| self.is_banned(session.ip))
            .map(|session| session.id)
            .collect();
        for id in ids {
            self.disconnect(id);
        }
    }

    pub fn remove_ban(&mut self, range: Range) {
        self.bans.retain(|ban| ban.range != range);
    }

    pub fn disconnect(&mut self, id: SessionId) {
        self.leave_channel(id);
        self.leave_game(id);
        if let Some(session) = self.sessions.remove(&id) {
            session.outbox.close.cancel();
            if let Some(account) = session.account
                && self.by_account.get(&account.id) == Some(&id)
            {
                self.by_account.remove(&account.id);
            }
        }
    }

    /// True if the account was online.
    pub fn disconnect_account(&mut self, account: AccountId) -> bool {
        match self.by_account.get(&account).copied() {
            Some(id) => {
                self.disconnect(id);
                true
            }
            None => false,
        }
    }

    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.get(&id)
    }

    pub fn session_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions.get_mut(&id)
    }

    pub fn session_by_token(&mut self, token: u32) -> Option<&mut Session> {
        self.sessions.values_mut().find(|session| session.token == token)
    }

    pub fn sessions(&self) -> impl Iterator<Item = &Session> {
        self.sessions.values()
    }

    pub fn find_user(&self, name: &str) -> Option<&Session> {
        let name = name.strip_prefix('*').unwrap_or(name);
        self.by_account
            .values()
            .filter_map(|id| self.sessions.get(id))
            .find(|session| session.name().eq_ignore_ascii_case(name))
    }

    pub fn user_count(&self) -> usize {
        self.by_account.len()
    }

    pub fn game_count(&self) -> usize {
        self.games.len()
    }

    pub fn is_online(&self, account: AccountId) -> bool {
        self.by_account.contains_key(&account)
    }

    /// Logs a session in. An older login to the same account is closed.
    pub fn log_in(&mut self, id: SessionId, account: Account, records: HashMap<i64, Record>) -> bool {
        if !self.sessions.contains_key(&id) {
            return false;
        }
        if let Some(old) = self.by_account.get(&account.id).copied()
            && old != id
        {
            self.disconnect(old);
        }
        self.by_account.insert(account.id, id);
        if let Some(session) = self.sessions.get_mut(&id) {
            session.account = Some(account);
            session.records = records;
        }
        true
    }

    pub fn send(&self, id: SessionId, event: Event) {
        if let Some(session) = self.sessions.get(&id) {
            session.send(event);
        }
    }

    pub fn broadcast(&self, event: &Event) {
        for id in self.by_account.values() {
            self.send(*id, event.clone());
        }
    }

    pub fn set_latency(&mut self, id: SessionId, latency: u32) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        if session.latency != latency {
            session.latency = latency;
            self.announce_flags(id);
        }
    }

    pub fn set_udp_works(&mut self, id: SessionId) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.flags &= !user_flags::NO_UDP;
        }
    }

    pub fn set_role(&mut self, account: AccountId, role: Role) {
        if let Some(id) = self.by_account.get(&account).copied() {
            if let Some(account) = self.sessions.get_mut(&id).and_then(|s| s.account.as_mut()) {
                account.role = role;
            }
            self.refresh_flags(id);
        }
    }

    pub fn set_muted(&mut self, account: AccountId, muted: bool) {
        if let Some(id) = self.by_account.get(&account)
            && let Some(account) = self.sessions.get_mut(id).and_then(|s| s.account.as_mut())
        {
            account.muted = muted;
        }
    }

    pub fn set_records(&mut self, account: AccountId, product: Tag, records: HashMap<i64, Record>) {
        if let Some(id) = self.by_account.get(&account)
            && let Some(session) = self.sessions.get_mut(id)
            && session.product == product
        {
            session.records = records;
        }
    }

    // Limits by address

    pub fn too_many_failures(&mut self, ip: IpAddr) -> bool {
        let rules = &self.config.accounts;
        self.failures.full(ip, rules.max_failures, rules.window_secs)
    }

    pub fn note_failure(&mut self, ip: IpAddr) {
        self.failures.note(ip, self.config.accounts.window_secs);
    }

    pub fn too_many_signups(&mut self, ip: IpAddr) -> bool {
        let rules = &self.config.accounts;
        self.signups.full(ip, rules.max_signups, rules.signup_window_secs)
    }

    pub fn note_signup(&mut self, ip: IpAddr) {
        self.signups.note(ip, self.config.accounts.signup_window_secs);
    }

    /// Applies new settings. Channels already open stay as they are.
    pub fn reconfigure(&mut self, config: &Config) {
        self.config = config.clone();
        for channel in &config.channels {
            if self.find_channel(channel.name.as_bytes()).is_none() {
                self.add_permanent(channel);
            }
        }
    }

    // Channels

    fn find_channel(&self, name: &[u8]) -> Option<ChannelId> {
        let mut ids: Vec<_> =
            self.channels.iter().filter(|(_, c)| c.name.eq_ignore_ascii_case(name)).map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids.first().copied()
    }

    /// The channel a name leads to. An alias leads to its first copy with room, opening another if needed.
    fn resolve_channel(&mut self, name: &[u8]) -> Option<ChannelId> {
        if let Some(id) = self.find_channel(name) {
            return Some(id);
        }
        let is_copy = |channel: &Channel| {
            channel.alias.as_ref().is_some_and(|alias| alias.as_bytes().eq_ignore_ascii_case(name))
        };
        let mut copies: Vec<ChannelId> =
            self.channels.iter().filter(|(_, c)| is_copy(c)).map(|(id, _)| *id).collect();
        copies.sort_unstable();
        let has_room =
            |id: &ChannelId| self.channels.get(id).is_some_and(|c| c.max.is_none_or(|max| c.members.len() < max));
        if let Some(id) = copies.iter().copied().find(has_room) {
            return Some(id);
        }
        let first = self.channels.get(copies.first()?)?;
        let alias = first.alias.clone()?;
        let copy = Channel {
            name: format!("{alias}-{}", copies.len() + 1).into_bytes(),
            alias: Some(alias),
            products: first.products.clone(),
            max: first.max,
            permanent: true,
            moderated: first.moderated,
            staff_only: first.staff_only,
            silent: first.silent,
            topic: first.topic.clone(),
            members: Vec::new(),
            operators: HashSet::new(),
            banned: HashSet::new(),
        };
        let id = self.next_id();
        self.channels.insert(id, copy);
        Some(id)
    }

    /// Moves a session to a channel. If it may not enter, it goes to The Void.
    pub fn join_channel(&mut self, id: SessionId, name: &[u8]) {
        if !self.try_join(id, name) {
            self.try_join(id, VOID.as_bytes());
        }
        self.refresh_flags(id);
    }

    fn try_join(&mut self, id: SessionId, name: &[u8]) -> bool {
        let Some(session) = self.sessions.get(&id) else { return true };
        let (role, account_name) = (session.role(), session.name().to_ascii_lowercase());
        if session.account.is_none() {
            return true;
        }
        let target = self.resolve_channel(name);
        if target.is_some() && target == self.sessions[&id].channel {
            return true;
        }
        self.leave_channel(id);
        if let Some(channel) = target.and_then(|target| self.channels.get(&target)) {
            let refusal = if channel.banned.contains(&account_name) {
                Some("You are banned from that channel.")
            } else if channel.staff_only && role < Role::Moderator {
                Some("That channel is for Admins/Operators only.")
            } else if role < Role::Moderator && channel.max.is_some_and(|max| channel.members.len() >= max) {
                Some("The channel is currently full.")
            } else {
                None
            };
            if let Some(refusal) = refusal {
                self.send(id, Event::error(refusal));
                return false;
            }
        }
        self.leave_game(id);
        let channel_id = target.unwrap_or_else(|| {
            let channel_id = self.next_id();
            self.channels.insert(
                channel_id,
                Channel {
                    name: name.to_vec(),
                    alias: None,
                    products: Vec::new(),
                    max: None,
                    permanent: false,
                    moderated: false,
                    staff_only: false,
                    silent: false,
                    topic: Vec::new(),
                    members: Vec::new(),
                    operators: HashSet::new(),
                    banned: HashSet::new(),
                },
            );
            channel_id
        });
        self.enter(id, channel_id);
        true
    }

    fn enter(&mut self, id: SessionId, channel_id: ChannelId) {
        let (Some(channel), Some(session)) = (self.channels.get_mut(&channel_id), self.sessions.get_mut(&id))
        else {
            return;
        };
        channel.members.insert(0, id);
        session.channel = Some(channel_id);
        session.last_channel = Some(channel.name.clone());
        let becomes_operator = !channel.permanent
            && !channel.silent
            && channel.members.len() == 1
            && session.role() < Role::Moderator;
        if becomes_operator {
            channel.operators.insert(id);
        }
        let mut flags = 0;
        if channel.permanent {
            flags |= channel_flags::PUBLIC;
        }
        if channel.moderated {
            flags |= channel_flags::MODERATED;
        }
        if channel.silent {
            flags |= channel_flags::SILENT;
        }
        let welcome = !std::mem::replace(&mut session.welcomed, true);

        let (Some(channel), Some(session)) = (self.channels.get(&channel_id), self.sessions.get(&id)) else {
            return;
        };
        session.send(Event {
            kind: EventKind::Channel,
            flags,
            latency: session.latency,
            name: session.name().into(),
            product: session.product,
            text: channel.name.clone(),
        });
        if becomes_operator {
            session.send(Event::info("you are now tempOP for this channel"));
        }
        if !channel.silent {
            for member in channel.members.iter().filter_map(|member| self.sessions.get(member)) {
                session.send(about(EventKind::ShowUser, member));
                if member.id != id {
                    member.send(about(EventKind::Join, session));
                }
            }
        }
        session.send(Event::plain(EventKind::Entered, channel.name.clone()));
        if welcome {
            for line in &self.config.chat.welcome {
                session.send(Event::info(line.as_bytes()));
            }
        }
        if channel.silent {
            session.send(Event::info("This channel does not have chat privileges."));
        }
        if !channel.topic.is_empty() {
            session.send(topic_event(channel));
        }
        if channel.moderated {
            session.send(Event::error("This channel is moderated."));
        }
    }

    /// The topic of a session's channel, as shown to it.
    pub fn show_topic(&self, id: SessionId) -> Option<Event> {
        self.channel_of(id).map(topic_event)
    }

    /// The topic of a channel.
    pub fn topic(&self, name: &[u8]) -> Option<&[u8]> {
        let topic = &self.channels.get(&self.find_channel(name)?)?.topic;
        (!topic.is_empty()).then_some(topic)
    }

    /// Sets the topic of a session's channel. If the channel is permanent, returns its name.
    pub fn set_topic(&mut self, id: SessionId, topic: &[u8]) -> Option<Vec<u8>> {
        let channel = self.channels.get_mut(&self.sessions.get(&id)?.channel?)?;
        channel.topic = topic.to_vec();
        channel.permanent.then(|| channel.name.clone())
    }

    /// Restores a topic set earlier.
    pub fn restore_topic(&mut self, name: &str, topic: &str) {
        if let Some(id) = self.find_channel(name.as_bytes())
            && let Some(channel) = self.channels.get_mut(&id)
        {
            channel.topic = topic.as_bytes().to_vec();
        }
    }

    pub fn leave_channel(&mut self, id: SessionId) {
        let Some(session) = self.sessions.get_mut(&id) else { return };
        let Some(channel_id) = session.channel.take() else { return };
        let session = &self.sessions[&id];
        let Some(channel) = self.channels.get_mut(&channel_id) else { return };
        channel.members.retain(|member| *member != id);
        channel.operators.remove(&id);
        if !channel.silent {
            let mut event = about(EventKind::Leave, session);
            event.text.clear();
            for member in channel.members.iter().filter_map(|member| self.sessions.get(member)) {
                member.send(event.clone());
            }
        }
        if channel.members.is_empty() && !channel.permanent {
            self.channels.remove(&channel_id);
        }
    }

    fn channel_of(&self, id: SessionId) -> Option<&Channel> {
        self.channels.get(&self.sessions.get(&id)?.channel?)
    }

    pub fn channel_name(&self, id: SessionId) -> Option<&[u8]> {
        self.channel_of(id).map(|channel| channel.name.as_slice())
    }

    /// Open channels anyone may see, and how many are in each.
    pub fn public_channels(&self) -> Vec<(Vec<u8>, usize)> {
        let mut found: Vec<_> =
            self.channels.iter().filter(|(_, c)| c.permanent && !c.silent && !c.staff_only).collect();
        found.sort_by_key(|(id, _)| **id);
        found.into_iter().map(|(_, c)| (c.name.clone(), c.members.len())).collect()
    }

    /// Games anyone may see.
    pub fn public_games(&self) -> Vec<(Tag, Vec<u8>, usize)> {
        let open = self.games.values().filter(|game| !game.private && game.status == GameStatus::Open);
        open.map(|game| {
            let players = game.players.iter().filter(|player| player.session.is_some()).count();
            (game.product, game.name.clone(), players)
        })
        .collect()
    }

    /// The channels a client is offered.
    pub fn channel_list(&self, id: SessionId) -> Vec<Vec<u8>> {
        let Some(session) = self.sessions.get(&id) else { return Vec::new() };
        let mut found: Vec<_> = self
            .channels
            .iter()
            .filter(|(_, c)| c.permanent && !c.silent)
            .filter(|(_, c)| c.products.is_empty() || c.products.contains(&session.product))
            .filter(|(_, c)| !c.staff_only || session.role() >= Role::Moderator)
            .collect();
        found.sort_by_key(|(id, _)| **id);
        found.into_iter().map(|(_, channel)| channel.name.clone()).collect()
    }

    /// The people in a channel, or why they can't be shown.
    pub fn channel_members(&self, id: SessionId, name: &[u8]) -> Result<Vec<Member>, &'static str> {
        let by_alias = |c: &&Channel| c.alias.as_ref().is_some_and(|a| a.as_bytes().eq_ignore_ascii_case(name));
        let channel = match self.find_channel(name) {
            Some(found) => &self.channels[&found],
            None => self.channels.values().find(by_alias).ok_or("That channel does not exist.")?,
        };
        let asker = self.sessions.get(&id);
        if asker.is_some_and(|s| channel.banned.contains(&s.name().to_ascii_lowercase())) {
            return Err("You are banned from that channel.");
        }
        if channel.staff_only && asker.is_none_or(|s| s.role() < Role::Moderator) {
            return Err("That channel does not exist.");
        }
        let members = channel.members.iter().filter_map(|member| self.sessions.get(member));
        Ok(members.map(|s| Member { name: s.name().into(), flags: s.flags, product: s.product }).collect())
    }

    pub fn is_operator(&self, id: SessionId) -> bool {
        self.sessions.get(&id).is_some_and(|s| s.role() >= Role::Moderator)
            || self.channel_of(id).is_some_and(|channel| channel.operators.contains(&id))
    }

    /// Leaves the channel, showing the session its own departure.
    pub fn part(&mut self, id: SessionId) {
        if let Some(session) = self.sessions.get(&id)
            && session.channel.is_some()
        {
            session.send(Event { text: Vec::new(), ..about(EventKind::Leave, session) });
        }
        self.leave_channel(id);
    }

    /// Sends a session out of its channel into The Void.
    pub fn eject(&mut self, id: SessionId) {
        self.part(id);
        self.join_channel(id, VOID.as_bytes());
    }

    /// Gives or takes channel operator status within the asker's channel.
    pub fn set_operator(&mut self, asker: SessionId, target: SessionId, operator: bool) -> bool {
        let Some(channel_id) = self.sessions.get(&asker).and_then(|s| s.channel) else { return false };
        let Some(channel) = self.channels.get_mut(&channel_id) else { return false };
        if !channel.members.contains(&target) {
            return false;
        }
        if operator {
            channel.operators.insert(target);
        } else {
            channel.operators.remove(&target);
        }
        self.refresh_flags(target);
        true
    }

    pub fn set_banned(&mut self, asker: SessionId, name: &str, banned: bool) -> bool {
        let Some(channel_id) = self.sessions.get(&asker).and_then(|s| s.channel) else { return false };
        let Some(channel) = self.channels.get_mut(&channel_id) else { return false };
        let key = name.to_ascii_lowercase();
        if banned { channel.banned.insert(key) } else { channel.banned.remove(&key) }
    }

    /// Sends an event to everyone in a session's channel, including the session.
    pub fn announce(&self, id: SessionId, event: &Event) {
        if let Some(channel) = self.channel_of(id) {
            for member in &channel.members {
                self.send(*member, event.clone());
            }
        }
    }

    fn announce_flags(&self, id: SessionId) {
        if let Some(session) = self.sessions.get(&id)
            && self.channel_of(id).is_some_and(|channel| !channel.silent)
        {
            self.announce(id, &about(EventKind::UserFlags, session));
        }
    }

    fn refresh_flags(&mut self, id: SessionId) {
        let operator = self.channel_of(id).is_some_and(|channel| channel.operators.contains(&id));
        let Some(session) = self.sessions.get_mut(&id) else { return };
        if session.channel.is_none() {
            return;
        }
        let standing = match session.role() {
            Role::Admin => user_flags::ADMIN,
            Role::Moderator => user_flags::MODERATOR,
            Role::Player if operator => user_flags::CHANNEL_OPERATOR,
            Role::Player => 0,
        };
        let flags = standing | (session.flags & user_flags::NO_UDP);
        if session.flags != flags {
            session.flags = flags;
            self.announce_flags(id);
        }
    }

    /// Says something in the session's channel.
    pub fn talk(&mut self, id: SessionId, kind: EventKind, text: &[u8]) {
        let Some(session) = self.sessions.get(&id) else { return };
        let Some(channel) = self.channel_of(id) else { return };
        if channel.silent {
            return;
        }
        if session.account.as_ref().is_some_and(|account| account.muted) {
            return session.send(Event::error("You can't talk on the channel. Your account has been muted"));
        }
        if channel.moderated && !self.is_operator(id) {
            return session.send(Event::error("This channel is moderated"));
        }
        let mut event = about(kind, session);
        event.text = text.to_vec();
        let mut heard = false;
        for member in channel.members.iter().filter_map(|member| self.sessions.get(member)) {
            // An emote is shown to its sender too; talk isn't, because the client echoes it.
            if (member.id == id && kind == EventKind::Talk) || member.ignores(session.name()) {
                continue;
            }
            heard |= member.id != id;
            member.send(event.clone());
        }
        if !heard {
            session.send(Event::info("No one hears you."));
        }
    }

    pub fn whisper(&mut self, id: SessionId, to: &str, text: &[u8]) {
        let Some(sender) = self.sessions.get(&id) else { return };
        if sender.account.as_ref().is_some_and(|account| account.muted) {
            let text = "Your account has been muted, you can't whisper to other users.";
            return sender.send(Event::error(text));
        }
        let Some(target) = self.find_user(to) else {
            return sender.send(Event::error("That user is not logged on."));
        };
        let notice = |state: &str, why: &[u8]| {
            let mut text = format!("{} is {state} (", target.name()).into_bytes();
            text.extend_from_slice(why);
            text.push(b')');
            Event::info(text)
        };
        if let Some(why) = &target.busy {
            return sender.send(notice("unavailable", why));
        }
        sender.send(Event { text: text.to_vec(), ..about(EventKind::WhisperSent, target) });
        if let Some(why) = &target.away {
            sender.send(notice("away", why));
        }
        if target.ignores(sender.name()) {
            return;
        }
        target.send(Event { text: text.to_vec(), ..about(EventKind::Whisper, sender) });
        let (target, from) = (target.id, sender.name().to_owned());
        if let Some(target) = self.sessions.get_mut(&target) {
            target.last_whisper_from = Some(from);
        }
    }

    /// Counts a line against the flood limit. False means it must not be delivered.
    pub fn allow_line(&mut self, id: SessionId, text: &[u8]) -> bool {
        const WRAP: usize = 40;
        let chat = &self.config.chat;
        let Some(session) = self.sessions.get_mut(&id) else { return false };
        if text.len() > chat.max_line {
            session.send(Event::error("Your line length quota has been exceeded!"));
            return false;
        }
        let window = Duration::from_secs(chat.flood_secs);
        while session.recent_lines.front().is_some_and(|(time, _)| time.elapsed() >= window) {
            session.recent_lines.pop_front();
        }
        session.recent_lines.push_back((Instant::now(), text.len().div_ceil(WRAP).max(1)));
        let lines: usize = session.recent_lines.iter().map(|(_, count)| count).sum();
        if lines < chat.flood_lines {
            return true;
        }
        session.send(Event::error("Your message quota has been exceeded!"));
        if lines >= chat.flood_disconnect_lines {
            session.outbox.close.cancel();
        }
        false
    }

    // Games

    fn find_game(&self, name: &[u8], product: Tag, joinable: bool) -> Option<GameId> {
        let mut ids: Vec<_> = self
            .games
            .iter()
            .filter(|(_, g)| g.product == product && g.name.eq_ignore_ascii_case(name))
            .filter(|(_, g)| !joinable || g.joinable())
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids.first().copied()
    }

    /// Changes the status of the session's game, or creates one. False means it has none afterwards.
    pub fn offer_game(&mut self, id: SessionId, offer: &GameOffer<'_>) -> bool {
        let Some(session) = self.sessions.get(&id) else { return false };
        if let Some(game) = session.game.and_then(|game| self.games.get_mut(&game)) {
            set_status(game, offer.status);
            return true;
        }
        if offer.status == GameStatus::Done || self.find_game(offer.name, session.product, true).is_some() {
            return false;
        }
        let game = Game {
            name: offer.name.to_vec(),
            password: offer.password.to_vec(),
            info: offer.info.to_vec(),
            product: session.product,
            version_tag: session.version_tag.clone(),
            kind: offer.kind,
            status: offer.status,
            private: offer.private || !offer.password.is_empty(),
            disconnect_is_loss: offer.disconnect_is_loss,
            started_at: 0,
            address: session.game_address,
            players: Vec::new(),
        };
        if ladder_of(game.product, game.kind) == 1 && !may_play_ladder(session) {
            return false;
        }
        let game_id = self.next_id();
        self.games.insert(game_id, game);
        self.add_player(id, game_id);
        true
    }

    /// Notes that a player went into a game. The players then talk to each other directly.
    pub fn join_game(&mut self, id: SessionId, name: &[u8], password: &[u8]) {
        self.leave_channel(id);
        let Some(session) = self.sessions.get(&id) else { return };
        let Some(game_id) = self.find_game(name, session.product, true) else { return };
        let game = &self.games[&game_id];
        if session.game == Some(game_id)
            || (ladder_of(game.product, game.kind) == 1 && !may_play_ladder(session))
            || (!game.password.is_empty() && !game.password.eq_ignore_ascii_case(password))
        {
            return;
        }
        self.leave_game(id);
        self.add_player(id, game_id);
    }

    fn add_player(&mut self, id: SessionId, game_id: GameId) {
        let (Some(session), Some(game)) = (self.sessions.get_mut(&id), self.games.get_mut(&game_id)) else {
            return;
        };
        let Some(account) = &session.account else { return };
        session.game = Some(game_id);
        let ladder = ladder_of(game.product, game.kind) > 0;
        let hosting = game.players.is_empty();
        match game.players.iter_mut().find(|player| player.account == account.id) {
            Some(player) => player.session = Some(id),
            None => game.players.push(Player {
                account: account.id,
                name: account.name.clone(),
                session: Some(id),
                reports: None,
            }),
        }
        if ladder {
            session.send(Event::info(if hosting { "Created ladder game" } else { "Joined ladder game" }));
        }
    }

    pub fn leave_game(&mut self, id: SessionId) {
        let Some(game_id) = self.sessions.get_mut(&id).and_then(|s| s.game.take()) else { return };
        let Some(game) = self.games.get_mut(&game_id) else { return };
        for player in game.players.iter_mut().filter(|player| player.session == Some(id)) {
            player.session = None;
        }
        if let Some(host) = game.players.iter().find_map(|player| player.session) {
            if let Some(host) = self.sessions.get(&host) {
                game.address = host.game_address;
            }
            return;
        }
        if let Some(game) = self.games.remove(&game_id)
            && let Some(finished) = self.judge(&game)
            && let Some(recorder) = &self.finished
        {
            // Fails only if the recorder has stopped.
            let _ = recorder.send(finished);
        }
    }

    /// Decides each player's result by majority of the reports. A game nobody reported doesn't count.
    fn judge(&self, game: &Game) -> Option<FinishedGame> {
        let mut reports = Vec::new();
        for player in &game.players {
            for (subject, outcome) in player.reports.iter().flatten() {
                reports.push((player.account, *subject, *outcome));
            }
        }
        let mut results = Vec::new();
        let counts_as_loss = self.config.games.disconnect_is_loss || game.disconnect_is_loss;
        for player in &game.players {
            let votes = |wanted| reports.iter().filter(|(_, s, o)| *s == player.account && *o == wanted).count();
            let (wins, losses) = (votes(Outcome::Win), votes(Outcome::Loss));
            let (draws, drops) = (votes(Outcome::Draw), votes(Outcome::Disconnect));
            if wins + losses + draws + drops == 0 {
                continue;
            }
            // Ties go to the worse result, as on PvPGN.
            let outcome = if drops >= draws && drops >= losses && drops >= wins {
                if counts_as_loss { Outcome::Loss } else { Outcome::Disconnect }
            } else if losses >= wins && losses >= draws {
                Outcome::Loss
            } else if draws >= wins {
                Outcome::Draw
            } else {
                Outcome::Win
            };
            results.push((player.account, outcome));
        }
        let count = |wanted| results.iter().filter(|(_, outcome)| *outcome == wanted).count();
        let (wins, losses, draws) = (count(Outcome::Win), count(Outcome::Loss), count(Outcome::Draw));
        let ladder = ladder_of(game.product, game.kind);
        let uneven = wins < 1 || losses < 1 || (wins > 1 && wins != losses);
        if results.is_empty()
            || (draws > 0 && draws != results.len())
            || (ladder > 0 && counts_as_loss && draws == 0 && uneven)
        {
            tracing::debug!(wins, losses, draws, "game not counted");
            return None;
        }
        Some(FinishedGame {
            product: game.product,
            name: String::from_utf8_lossy(&game.name).replace('\0', ""),
            game_type: game.kind,
            ladder,
            started_at: if game.started_at == 0 { unix_now() } else { game.started_at },
            results,
            reports,
        })
    }

    /// Records what a player says the results were, then takes them out of the game. Players aren't obliged
    /// to tell the truth, so their reports are only counted, and only about people who were in the game.
    pub fn report_game(&mut self, id: SessionId, claims: &[(&str, Option<Outcome>)]) {
        let Some(session) = self.sessions.get(&id) else { return };
        let Some(reporter) = session.account.as_ref().map(|account| account.id) else { return };
        if let Some(game) = session.game.and_then(|game| self.games.get_mut(&game)) {
            let mut reports: Vec<(AccountId, Outcome)> = Vec::new();
            for (name, outcome) in claims {
                let subject = game.players.iter().find(|p| p.name.eq_ignore_ascii_case(name)).map(|p| p.account);
                if let (Some(subject), Some(outcome)) = (subject, outcome)
                    && reports.iter().all(|(known, _)| *known != subject)
                {
                    reports.push((subject, *outcome));
                }
            }
            if let Some(player) = game.players.iter_mut().find(|player| player.account == reporter)
                && player.reports.is_none()
            {
                player.reports = Some(reports);
            }
        }
        self.leave_game(id);
    }

    /// Marks a game started after its map was checked. None means there is no game; true, a ladder game.
    pub fn start_checked_game(&mut self, id: SessionId) -> Option<bool> {
        let game = self.games.get_mut(&self.sessions.get(&id)?.game?)?;
        set_status(game, GameStatus::Started);
        Some(ladder_of(game.product, game.kind) == 1)
    }

    fn listing(&self, viewer: &Session, game: &Game) -> Listing {
        Listing {
            kind: game.kind,
            address: self.config.network.host_address(viewer.ip, game.address),
            status: game.status,
            name: game.name.clone(),
            password: game.password.clone(),
            info: game.info.clone(),
        }
    }

    pub fn look_up_game(&self, id: SessionId, name: &[u8], password: &[u8]) -> Lookup {
        let Some(session) = self.sessions.get(&id) else { return Lookup::NotFound };
        let Some(game) = self.find_game(name, session.product, false).and_then(|id| self.games.get(&id)) else {
            return Lookup::NotFound;
        };
        match game.status {
            GameStatus::Started => Lookup::Started,
            GameStatus::Full => Lookup::Full,
            GameStatus::Done => Lookup::NotFound,
            GameStatus::Open if game.password != password => Lookup::WrongPassword,
            GameStatus::Open => Lookup::Found(self.listing(session, game)),
        }
    }

    /// Games a session may be shown. `kind` 0 means any.
    pub fn list_games(&self, id: SessionId, kind: u16) -> Vec<Listing> {
        let Some(session) = self.sessions.get(&id) else { return Vec::new() };
        let rules = &self.config.games;
        // In the Diablo games, the kind is the host's difficulty or level.
        let any_kind = kind == 0
            || matches!(
                session.product,
                product::DIABLO | product::DIABLO_SHAREWARE | product::DIABLO2 | product::DIABLO2_EXPANSION
            );
        let compatible = |game: &Game| match (&game.version_tag, &session.version_tag) {
            (Some(theirs), Some(ours)) => theirs == ours,
            _ => true,
        };
        let mut found: Vec<_> = self
            .games
            .iter()
            .filter(|(_, g)| g.product == session.product && (any_kind || g.kind == kind))
            .filter(|(_, g)| !(rules.hide_private && g.private))
            .filter(|(_, g)| !rules.hide_started || g.status == GameStatus::Open)
            .filter(|(_, g)| compatible(g))
            .collect();
        found.sort_by_key(|(id, _)| std::cmp::Reverse(**id));
        found.into_iter().map(|(_, game)| self.listing(session, game)).collect()
    }

    /// Where a friend is. None means offline.
    pub fn presence(&self, account: AccountId) -> Option<Presence> {
        let session = self.sessions.get(self.by_account.get(&account)?)?;
        let (location, place) = match (self.game_of(session), self.channel_of(session.id)) {
            (Some((name, false)), _) => (3, name.to_vec()),
            (Some((name, true)), _) => (5, name.to_vec()),
            (None, Some(channel)) => (2, channel.name.clone()),
            (None, None) => (1, Vec::new()),
        };
        let status = if session.busy.is_some() { 2 } else { 0 } | if session.away.is_some() { 4 } else { 0 };
        Some(Presence { status, location, product: session.product, place })
    }

    /// The name of a session's game, and whether it is private.
    pub fn game_of(&self, session: &Session) -> Option<(&[u8], bool)> {
        let game = self.games.get(&session.game?)?;
        Some((&game.name, game.private))
    }
}

fn topic_event(channel: &Channel) -> Event {
    let mut text = channel.name.clone();
    text.extend_from_slice(b" topic: ");
    text.extend_from_slice(if channel.topic.is_empty() { b"no topic" } else { &channel.topic });
    Event::info(text)
}

fn may_play_ladder(session: &Session) -> bool {
    session.records.get(&0).is_some_and(|record| record.wins >= LADDER_WINS_NEEDED)
}

fn set_status(game: &mut Game, status: GameStatus) {
    let reopening = matches!(status, GameStatus::Open | GameStatus::Full);
    if (game.status == GameStatus::Started && reopening) || game.status == GameStatus::Done {
        return;
    }
    if status == GameStatus::Started && game.started_at == 0 {
        game.started_at = unix_now();
    }
    game.status = status;
}

fn about(kind: EventKind, session: &Session) -> Event {
    Event {
        kind,
        flags: session.flags,
        latency: session.latency,
        name: session.name().into(),
        product: session.product,
        text: session.statstring(),
    }
}
