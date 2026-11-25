//! IRC, for chat only. A person is in one channel at a time, as in the games.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use parking_lot::Mutex;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::auth::{self, LoginError, SignupError};
use crate::command;
use crate::hub::{Event, EventKind, Member, Out, Outbox, SessionId, user_flags};
use crate::net::{Lines, Link, one_line};
use crate::server::Server;
use crate::wire::{Tag, product};
use crate::xsha1::Hash;

const HOST: &str = "openbnet";
/// The protocol's limit is 512 bytes with the prefix, which is at most about 100.
const MAX_LINE: usize = 1024;
const MAX_TEXT: usize = 400;

pub async fn serve<S>(server: Arc<Server>, stream: S, ip: IpAddr) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (reader, writer) = tokio::io::split(stream);
    let nick = Arc::new(Mutex::new(String::from("*")));
    let mut view = View { nick: nick.clone(), channel: None, shown: HashMap::new() };
    let link = Link::open(&server, ip, product::IRC, writer, Box::new(move |event| view.lines(event)))?;
    let mut connection = Connection {
        server: server.clone(),
        id: link.id,
        ip,
        outbox: link.outbox.clone(),
        nick,
        has_user: false,
        password: None,
        welcomed: false,
        logged_in: false,
    };
    let reading = async move { connection.read(Lines::new(reader, MAX_LINE)).await };
    link.run(reading).await
}

/// What the client has been shown, so that changes can be told apart.
struct View {
    nick: Arc<Mutex<String>>,
    channel: Option<String>,
    shown: HashMap<String, &'static str>,
}

/// The sign before a name.
fn rank(flags: u32) -> &'static str {
    if flags & user_flags::ADMIN != 0 {
        "@"
    } else if flags & (user_flags::MODERATOR | user_flags::CHANNEL_OPERATOR) != 0 {
        "%"
    } else {
        ""
    }
}

fn mode(rank: &str) -> char {
    if rank == "@" { 'o' } else { 'h' }
}

fn from(name: &str, product: Tag) -> String {
    format!(":{name}!{product}@{HOST}")
}

fn line(head: String, text: &[u8]) -> Vec<u8> {
    let text = one_line(text);
    let mut line = head.into_bytes();
    line.extend_from_slice(b" :");
    line.extend_from_slice(&text[..text.len().min(MAX_TEXT)]);
    line.extend_from_slice(b"\r\n");
    line
}

impl View {
    fn lines(&mut self, event: &Event) -> Vec<u8> {
        let nick = self.nick.lock().clone();
        let channel = self.channel.clone().unwrap_or_else(|| "*".into());
        let sender = from(&event.name, event.product);
        match event.kind {
            EventKind::Channel => {
                let new = to_irc(&event.text);
                let mut out = Vec::new();
                if let Some(old) = self.channel.replace(new.clone()) {
                    out.extend(format!("{sender} PART {old}\r\n").bytes());
                }
                self.shown.clear();
                out.extend(format!("{sender} JOIN {new}\r\n").bytes());
                out
            }
            EventKind::ShowUser => {
                let rank = rank(event.flags);
                self.shown.insert(event.name.clone(), rank);
                format!(":{HOST} 353 {nick} = {channel} :{rank}{}\r\n", event.name).into_bytes()
            }
            EventKind::Entered => format!(":{HOST} 366 {nick} {channel} :End of NAMES list\r\n").into_bytes(),
            EventKind::Join => {
                let rank = rank(event.flags);
                self.shown.insert(event.name.clone(), rank);
                let mut out = format!("{sender} JOIN {channel}\r\n");
                if !rank.is_empty() {
                    out += &format!(":{HOST} MODE {channel} +{} {}\r\n", mode(rank), event.name);
                }
                out.into_bytes()
            }
            EventKind::Leave => {
                self.shown.remove(&event.name);
                if event.name == nick {
                    self.channel = None;
                }
                format!("{sender} PART {channel}\r\n").into_bytes()
            }
            EventKind::UserFlags => {
                let new = rank(event.flags);
                let old = self.shown.insert(event.name.clone(), new).unwrap_or("");
                let mut out = String::new();
                if old != new {
                    if !old.is_empty() {
                        out += &format!(":{HOST} MODE {channel} -{} {}\r\n", mode(old), event.name);
                    }
                    if !new.is_empty() {
                        out += &format!(":{HOST} MODE {channel} +{} {}\r\n", mode(new), event.name);
                    }
                }
                out.into_bytes()
            }
            EventKind::Talk => line(format!("{sender} PRIVMSG {channel}"), &event.text),
            // IRC clients show their own actions.
            EventKind::Emote if event.name == nick => Vec::new(),
            EventKind::Emote => {
                let mut text = b"\x01ACTION ".to_vec();
                text.extend_from_slice(&event.text[..event.text.len().min(MAX_TEXT - 10)]);
                text.push(1);
                line(format!("{sender} PRIVMSG {channel}"), &text)
            }
            EventKind::Whisper => line(format!("{sender} PRIVMSG {nick}"), &event.text),
            EventKind::WhisperSent => Vec::new(),
            EventKind::Info | EventKind::Error | EventKind::Broadcast => {
                line(format!(":{HOST} NOTICE {nick}"), &event.text)
            }
        }
    }
}

