//! Compares replies with a real PvPGN, the reference server. See docs/TESTING.md for how to start one.
//!
//! OPENBNET_REF=172.31.78.2:6112 OPENBNET_FILES=../.vendor/pvpgn-server/files cargo test --test reference -- --ignored

mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use common::{Client, Packet, event, server};
use openbnet::wire::Writer;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Writes down everything a server sends during one script.
struct Transcript {
    lines: Vec<String>,
    /// The flags each client was last shown for each name.
    shown: HashMap<(String, String), u32>,
}

impl Transcript {
    fn note(&mut self, who: &str, packets: &[Packet]) {
        for (id, body) in packets {
            let mut body = body.clone();
            match id {
                // Times and tokens differ between any two runs.
                0x25 => continue,
                0x1d => body.fill(0),
                0x06 | 0x2d => body[..8].fill(0),
                0x33 => body[8..16].fill(0),
                // PvPGN asks for an e-mail address, which openbnet doesn't use.
                0x59 => continue,
                0x0f => {
                    let (kind, name, text) = event(&body);
                    body[8..12].fill(0); // latency
                    if name == "Robot" {
                        body[4] &= !0x10;
                    }
                    let flags = u32::from(body[4]);
                    let before = match kind {
                        1 | 2 | 9 => self.shown.insert((who.into(), name.clone()), flags),
                        _ => None,
                    };
                    // PvPGN repeats flags that haven't changed.
                    if kind == 9 && before.unwrap_or(0) == flags {
                        continue;
                    }
                    self.lines.push(format!("{who} event {kind:#x} {name:?} {text:?} {}", hex(&body)));
                    continue;
                }
                0x09 if body.len() > 24 => body[16..20].fill(0), // the host's address
                _ => {}
            }
            self.lines.push(format!("{who} {id:#04x} {}", hex(&body)));
        }
    }
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn read_text(stream: &mut TcpStream) -> String {
    let mut text = Vec::new();
    let mut chunk = [0; 4096];
    while let Ok(Ok(count)) = tokio::time::timeout(Duration::from_millis(400), stream.read(&mut chunk)).await {
        if count == 0 {
            break;
        }
        text.extend_from_slice(&chunk[..count]);
    }
    // Bots have no UDP, so the plug differs here too. PvPGN also repeats flags. Replies such as /whoami can't be
    // compared: PvPGN sends bots freed memory instead of the text.
    let text = String::from_utf8_lossy(&text).replace(" 0010", " 0000");
    text.split_inclusive('\n').filter(|line| !line.starts_with("1009 USER Robot 0000")).collect()
}

async fn script(address: SocketAddr) -> Vec<String> {
    let mut t = Transcript { lines: Vec::new(), shown: HashMap::new() };
    let mut a = Client::open(address).await;
    a.send(0x1e, Writer::new().u32(1).u32(1).u32(0).u32(0).u32(0).cstr("HOST").cstr("USER")).await;
    t.note("a", &a.quiet().await);
    a = Client::connect(address).await;
    a.send(0x06, Writer::new().raw(b"68XI").raw(b"NB2W").u32(0x4f).u32(0)).await;
    a.send(0x2d, Writer::new()).await;
    for name in ["tos_USA.txt", "bnserver.ini", "missing.txt", "IX86ver1.mpq"] {
        a.send(0x33, Writer::new().u32(0x1a).u32(0).cstr(name)).await;
    }
    a.send(0x30, Writer::new().u32(0).cstr("1234567890123").cstr("Owner")).await;
    a.send(0x00, Writer::new()).await;
    t.note("a", &a.quiet().await);

    let hash = |password| openbnet::xsha1::Hash::of_password(password).to_wire();
    a.send(0x2a, Writer::new().raw(hash("secret")).cstr("Alice")).await;
    a.send(0x2a, Writer::new().raw(hash("secret")).cstr("ALICE")).await;
    a.send(0x3d, Writer::new().raw(hash("secret")).cstr("Bob")).await;
    a.send(0x3d, Writer::new().raw(hash("secret")).cstr("bob")).await;
    a.send(0x3d, Writer::new().raw(hash("secret")).cstr("b/d")).await;
    a.send(0x29, a.proof("wrong").cstr("Alice")).await;
    a.send(0x3a, a.proof("wrong").cstr("Alice")).await;
    a.send(0x3a, a.proof("secret").cstr("Nobody")).await;
    a.send(0x31, a.proof("wrong").raw(hash("new")).cstr("Alice")).await;
    a.send(0x31, a.proof("secret").raw(hash("new")).cstr("Alice")).await;
    // Without working UDP there is a plug, which PvPGN drops on joining a channel and openbnet doesn't.
    a.send(0x14, Writer::new().raw(b"tenb")).await;
    a.send(0x29, a.proof("new").cstr("Alice")).await;
    t.note("a", &a.quiet().await);
    let mut b = Client::connect(address).await;
    b.send(0x14, Writer::new().raw(b"tenb")).await;
    b.send(0x3a, b.proof("secret").cstr("Bob")).await;
    t.note("b", &b.quiet().await);

    for client in [&mut a, &mut b] {
        client.send(0x0a, Writer::new().cstr("").cstr("")).await;
        client.send(0x0b, Writer::new().raw(b"NB2W")).await;
    }
    t.note("a", &a.quiet().await);
    t.note("b", &b.quiet().await);

    a.join("War2BNE").await;
    t.note("a", &a.quiet().await);
    b.join("War2BNE").await;
    t.note("b", &b.quiet().await);
    t.note("a", &a.quiet().await);
    for text in ["hello", "/me waves", "/w bob psst", "/w nobody hi", "/whoami", "/whois bob", "/who War2BNE"] {
        a.say(text).await;
        t.note("a", &a.quiet().await);
        t.note("b", &b.quiet().await);
    }
    b.say("/r back").await;
    t.note("b", &b.quiet().await);
    t.note("a", &a.quiet().await);
    a.join("Our Own").await;
    t.note("a", &a.quiet().await);
    t.note("b", &b.quiet().await);
    a.say("anyone?").await;
    t.note("a", &a.quiet().await);
    b.join("our own").await;
    t.note("b", &b.quiet().await);
    t.note("a", &a.quiet().await);
    a.say("/kick bob bye").await;
    t.note("a", &a.quiet().await);
    t.note("b", &b.quiet().await);
    b.join("Backstage").await;
    t.note("b", &b.quiet().await);
    b.join("War2BNE").await;
    b.quiet().await;

    a.send(0x10, Writer::new()).await;
    let start = |status: u32| Writer::new().u32(status).u32(0).u16(2).u16(1).u32(0).u32(0);
    let list = || Writer::new().u16(0).u16(0).u32(0).u32(0).u32(25);
    a.send(0x08, start(4).cstr("Duel").cstr("").cstr("info")).await;
    t.note("a", &a.quiet().await);
    for (name, password) in [("", ""), ("Duel", ""), ("duel", "x"), ("None", "")] {
        b.send(0x09, list().cstr(name).cstr(password)).await;
        t.note("b", &b.quiet().await);
    }
    b.send(0x22, Writer::new().raw(b"NB2W").u32(0x4f).cstr("Duel").cstr("")).await;
    t.note("b", &b.quiet().await);
    for status in [6, 0x0e] {
        a.send(0x08, start(status).cstr("Duel").cstr("").cstr("info")).await;
        t.note("a", &a.quiet().await);
        b.send(0x09, list().cstr("").cstr("")).await;
        b.send(0x09, list().cstr("Duel").cstr("")).await;
        t.note("b", &b.quiet().await);
    }
    a.report(&[("Alice", 1), ("Bob", 2)]).await;
    t.note("a", &a.quiet().await);
    b.report(&[("Alice", 1), ("Bob", 2)]).await;
    t.note("b", &b.quiet().await);
    for name in ["Alice", "Bob"] {
        let fields = ["wins", "losses", "disconnects", "last game result"];
        let keys = Writer::new().u32(1).u32(fields.len() as u32).u32(9).cstr(name);
        let keys = fields.iter().fold(keys, |keys, field| keys.cstr(format!("Record\\W2BN\\0\\{field}")));
        a.send(0x26, keys).await;
        t.note("a", &a.quiet().await);
    }
    // A second game: hosted the newer way, with a password.
    let start4 = |status: u16| Writer::new().u16(status).u16(0).u32(0).u16(2).u16(1).u32(0).u32(0);
    a.send(0x1c, start4(0).cstr("Priv").cstr("pw").cstr("info")).await;
    t.note("a", &a.quiet().await);
    for (name, password) in [("", ""), ("Priv", ""), ("Priv", "pw")] {
        b.send(0x09, list().cstr(name).cstr(password)).await;
        t.note("b", &b.quiet().await);
    }
    a.send(0x1c, start4(8).cstr("Priv").cstr("pw").cstr("info")).await;
    a.send(0x02, Writer::new()).await;
    t.note("a", &a.quiet().await);
    b.send(0x09, list().cstr("Priv").cstr("pw")).await;
    t.note("b", &b.quiet().await);
    // A bot talking with a game.
    a.send(0x0c, Writer::new().u32(2).cstr("Bots")).await;
    a.quiet().await;
    let mut maker = Client::connect(address).await;
    assert!(maker.create("Robot", "secret").await);
    let mut robot = TcpStream::connect(address).await.unwrap();
    robot.write_all(b"\x03\x04").await.unwrap();
    for line in ["Robot", "wrong", "Robot", "secret", "/join Bots", "beep", "/me boops", "/w Alice psst"] {
        robot.write_all(format!("{line}\r\n").as_bytes()).await.unwrap();
        t.lines.push(format!("bot {:?}", read_text(&mut robot).await));
        t.note("a", &a.quiet().await);
    }
    for text in ["hi bot", "/me waves", "/w Robot psst"] {
        a.say(text).await;
        t.lines.push(format!("bot {:?}", read_text(&mut robot).await));
        t.note("a", &a.quiet().await);
    }
    a.join("War2BNE").await;
    t.lines.push(format!("bot {:?}", read_text(&mut robot).await));

    t.lines.push(format!("file missing {:?}", Client::download(address, "missing.mpq").await));
    for name in ["icons.bni", "ver-IX86-1.mpq", "IX86ver1.mpq"] {
        let data = Client::download(address, name).await;
        t.lines.push(format!("file {name} {} {}", hex(&data[..16]), hex(&data[24..])));
    }
    t.lines
}

#[tokio::test]
#[ignore = "needs a running PvPGN"]
async fn replies_match_pvpgn() {
    let reference: SocketAddr = std::env::var("OPENBNET_REF").expect("OPENBNET_REF").parse().unwrap();
    let files = std::env::var("OPENBNET_FILES").expect("OPENBNET_FILES");
    let (address, _) = server(|config| {
        config.files.directory = files.into();
        config.bots.enabled = true;
        config.bots.accounts = vec!["*".into()];
    })
    .await;
    let (theirs, ours) = (script(reference).await, script(address).await);
    if let Ok(directory) = std::env::var("OPENBNET_TRANSCRIPTS") {
        std::fs::write(format!("{directory}/pvpgn.txt"), theirs.join("\n")).unwrap();
        std::fs::write(format!("{directory}/openbnet.txt"), ours.join("\n")).unwrap();
    }
    assert!(theirs == ours, "replies differ; set OPENBNET_TRANSCRIPTS to a directory to see them");
}
