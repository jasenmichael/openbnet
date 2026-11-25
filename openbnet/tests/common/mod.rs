//! A simulated Warcraft II client.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use openbnet::config::Config;
use openbnet::server;
use openbnet::wire::{Reader, Tag, Writer, product};
use openbnet::xsha1::Hash;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub type Packet = (u8, Vec<u8>);

pub const CLIENT_TOKEN: u32 = 0x1234_5678;

pub struct Client {
    stream: TcpStream,
    pub token: u32,
}

pub struct Started {
    pub address: SocketAddr,
    pub irc: Option<SocketAddr>,
    pub irc_tls: Option<SocketAddr>,
    pub http: Option<SocketAddr>,
    pub server: Arc<server::Server>,
}

/// Starts a server on free ports with an empty database.
pub async fn start(change: impl FnOnce(&mut Config)) -> Started {
    let mut config = Config::default();
    config.server.listen = "127.0.0.1:0".parse().unwrap();
    config.database.url = "sqlite::memory:".into();
    config.http.listen = None;
    config.accounts.max_signups = 0;
    config.chat.flood_lines = 1000;
    config.chat.flood_disconnect_lines = 2000;
    change(&mut config);
    // A port that is free for TCP may be taken for UDP by another test.
    let mut attempt = 0;
    let running = loop {
        match server::start(config.clone(), None).await {
            Ok(running) => break running,
            Err(error) if attempt < 10 => {
                eprintln!("trying another port: {error:#}");
                attempt += 1;
            }
            Err(error) => panic!("{error:#}"),
        }
    };
    let started = Started {
        address: running.address,
        irc: running.irc.as_ref().map(|l| l.local_addr().unwrap()),
        irc_tls: running.irc_tls.as_ref().map(|(l, _)| l.local_addr().unwrap()),
        http: running.http.as_ref().map(|l| l.local_addr().unwrap()),
        server: running.server.clone(),
    };
    tokio::spawn(running.run());
    started
}

pub async fn server(change: impl FnOnce(&mut Config)) -> (SocketAddr, Arc<server::Server>) {
    let started = start(change).await;
    (started.address, started.server)
}

impl Client {
    pub async fn open(address: SocketAddr) -> Client {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(&[0x01]).await.unwrap();
        Client { stream, token: 0 }
    }

    /// Drops the connection at once, as a crash or a lost network would.
    pub fn reset(self) {
        #[allow(deprecated)] // blocks only for a non-zero time
        let _ = self.stream.set_linger(Some(Duration::ZERO));
    }

    /// Connects and passes the version check, as Warcraft II 2.02a does.
    pub async fn connect(address: SocketAddr) -> Client {
        let mut client = Client::open(address).await;
        client.send(0x1e, Writer::new().u32(1).u32(1).u32(0).u32(0).u32(0).cstr("HOST").cstr("USER")).await;
        client.expect(0x05).await;
        let token = client.expect(0x1d).await;
        client.token = u32::from_le_bytes(token[4..8].try_into().unwrap());
        let country = Writer::new().raw([0; 16]).u32(0).u32(1033).u32(1033).u32(1033);
        client.send(0x12, country.cstr("ENU").cstr("1").cstr("USA").cstr("United States")).await;
        client.send(0x06, identity().u32(0)).await;
        client.expect(0x06).await;
        client.send(0x07, identity().u32(0x0200_0200).u32(0xb52b_ad87).cstr("Warcraft II BNE.exe 1")).await;
        assert_eq!(client.expect(0x07).await[..4], [2, 0, 0, 0]);
        client
    }

    /// Connects the way StarCraft 1.16 and later games do.
    pub async fn connect_newer(address: SocketAddr, product: &[u8; 4], version_byte: u32) -> Client {
        let mut client = Client::open(address).await;
        let body = Writer::new().u32(0).u32(Tag::new(b"IX86").0).u32(Tag::new(product).0).u32(version_byte);
        client.send(0x50, body.u32(0).u32(0).u32(0).u32(1033).u32(1033).cstr("USA").cstr("United States")).await;
        let auth = client.expect(0x50).await;
        client.token = u32::from_le_bytes(auth[4..8].try_into().unwrap());
        client
            .send(0x51, Writer::new().u32(CLIENT_TOKEN).u32(1).u32(1).u32(0).u32(0).cstr("x.exe").cstr("Me"))
            .await;
        assert_eq!(client.expect(0x51).await[..4], [0; 4]);
        client
    }