struct Connection {
    server: Arc<Server>,
    id: SessionId,
    ip: IpAddr,
    outbox: Outbox,
    nick: Arc<Mutex<String>>,
    has_user: bool,
    password: Option<Hash>,
    welcomed: bool,
    logged_in: bool,
}

/// A line's command, its parameters, and its last parameter's bytes.
struct Message {
    command: String,
    params: Vec<Vec<u8>>,
}

impl Message {
    fn parse(line: &[u8]) -> Message {
        let mut rest = line;
        if rest.first() == Some(&b':') {
            rest = split_word(rest).1;
        }
        let (command, mut rest) = split_word(rest);
        let mut params = Vec::new();
        while !rest.is_empty() {
            if rest[0] == b':' {
                params.push(rest[1..].to_vec());
                break;
            }
            let (word, after) = split_word(rest);
            params.push(word.to_vec());
            rest = after;
        }
        Message { command: String::from_utf8_lossy(command).to_ascii_uppercase(), params }
    }

    fn text(&self, index: usize) -> String {
        self.params.get(index).map(|param| String::from_utf8_lossy(param).into_owned()).unwrap_or_default()
    }

    fn bytes(&self, index: usize) -> &[u8] {
        self.params.get(index).map_or(&[], Vec::as_slice)
    }
}

fn split_word(text: &[u8]) -> (&[u8], &[u8]) {
    let end = text.iter().position(|b| *b == b' ').unwrap_or(text.len());
    let rest = &text[end..];
    (&text[..end], &rest[rest.iter().take_while(|b| **b == b' ').count()..])
}

impl Connection {
    async fn read<R: AsyncRead + Unpin>(&mut self, mut lines: Lines<R>) -> Result<()> {
        let settings = self.server.config();
        let deadline = tokio::time::sleep(Duration::from_secs(settings.server.login_timeout_secs));
        tokio::pin!(deadline);
        let interval = Duration::from_secs(settings.server.ping_interval_secs.max(1));
        let mut pings = tokio::time::interval_at(tokio::time::Instant::now() + interval, interval);
        let mut silent_intervals = 0;
        loop {
            tokio::select! {
                () = self.outbox.close.cancelled() => return Ok(()),
                line = lines.next() => {
                    let Some(line) = line? else { return Ok(()) };
                    silent_intervals = 0;
                    if !self.handle(&Message::parse(&line)).await? {
                        return Ok(());
                    }
                }
                () = &mut deadline, if !self.logged_in => anyhow::bail!("did not log in in time"),
                _ = pings.tick() => {
                    silent_intervals += 1;
                    anyhow::ensure!(silent_intervals < 4, "stopped responding");
                    self.send(format!("PING :{}", self.server.ticks()));
                }
            }
        }
    }

    fn send(&self, line: String) {
        self.outbox.send(Out::Raw([&one_line(line.as_bytes())[..], b"\r\n"].concat()));
    }

    fn reply(&self, code: u16, text: impl AsRef<str>) {
        let nick = self.nick.lock().clone();
        self.send(format!(":{HOST} {code:03} {nick} {}", text.as_ref()));
    }

