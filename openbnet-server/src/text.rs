//! Bot and telnet chat: lines of text on the game port.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedReadHalf;
use tokio::time::Instant;

use crate::auth;
use crate::command;
use crate::hub::{Event, EventKind, Out, Outbox, SessionId};
use crate::net::{Lines, Link, one_line};
use crate::server::Server;
use crate::wire::product;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Numbered lines for programs.
    Bot,
    /// Plain lines for people.
    Telnet,
}

const MAX_LINE: usize = 255;
const CHANNEL: &[u8] = b"Chat";

pub async fn serve(server: Arc<Server>, stream: TcpStream, ip: IpAddr, style: Style) -> Result<()> {
    let (reader, writer) = stream.into_split();
    let encode = Box::new(move |event: &Event| match style {
        Style::Bot => bot_line(event),
        Style::Telnet => telnet_line(event),
    });
    let link = Link::open(&server, ip, product::CHAT, writer, encode)?;
    let connection = Connection { server: server.clone(), id: link.id, ip, outbox: link.outbox.clone(), style };
    link.run(connection.read(Lines::new(reader, MAX_LINE))).await
}

struct Connection {
    server: Arc<Server>,
    id: SessionId,
    ip: IpAddr,
    outbox: Outbox,
    style: Style,
}

impl Connection {
    fn send(&self, text: &str) {
        self.outbox.send(Out::Raw(text.as_bytes().to_vec()));
    }

    async fn read(self, mut lines: Lines<OwnedReadHalf>) -> Result<()> {
        let settings = self.server.config();
        let limit = Duration::from_secs(settings.server.login_timeout_secs);
        let deadline = Instant::now() + limit;
        if !self.log_in(&mut lines, deadline).await? {
            return Ok(());
        }
        let interval = Duration::from_secs(settings.server.ping_interval_secs.max(1));
        let mut ticks = tokio::time::interval_at(Instant::now() + interval, interval);
        loop {
            tokio::select! {
                () = self.outbox.close.cancelled() => return Ok(()),
                line = lines.next() => {
                    let Some(line) = line? else { return Ok(()) };
                    command::say(&self.server, self.id, &line).await?;
                }
                _ = ticks.tick(), if self.style == Style::Bot => self.send("2000 NULL\r\n"),
            }
        }
    }

    /// The next line before login. None means the connection is ending.
    async fn next(&self, lines: &mut Lines<OwnedReadHalf>, deadline: Instant) -> Result<Option<Vec<u8>>> {
        tokio::select! {
            () = self.outbox.close.cancelled() => Ok(None),
            line = tokio::time::timeout_at(deadline, lines.next()) => line?,
        }
    }

    /// Asks for a name and password until they are right. False means the connection ended.
    async fn log_in(&self, lines: &mut Lines<OwnedReadHalf>, deadline: Instant) -> Result<bool> {
        self.send("Username: ");
        loop {
            let Some(name) = self.next(lines, deadline).await? else { return Ok(false) };
            // Some bots send a control character first.
            let name = String::from_utf8_lossy(name.strip_prefix(b"\x04").unwrap_or(&name)).into_owned();
            if name.is_empty() {
                continue;
            }
            if self.style == Style::Bot {
                self.send(&name);
            }
            self.send("\r\nPassword: ");
            let Some(password) = self.next(lines, deadline).await? else { return Ok(false) };
            let password = String::from_utf8_lossy(&password).into_owned();
            let allowed = self.allows(&name);
            match auth::check_password(&self.server, self.ip, &name, &password).await? {
                Ok(account) if allowed => {
                    let name = account.name.clone();
                    if !auth::enter(&self.server, self.id, self.ip, account, product::CHAT).await? {
                        return Ok(false);
                    }
                    self.send("\r\n");
                    self.send(&match self.style {
                        Style::Bot => format!("2010 NAME {name}\r\n"),
                        Style::Telnet => format!("Your unique name: {name}\r\n"),
                    });
                    self.server.hub.lock().join_channel(self.id, CHANNEL);
                    return Ok(true);
                }
                Ok(_) => self.send("\r\nAccount has no bot access.\r\n\r\nUsername: "),
                Err(_) => self.send("\r\nLogin failed.\r\n\r\nUsername: "),
            }
        }
    }

    fn allows(&self, name: &str) -> bool {
        self.server.config().bots.accounts.iter().any(|listed| listed == "*" || listed.eq_ignore_ascii_case(name))
    }
}

fn bot_line(event: &Event) -> Vec<u8> {
    let Event { flags, name, product, .. } = event;
    let text = one_line(&event.text);
    let code = 1000 + event.kind as u32;
    let person = |word: &str| format!("{code} {word} {name} {flags:04x}").into_bytes();
    let with_text = |mut head: Vec<u8>| {
        head.extend_from_slice(b" \"");
        head.extend_from_slice(&text[..text.len().min(MAX_LINE)]);
        head.push(b'"');
        head
    };
    let with_product = |mut head: Vec<u8>| {
        head.extend(format!(" [{product}]").bytes());
        head
    };
    let mut line = match event.kind {
        EventKind::ShowUser => with_product(person("USER")),
        EventKind::Join => with_product(person("JOIN")),
        EventKind::Leave => person("LEAVE"),
        EventKind::UserFlags => person("USER"),
        EventKind::Whisper | EventKind::WhisperSent => with_text(person("WHISPER")),
        EventKind::Talk => with_text(person("TALK")),
        EventKind::Emote => with_text(person("EMOTE")),
        EventKind::Broadcast => with_text(format!("{code} _").into_bytes()),
        EventKind::Channel => with_text(format!("{code} CHANNEL").into_bytes()),
        EventKind::Info => with_text(format!("{code} INFO").into_bytes()),
        EventKind::Error => with_text(format!("{code} ERROR").into_bytes()),
        EventKind::Entered => return Vec::new(),
    };
    line.extend_from_slice(b"\r\n");
    line
}

fn telnet_line(event: &Event) -> Vec<u8> {
    let name = &event.name;
    // Control characters could drive a terminal.
    let text: Vec<u8> =
        one_line(&event.text).into_iter().map(|b| if b < 0x20 || b == 0x7f { b'?' } else { b }).collect();
    let (before, after) = match event.kind {
        EventKind::ShowUser => return format!("[{name} is here]\r\n").into_bytes(),
        EventKind::Join => return format!("[{name} enters]\r\n").into_bytes(),
        EventKind::Leave => return format!("[{name} leaves]\r\n").into_bytes(),
        EventKind::UserFlags | EventKind::Entered => return Vec::new(),
        EventKind::Whisper => (format!("<from {name}> "), ""),
        EventKind::WhisperSent => (format!("<to {name}> "), ""),
        EventKind::Talk => (format!("<{name}> "), ""),
        EventKind::Emote => (format!("<{name} "), ">"),
        EventKind::Broadcast => ("Broadcast: ".into(), ""),
        EventKind::Channel => ("Joining channel: \"".into(), "\""),
        EventKind::Info => (String::new(), ""),
        EventKind::Error => ("ERROR: ".into(), ""),
    };
    let mut line = before.into_bytes();
    line.extend(text);
    line.extend_from_slice(after.as_bytes());
    line.extend_from_slice(b"\r\n");
    line
}