    /// Plays a game of a kind between two players, with the results they agree on.
    pub async fn play(&mut self, other: &mut Client, name: &str, kind: u16, results: &[(&str, u32)]) {
        let start = |status: u32| Writer::new().u32(status).u32(0).u16(kind).u16(1).u32(0).u32(0);
        self.send(0x08, start(4).cstr(name).cstr("").cstr("info")).await;
        assert_eq!(self.expect(0x08).await[0], 1, "could not host");
        other.join_game(name, "").await;
        other.quiet().await;
        self.send(0x08, start(0x0e).cstr(name).cstr("").cstr("info")).await;
        self.report(results).await;
        self.quiet().await;
        other.report(results).await;
        other.quiet().await;
    }

    /// Connects, creates an account and logs in.
    pub async fn player(address: SocketAddr, name: &str) -> Client {
        let mut client = Client::connect(address).await;
        assert!(client.create(name, "secret").await);
        assert!(client.log_in(name, "secret").await);
        client
    }

    /// The above, then enters the Warcraft II channel.
    pub async fn chatter(address: SocketAddr, name: &str) -> Client {
        let mut client = Client::player(address, name).await;
        client.enter_chat().await;
        client.join("War2BNE").await;
        client.quiet().await;
        client
    }

    pub async fn send(&mut self, id: u8, body: Writer) {
        let body = body.finish();
        let mut data = vec![0xff, id];
        data.extend(((body.len() + 4) as u16).to_le_bytes());
        data.extend(body);
        self.stream.write_all(&data).await.unwrap();
    }

    pub async fn send_bytes(&mut self, data: &[u8]) {
        let _ = self.stream.write_all(data).await;
    }

    /// The next packet, or None if none comes or the connection closes.
    pub async fn receive(&mut self, wait: Duration) -> Option<Packet> {
        let mut header = [0; 4];
        tokio::time::timeout(wait, self.stream.read_exact(&mut header)).await.ok()?.ok()?;
        assert_eq!(header[0], 0xff);
        let mut body = vec![0; usize::from(u16::from_le_bytes([header[2], header[3]])) - 4];
        self.stream.read_exact(&mut body).await.ok()?;
        Some((header[1], body))
    }

    /// The next packet with an id, skipping others.
    pub async fn expect(&mut self, id: u8) -> Vec<u8> {
        loop {
            let (found, body) =
                self.receive(Duration::from_secs(5)).await.unwrap_or_else(|| panic!("no packet {id:#04x}"));
            if found == id {
                return body;
            }
        }
    }

    /// Everything that arrives until nothing has for a moment.
    pub async fn quiet(&mut self) -> Vec<Packet> {
        let mut packets = Vec::new();
        while let Some(packet) = self.receive(Duration::from_millis(250)).await {
            packets.push(packet);
        }
        packets
    }

    pub async fn closed(&mut self) -> bool {
        let mut byte = [0; 256];
        loop {
            match tokio::time::timeout(Duration::from_secs(5), self.stream.read(&mut byte)).await {
                Err(_) => return false,
                Ok(Ok(0) | Err(_)) => return true,
                Ok(Ok(_)) => {}
            }
        }
    }

    pub async fn create(&mut self, name: &str, password: &str) -> bool {
        self.send(0x2a, Writer::new().raw(Hash::of_password(password).to_wire()).cstr(name)).await;
        self.expect(0x2a).await[0] == 1
    }

    pub fn proof(&self, password: &str) -> Writer {
        let proof = Hash::of_password(password).proof(CLIENT_TOKEN, self.token);
        Writer::new().u32(CLIENT_TOKEN).u32(self.token).raw(proof.to_wire())
    }