    fn notice(&self, text: &str) {
        let nick = self.nick.lock().clone();
        self.send(format!(":NickServ!NickServ@{HOST} NOTICE {nick} :{text}"));
    }

    /// False ends the connection.
    async fn handle(&mut self, message: &Message) -> Result<bool> {
        match message.command.as_str() {
            "PING" => self.send(format!(":{HOST} PONG {HOST} :{}", message.text(0))),
            "PONG" => {
                let now = self.server.ticks();
                if let Ok(then) = message.text(0).parse::<u32>()
                    && then <= now
                {
                    self.server.hub.lock().set_latency(self.id, now - then);
                }
            }
            "CAP" => {
                if message.text(0).eq_ignore_ascii_case("LS") {
                    self.send(format!(":{HOST} CAP * LS :"));
                }
            }
            "QUIT" => return Ok(false),
            "PASS" if !self.welcomed => self.password = Some(Hash::of_password(&message.text(0))),
            "NICK" => self.set_nick(&message.text(0)).await?,
            "USER" if !self.has_user => {
                self.has_user = true;
                self.welcome().await?;
            }
            "PRIVMSG" | "NOTICE" if message.params.len() < 2 => self.reply(412, ":No text to send"),
            "PRIVMSG" if message.text(0).eq_ignore_ascii_case("NickServ") => {
                self.nickserv(&message.text(1)).await?;
            }
            _ if !self.logged_in => self.reply(451, ":Log in first: /msg NickServ identify <password>"),
            "PRIVMSG" => self.message(message).await?,
            // Nobody may be answered automatically by the server either.
            "NOTICE" => {
                if !message.bytes(0).starts_with(b"#") {
                    self.server.hub.lock().whisper(self.id, &message.text(0), message.bytes(1));
                }
            }
            "JOIN" => self.join(message),
            "PART" => self.server.hub.lock().part(self.id),
            "NAMES" => {
                let channel = message.text(0);
                if let Ok(members) = self.members(&channel) {
                    for member in members {
                        self.reply(353, format!("= {channel} :{}{}", rank(member.flags), member.name));
                    }
                }
                self.reply(366, format!("{channel} :End of NAMES list"));
            }
            "WHO" => {
                let channel = message.text(0);
                let nick = self.nick.lock().clone();
                if let Ok(members) = self.members(&channel) {
                    for m in members {
                        let rank = rank(m.flags);
                        self.reply(
                            352,
                            format!("{channel} {} {HOST} {HOST} {} H{rank} :0 {nick}", m.product, m.name),
                        );
                    }
                }
                self.reply(315, format!("{channel} :End of WHO list"));
            }
            "LIST" => {
                self.reply(321, "Channel :Users Name");
                let channels = self.server.hub.lock().public_channels();
                for (name, count) in channels {
                    self.reply(322, format!("{} {count} :", to_irc(&name)));
                }
                self.reply(323, ":End of LIST");
            }
            "TOPIC" if message.params.len() > 1 => {
                let text = [b"/topic ", message.bytes(1)].concat();
                command::run(&self.server, self.id, &text).await?;
            }
            "TOPIC" => {
                let name = from_irc(message.bytes(0)).unwrap_or_default();
                let topic = self.server.hub.lock().topic(&name).map(<[u8]>::to_vec);
                match topic {
                    None => self.reply(331, format!("{} :No topic is set", message.text(0))),
                    Some(topic) => {
                        self.reply(332, format!("{} :{}", message.text(0), String::from_utf8_lossy(&topic)));
                    }
                }
            }
            "MODE" => {
                if message.params.len() == 1 && message.bytes(0).starts_with(b"#") {
                    self.reply(324, format!("{} +", message.text(0)));
                }
            }
            "WHOIS" => self.whois(&message.text(message.params.len().saturating_sub(1))),
            "ISON" => {
                let hub = self.server.hub.lock();
                let names = message.params.iter().flat_map(|param| param.split(|b| *b == b' '));
                let online: Vec<String> = names
                    .filter_map(|name| hub.find_user(&String::from_utf8_lossy(name)).map(|s| s.name().to_owned()))
                    .collect();
                drop(hub);
                self.reply(303, format!(":{}", online.join(" ")));
            }
            "USERHOST" => self.reply(302, ":"),
            "KICK" => {
                let text = format!("/kick {} {}", message.text(1), message.text(2));
                command::run(&self.server, self.id, text.as_bytes()).await?;
            }
            "AWAY" => {
                let text = format!("/away {}", message.text(0));
                command::run(&self.server, self.id, text.trim_end().as_bytes()).await?;
            }
            "TIME" => command::run(&self.server, self.id, b"/time").await?,
            other => self.reply(421, format!("{other} :Unknown command")),
        }
        Ok(true)
    }

    async fn set_nick(&mut self, name: &str) -> Result<()> {
        let symbols = self.server.config().accounts.symbols.clone();
        if self.logged_in {
            self.reply(433, format!("{name} :Your name is your account's and can't be changed"));
        } else if auth::valid_name(name.as_bytes(), &symbols).is_none() {
            self.reply(432, format!("{name} :Not a valid account name"));
        } else {
            *self.nick.lock() = name.to_owned();
            self.welcome().await?;
        }
        Ok(())
    }

    async fn welcome(&mut self) -> Result<()> {
        let nick = self.nick.lock().clone();
        if self.welcomed || !self.has_user || nick == "*" {
            return Ok(());
        }
        self.welcomed = true;
        let config = self.server.config();
        let network = config.server.name.replace(' ', "_");
        let version = env!("CARGO_PKG_VERSION");
        self.reply(1, format!(":Welcome to {}, {nick}", config.server.name));
        self.reply(2, format!(":Your host is {HOST}, running version {version}"));
        self.reply(3, ":This server is part of a game server");
        self.reply(4, format!("{HOST} {version} - -"));
        self.reply(
            5,
            format!(
                "NICKLEN={} CHANNELLEN=31 PREFIX=(oh)@% CHANTYPES=# CHANLIMIT=#:1 NETWORK={network} :are supported",
                auth::MAX_NAME
            ),
        );
        self.reply(375, ":- Message of the day");
        for line in &config.chat.welcome {
            self.reply(372, format!(":- {line}"));
        }
        self.reply(376, ":End of MOTD");
        match self.password.take() {
            Some(password) => self.identify(password).await?,
            None => self.notice("Log in with /msg NickServ identify <password>"),
        }
        Ok(())
    }

    async fn nickserv(&mut self, text: &str) -> Result<()> {
        let (word, password) = text.split_once(' ').unwrap_or((text, ""));
        let password = password.trim();
        if self.logged_in {
            self.notice("You are already logged in.");
        } else if !self.welcomed {
            self.notice("Send NICK and USER first.");
        } else if password.is_empty() || password.len() > auth::MAX_PASSWORD {
            self.notice("Use: identify <password>, or register <password>");
        } else if word.eq_ignore_ascii_case("identify") {
            self.identify(Hash::of_password(password)).await?;
        } else if word.eq_ignore_ascii_case("register") {
            let nick = self.nick.lock().clone();
            let hash = Hash::of_password(password);
            match auth::sign_up(&self.server, self.ip, nick.as_bytes(), hash).await? {
                Ok(()) => self.identify(hash).await?,
                Err(SignupError::NameTaken) => self.notice("That name is taken."),
                Err(SignupError::InvalidName) => self.notice("That name isn't allowed."),
                Err(SignupError::NotAllowed) => self.notice("New accounts aren't allowed right now."),
            }
        } else {
            self.notice("Use: identify <password>, or register <password>");
        }
        Ok(())
    }

    async fn identify(&mut self, password: Hash) -> Result<()> {
        let nick = self.nick.lock().clone();
        match auth::check(&self.server, self.ip, &nick, |stored| stored.matches(&password)).await? {
            Ok(account) => {
                let name = account.name.clone();
                if auth::enter(&self.server, self.id, self.ip, account, product::IRC).await? {
                    if name != nick {
                        self.send(format!("{} NICK {name}", from(&nick, product::IRC)));
                        *self.nick.lock() = name;
                    }
                    self.logged_in = true;
                    self.notice("You are now logged in.");
                }
            }
            Err(LoginError::Locked) => self.notice("This account has been locked."),
            Err(_) => self.notice("Wrong name or password."),
        }
        Ok(())
    }

    fn here(&self) -> Option<String> {
        self.server.hub.lock().channel_name(self.id).map(to_irc)
    }