    pub async fn log_in(&mut self, name: &str, password: &str) -> bool {
        self.send(0x29, self.proof(password).cstr(name)).await;
        self.expect(0x29).await[0] == 1
    }

    pub async fn enter_chat(&mut self) -> Vec<u8> {
        self.send(0x0a, Writer::new().cstr("").cstr("")).await;
        self.expect(0x0a).await
    }

    pub async fn join(&mut self, channel: &str) {
        self.send(0x0c, Writer::new().u32(1).cstr(channel)).await;
    }

    pub async fn say(&mut self, text: &str) {
        self.send(0x0e, Writer::new().cstr(text)).await;
    }

    pub async fn events(&mut self) -> Vec<(u32, String, String)> {
        self.quiet().await.iter().filter(|(id, _)| *id == 0x0f).map(|(_, body)| event(body)).collect()
    }

    pub async fn host(&mut self, name: &str, password: &str, status: u32) -> bool {
        let body = Writer::new().u32(status).u32(0).u16(2).u16(1).u32(0).u32(0);
        self.send(0x08, body.cstr(name).cstr(password).cstr("info")).await;
        self.expect(0x08).await[0] == 1
    }

    pub async fn set_status(&mut self, name: &str, password: &str, status: u32) {
        let body = Writer::new().u32(status).u32(0).u16(2).u16(1).u32(0).u32(0);
        self.send(0x08, body.cstr(name).cstr(password).cstr("info")).await;
    }

    pub async fn games(&mut self, name: &str, password: &str) -> Vec<u8> {
        let body = Writer::new().u16(0).u16(0).u32(0).u32(0).u32(25);
        self.send(0x09, body.cstr(name).cstr(password)).await;
        self.expect(0x09).await
    }

    pub async fn join_game(&mut self, name: &str, password: &str) {
        let body = Writer::new().u32(product::WARCRAFT2.0).u32(0x4f);
        self.send(0x22, body.cstr(name).cstr(password)).await;
    }

    pub async fn report(&mut self, results: &[(&str, u32)]) {
        let mut body = Writer::new().u32(0).u32(results.len() as u32);
        for (_, result) in results {
            body = body.u32(*result);
        }
        for (name, _) in results {
            body = body.cstr(name);
        }
        self.send(0x2c, body.cstr("head").cstr("body")).await;
    }

    pub async fn user_data(&mut self, name: &str, keys: &[&str]) -> Vec<u8> {
        let mut body = Writer::new().u32(1).u32(keys.len() as u32).u32(7).cstr(name);
        for key in keys {
            body = body.cstr(key);
        }
        self.send(0x26, body).await;
        self.expect(0x26).await[12..].to_vec()
    }

    pub async fn download(address: SocketAddr, name: &str) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).await.unwrap();
        let size = 2 + 2 + 8 + 12 + 8 + name.len() + 1;
        let request = Writer::new()
            .u16(size as u16)
            .u16(0x0100)
            .u32(Tag::new(b"IX86").0)
            .u32(product::WARCRAFT2.0)
            .u32(0)
            .u32(0)
            .u32(0)
            .u64(0)
            .cstr(name);
        stream.write_all(&[0x02]).await.unwrap();
        stream.write_all(&request.finish()).await.unwrap();
        let mut data = Vec::new();
        let mut chunk = [0; 4096];
        while let Ok(Ok(count)) = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut chunk)).await {
            if count == 0 {
                break;
            }
            data.extend_from_slice(&chunk[..count]);
        }
        data
    }
}

fn identity() -> Writer {
    Writer::new().u32(Tag::new(b"IX86").0).u32(product::WARCRAFT2.0).u32(0x4f)
}

/// An event's kind, name and text.
pub fn event(body: &[u8]) -> (u32, String, String) {
    let mut r = Reader::new(body);
    let kind = r.u32().unwrap();
    r.skip(20).unwrap();
    let mut text = || String::from_utf8_lossy(r.cstr(999).unwrap()).into_owned();
    (kind, text(), text())
}