    fn members(&self, channel: &str) -> Result<Vec<Member>, &'static str> {
        let name = from_irc(channel.as_bytes()).ok_or("not a channel")?;
        self.server.hub.lock().channel_members(self.id, &name)
    }

    fn join(&self, message: &Message) {
        // Only one channel at a time, so of several the last would win anyway.
        let last = message.bytes(0).rsplit(|b| *b == b',').next().unwrap_or_default();
        match from_irc(last) {
            Some(name) if !name.is_empty() && name.len() <= 31 => {
                self.server.hub.lock().join_channel(self.id, &name);
            }
            _ => self.reply(403, format!("{} :No such channel", String::from_utf8_lossy(last))),
        }
    }

    async fn message(&self, message: &Message) -> Result<()> {
        let (target, text) = (message.bytes(0), message.bytes(1));
        if !target.starts_with(b"#") {
            self.server.hub.lock().whisper(self.id, &message.text(0), text);
            return Ok(());
        }
        let here = self.here();
        if !here.is_some_and(|here| here.as_bytes().eq_ignore_ascii_case(target)) {
            self.reply(404, format!("{} :You aren't in that channel", message.text(0)));
            return Ok(());
        }
        match text.strip_prefix(b"\x01ACTION ") {
            Some(action) => {
                let mut line = b"/me ".to_vec();
                line.extend_from_slice(action.strip_suffix(b"\x01").unwrap_or(action));
                command::run(&self.server, self.id, &line).await
            }
            // Other such requests, like asking for someone's version, aren't chat.
            None if text.first() == Some(&1) => Ok(()),
            None => command::say(&self.server, self.id, text).await,
        }
    }

    fn whois(&self, name: &str) {
        let hub = self.server.hub.lock();
        let found = hub.find_user(name).map(|session| {
            let channel = hub.channel_name(session.id).map(to_irc);
            (session.name().to_owned(), session.product, rank(session.flags), channel)
        });
        drop(hub);
        match found {
            None => self.reply(401, format!("{name} :No such nick")),
            Some((name, product, rank, channel)) => {
                self.reply(311, format!("{name} {product} {HOST} * :{}", product::title(product)));
                if let Some(channel) = channel {
                    self.reply(319, format!("{name} :{rank}{channel}"));
                }
            }
        }
        self.reply(318, format!("{name} :End of WHOIS list"));
    }
}

/// Channel names may hold characters IRC doesn't allow. The escapes are PvPGN's.
fn to_irc(name: &[u8]) -> String {
    let mut out = String::from("#");
    for c in String::from_utf8_lossy(name).chars() {
        match c {
            ' ' => out.push('_'),
            '_' => out.push_str("%_"),
            '%' => out.push_str("%%"),
            '\x08' => out.push_str("%b"),
            '\n' => out.push_str("%n"),
            '\r' => out.push_str("%r"),
            ':' => out.push_str("%="),
            ',' => out.push_str("%-"),
            '\0' | '\x07' => {}
            c => out.push(c),
        }
    }
    out
}

fn from_irc(name: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut bytes = name.strip_prefix(b"#")?.iter();
    while let Some(byte) = bytes.next() {
        out.push(match byte {
            b'_' => b' ',
            b'%' => match bytes.next()? {
                b'=' => b':',
                b'-' => b',',
                b'b' => 8,
                // Line breaks can be read but not made.
                b'n' | b'r' => return None,
                other => *other,
            },
            byte => *byte,
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_names_survive_the_trip() {
        for name in ["War2BNE-1", "The Void", "a_b%c", "x:y,z"] {
            assert_eq!(from_irc(to_irc(name.as_bytes()).as_bytes()).unwrap(), name.as_bytes());
        }
        assert_eq!(to_irc(b"a\r\nQUIT"), "#a%r%nQUIT");
        assert_eq!(from_irc(b"plain"), None);
        assert_eq!(from_irc(b"#end%"), None);
    }

    #[test]
    fn parses_lines() {
        let message = Message::parse(b":x PRIVMSG  #a :hello there");
        assert_eq!(message.command, "PRIVMSG");
        assert_eq!(message.params, [b"#a".to_vec(), b"hello there".to_vec()]);
        assert_eq!(Message::parse(b":").command, "");
    }
}
