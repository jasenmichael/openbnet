//! Chatting without a game: bots, telnet, IRC, WebSocket and the HTTP API.

mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use common::{Client, Started, start};
use futures_util::{SinkExt, StreamExt};
use openbnet::store::Role;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_tungstenite::tungstenite::Message;

/// A client of a line-based protocol.
struct Terminal<S> {
    stream: S,
    seen: String,
}

impl Terminal<TcpStream> {
    async fn open(address: SocketAddr, first: &[u8]) -> Self {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(first).await.unwrap();
        Terminal { stream, seen: String::new() }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> Terminal<S> {
    async fn send(&mut self, line: &str) {
        self.stream.write_all(format!("{line}\r\n").as_bytes()).await.unwrap();
    }

    /// Waits for text, and returns everything up to its end.
    async fn until(&mut self, text: &str) -> String {
        loop {
            if let Some(at) = self.seen.find(text) {
                let rest = self.seen.split_off(at + text.len());
                return std::mem::replace(&mut self.seen, rest);
            }
            let mut chunk = [0; 4096];
            let read = tokio::time::timeout(Duration::from_secs(5), self.stream.read(&mut chunk)).await;
            match read {
                Ok(Ok(count)) if count > 0 => self.seen += &String::from_utf8_lossy(&chunk[..count]),
                other => panic!("waiting for {text:?}, got {other:?} after {:?}", self.seen),
            }
        }
    }

    /// Everything that arrives until nothing has for a moment.
    async fn quiet(&mut self) -> String {
        let mut chunk = [0; 4096];
        while let Ok(Ok(count)) =
            tokio::time::timeout(Duration::from_millis(250), self.stream.read(&mut chunk)).await
        {
            if count == 0 {
                break;
            }
            self.seen += &String::from_utf8_lossy(&chunk[..count]);
        }
        std::mem::take(&mut self.seen)
    }

    async fn irc_login(&mut self, name: &str, password: &str) {
        self.send(&format!("PASS {password}")).await;
        self.send(&format!("NICK {name}")).await;
        self.send("USER x 0 * :x").await;
        self.until("now logged in").await;
        self.until("\r\n").await;
    }
}

async fn chat_server() -> Started {
    start(|config| {
        config.bots.enabled = true;
        config.bots.accounts = vec!["*".into()];
        config.irc.listen = Some("127.0.0.1:0".parse().unwrap());
        config.http.listen = Some("127.0.0.1:0".parse().unwrap());
        config.http.admin_token = "token".into();
    })
    .await
}

async fn bot(address: SocketAddr, name: &str) -> Terminal<TcpStream> {
    let mut bot = Terminal::open(address, b"\x03\x04").await;
    bot.send(name).await;
    bot.send("secret").await;
    bot.until("1007 CHANNEL \"Chat\"\r\n").await;
    bot.quiet().await;
    bot
}

#[tokio::test]
async fn bots_chat_with_games() {
    let s = chat_server().await;
    let mut game = Client::chatter(s.address, "Thrall").await;
    let mut maker = Client::connect(s.address).await;
    assert!(maker.create("Robot", "secret").await);
    let mut robot = Terminal::open(s.address, b"\x03\x04").await;
    robot.until("Username: ").await;
    robot.send("Robot").await;
    robot.until("Password: ").await;
    robot.send("wrong").await;
    robot.until("Login failed.").await;
    robot.send("robot").await;
    robot.send("secret").await;
    robot.until("2010 NAME Robot\r\n1007 CHANNEL \"Chat\"\r\n1001 USER Robot 0010 [CHAT]\r\n").await;
    robot.send("/join War2BNE").await;
    robot.until("1001 USER Thrall 0010 [W2BN]\r\n").await;
    robot.send("beep").await;
    let events = game.events().await;
    assert!(events.iter().any(|e| e.0 == 2 && e.1 == "Robot" && e.2 == "TAHC"));
    assert!(events.iter().any(|e| e.0 == 5 && e.1 == "Robot" && e.2 == "beep"));
    game.say("zug zug").await;
    robot.until("1005 TALK Thrall 0010 \"zug zug\"\r\n").await;
    game.say("/w robot hi\r\n1019 ERROR \"forged\"").await;
    let got = robot.until("forged").await;
    assert!(!got.contains("\n1019"), "a game can't add lines: {got}");
    game.say("/me dances").await;
    robot.until("1023 EMOTE Thrall 0010 \"dances\"\r\n").await;
    drop(game);
    robot.until("1003 LEAVE Thrall 0010\r\n").await;
}

#[tokio::test]
async fn people_chat_by_telnet() {
    let s = chat_server().await;
    let mut maker = Client::connect(s.address).await;
    assert!(maker.create("Human", "secret").await);
    let mut human = Terminal::open(s.address, b"\r\n").await;
    human.until("Username: ").await;
    human.send("Human").await;
    human.until("Password: ").await;
    human.send("secret").await;
    human.until("Joining channel: \"Chat\"\r\n[Human is here]\r\n").await;
    let mut game = Client::chatter(s.address, "Thrall").await;
    game.say("/w human \x1b[2Jboo").await;
    human.until("<from Thrall> ?[2Jboo\r\n").await;
}

#[tokio::test]
async fn text_chat_is_off_unless_allowed() {
    let s = start(|_| {}).await;
    for first in [b"\x03", b"\r"] {
        let mut stream = TcpStream::connect(s.address).await.unwrap();
        stream.write_all(first).await.unwrap();
        assert_eq!(stream.read(&mut [0; 10]).await.unwrap_or(0), 0);
    }
    let s = start(|config| {
        config.bots.enabled = true;
        config.bots.accounts = vec!["Allowed".into()];
    })
    .await;
    let mut maker = Client::connect(s.address).await;
    assert!(maker.create("Allowed", "secret").await && maker.create("Other", "secret").await);
    let mut other = Terminal::open(s.address, b"\x03\x04").await;
    other.send("Other").await;
    other.send("secret").await;
    other.until("Account has no bot access.").await;
    bot(s.address, "Allowed").await;
}

#[tokio::test]
async fn irc_users_chat_with_games() {
    let s = chat_server().await;
    let mut game = Client::chatter(s.address, "Thrall").await;
    let mut irc = Terminal::open(s.irc.unwrap(), b"").await;
    irc.send("NICK Jaina").await;
    irc.send("USER x 0 * :Jaina").await;
    irc.until(" 376 ").await;
    irc.send("JOIN #War2BNE").await;
    irc.until(" 451 ").await;
    irc.send("PRIVMSG NickServ :identify secret").await;
    irc.until("Wrong name or password").await;
    irc.send("PRIVMSG NickServ :register secret").await;
    let shown = irc.until("now logged in").await;
    assert!(!shown.contains("secret"), "passwords aren't shown, unlike on PvPGN");
    irc.send("JOIN #War2BNE").await;
    irc.until(":Jaina!IIRC@openbnet JOIN #War2BNE-1\r\n").await;
    let names = irc.until(" 366 Jaina #War2BNE-1 ").await;
    assert!(names.contains(":Thrall\r\n") && names.contains(":Jaina\r\n"), "{names}");
    irc.send("PRIVMSG #War2BNE-1 :hello").await;
    irc.send("PRIVMSG #War2BNE-1 :\x01ACTION waves\x01").await;
    let events = game.events().await;
    assert!(events.iter().any(|e| e.0 == 5 && e.1 == "Jaina" && e.2 == "hello"));
    assert!(events.iter().any(|e| e.0 == 0x17 && e.1 == "Jaina" && e.2 == "waves"));
    irc.quiet().await;
    game.say("hi\r\n:x KILL y").await;
    let got = irc.until("KILL y").await;
    assert_eq!(got.matches('\n').count(), 0, "a game can't add lines: {got}");
    game.say("/me bows").await;
    irc.until(":Thrall!W2BN@openbnet PRIVMSG #War2BNE-1 :\x01ACTION bows\x01\r\n").await;
    game.say("/w jaina psst").await;
    irc.until(":Thrall!W2BN@openbnet PRIVMSG Jaina :psst\r\n").await;
    irc.send("PRIVMSG thrall :yes?").await;
    assert!(game.events().await.iter().any(|e| e.0 == 4 && e.1 == "Jaina" && e.2 == "yes?"));
    irc.send("WHOIS thrall").await;
    let whois = irc.until(" 318 ").await;
    assert!(whois.contains("311 Jaina Thrall W2BN openbnet") && !whois.contains("127.0.0.1"), "{whois}");
    irc.send("PRIVMSG #Elsewhere :hi").await;
    irc.until(" 404 ").await;
    irc.send("PART #War2BNE-1").await;
    irc.until(":Jaina!IIRC@openbnet PART #War2BNE-1\r\n").await;
    irc.send("JOIN #My_Own_Place").await;
    let joined = irc.until(" 366 ").await;
    assert!(!joined.contains("PART"), "left only once: {joined}");
    irc.until("MODE #My_Own_Place +h Jaina\r\n").await;
    game.join("my own place").await;
    irc.until(":Thrall!W2BN@openbnet JOIN #My_Own_Place\r\n").await;
    irc.send("KICK #My_Own_Place Thrall :rude").await;
    irc.until(":Thrall!W2BN@openbnet PART #My_Own_Place\r\n").await;
    let events = game.events().await;
    assert!(events.iter().any(|e| e.0 == 7 && e.2 == "The Void"));
    irc.send("LIST").await;
    let list = irc.until(" 323 ").await;
    assert!(list.contains("#War2BNE-1") && !list.contains("Backstage") && !list.contains("My_Own"), "{list}");
    irc.send("PING :abc").await;
    irc.until("PONG openbnet :abc").await;
    irc.send("QUIT").await;
    irc.quiet().await;
    assert!(s.server.hub.lock().find_user("jaina").is_none());
}

#[tokio::test]
async fn irc_works_over_tls() {
    let dir = tempfile::tempdir().unwrap();
    let made = rcgen::generate_simple_self_signed(["localhost".to_owned()]).unwrap();
    let (certificate, key) = (dir.path().join("cert.pem"), dir.path().join("key.pem"));
    std::fs::write(&certificate, made.cert.pem()).unwrap();
    std::fs::write(&key, made.signing_key.serialize_pem()).unwrap();
    let s = start(|config| {
        config.irc.tls_listen = Some("127.0.0.1:0".parse().unwrap());
        config.irc.certificate = Some(certificate.clone());
        config.irc.key = Some(key);
    })
    .await;
    let mut maker = Client::connect(s.address).await;
    assert!(maker.create("Jaina", "secret").await);
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from_pem_file(certificate).unwrap()).unwrap();
    let config = ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
    let stream = TcpStream::connect(s.irc_tls.unwrap()).await.unwrap();
    let name = ServerName::try_from("localhost").unwrap();
    let stream = TlsConnector::from(Arc::new(config)).connect(name, stream).await.unwrap();
    let mut irc = Terminal { stream, seen: String::new() };
    irc.irc_login("jaina", "secret").await;
    irc.send("JOIN #Chat").await;
    irc.until(":Jaina!IIRC@openbnet JOIN #Chat\r\n").await;
}

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

struct Web(Socket);

impl Web {
    async fn open(address: SocketAddr) -> Web {
        Web(tokio_tungstenite::connect_async(format!("ws://{address}/ws")).await.unwrap().0)
    }

    async fn send(&mut self, value: Value) {
        self.0.send(Message::text(value.to_string())).await.unwrap();
    }

    async fn next(&mut self) -> Value {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(5), self.0.next()).await;
            match message.expect("no message").expect("closed").expect("failed") {
                Message::Text(text) => return serde_json::from_str(&text).unwrap(),
                _ => continue,
            }
        }
    }

    async fn until(&mut self, kind: &str) -> Value {
        loop {
            let value = self.next().await;
            if value["type"] == kind {
                return value;
            }
        }
    }
}

#[tokio::test]
async fn browsers_chat_with_games() {
    let s = chat_server().await;
    let mut game = Client::chatter(s.address, "Thrall").await;
    let mut maker = Client::connect(s.address).await;
    assert!(maker.create("Jaina", "secret").await);
    let mut web = Web::open(s.http.unwrap()).await;
    web.send(json!({ "type": "say", "text": "hi" })).await;
    assert_eq!(web.next().await["code"], "not_logged_in");
    web.send(json!({ "type": "nonsense" })).await;
    assert_eq!(web.next().await["code"], "bad_request");
    web.send(json!({ "type": "login", "name": "jaina", "password": "wrong" })).await;
    assert_eq!(web.next().await["code"], "wrong_name_or_password");
    web.send(json!({ "type": "login", "name": "jaina", "password": "secret" })).await;
    assert_eq!(web.next().await, json!({ "type": "logged_in", "name": "Jaina" }));
    web.send(json!({ "type": "join", "channel": "War2BNE" })).await;
    assert_eq!(web.next().await, json!({ "type": "channel", "channel": "War2BNE-1" }));
    let mut names = Vec::new();
    loop {
        let value = web.next().await;
        if value["type"] == "entered" {
            break;
        }
        names.push(format!("{} {}", value["name"], value["product"]));
    }
    assert_eq!(names, [r#""Jaina" "WEBC""#, r#""Thrall" "W2BN""#]);
    web.send(json!({ "type": "say", "text": "hello from the web" })).await;
    assert!(game.events().await.iter().any(|e| e.0 == 5 && e.1 == "Jaina" && e.2 == "hello from the web"));
    game.say("zug zug").await;
    assert_eq!(web.until("talk").await, json!({ "type": "talk", "name": "Thrall", "text": "zug zug" }));
    web.send(json!({ "type": "say", "text": "/w thrall psst" })).await;
    assert!(game.events().await.iter().any(|e| e.0 == 4 && e.2 == "psst"));
    web.send(json!({ "type": "say", "text": "x".repeat(300) })).await;
    assert_eq!(web.until("problem").await["code"], "bad_text");
    drop(web);
    assert!(game.events().await.iter().any(|e| e.0 == 3 && e.1 == "Jaina"));
}

#[tokio::test]
async fn the_api_creates_accounts_and_changes_passwords() {
    let s = chat_server().await;
    let api = format!("http://{}/api", s.http.unwrap());
    let http = reqwest::Client::new();
    let post = async |path: &str, body: Value| {
        http.post(format!("{api}/{path}")).json(&body).send().await.unwrap().status().as_u16()
    };
    let new = json!({ "name": "Jaina", "password": "secret" });
    assert_eq!(post("accounts", new.clone()).await, 201);
    assert_eq!(post("accounts", new).await, 409);
    assert_eq!(post("accounts", json!({ "name": "a b", "password": "x" })).await, 400);
    assert_eq!(post("accounts", json!({ "name": "Other", "password": "" })).await, 400);
    let change = |old: &str| json!({ "name": "jaina", "password": old, "new_password": "better" });
    assert_eq!(post("password", change("wrong")).await, 401);
    assert_eq!(post("password", change("secret")).await, 200);
    let mut game = Client::connect(s.address).await;
    assert!(!game.log_in("Jaina", "secret").await);
    assert!(game.log_in("Jaina", "better").await, "games accept the new password");
    let status: Value = http.get(format!("{api}/status")).send().await.unwrap().json().await.unwrap();
    assert_eq!(status["users"], 1);
    let channels: Value = http.get(format!("{api}/channels")).send().await.unwrap().json().await.unwrap();
    assert!(channels.as_array().unwrap().iter().all(|channel| channel["name"] != "Backstage"));
}

#[tokio::test]
async fn administration_needs_the_token() {
    let s = chat_server().await;
    let mut game = Client::chatter(s.address, "Thrall").await;
    let url = format!("http://{}/api/admin/accounts/thrall", s.http.unwrap());
    let http = reqwest::Client::new();
    assert_eq!(http.get(&url).send().await.unwrap().status(), 401);
    assert_eq!(http.get(&url).bearer_auth("guess").send().await.unwrap().status(), 401);
    let patch = async |body: Value| {
        let sent = http.patch(&url).bearer_auth("token").json(&body).send().await.unwrap();
        (sent.status().as_u16(), sent.json::<Value>().await.unwrap())
    };
    assert_eq!(patch(json!({ "role": "king" })).await.0, 400);
    let (status, account) = patch(json!({ "role": "moderator" })).await;
    assert_eq!((status, &account["role"], &account["online"]), (200, &json!("moderator"), &json!(true)));
    assert!(account.get("password").is_none());
    assert_eq!(s.server.hub.lock().find_user("thrall").unwrap().role(), Role::Moderator);
    assert_eq!(patch(json!({ "locked": true })).await.1["online"], false);
    assert!(game.closed().await);

    let off = start(|config| config.http.listen = Some("127.0.0.1:0".parse().unwrap())).await;
    let url = format!("http://{}/api/admin/reload", off.http.unwrap());
    assert_eq!(http.post(&url).bearer_auth("").send().await.unwrap().status(), 401, "no token set");
}

#[tokio::test]
async fn forwarded_addresses_are_believed_only_behind_a_proxy() {
    for behind_proxy in [false, true] {
        let s = start(|config| {
            config.http.listen = Some("127.0.0.1:0".parse().unwrap());
            config.http.behind_proxy = behind_proxy;
            config.accounts.max_failures = 2;
        })
        .await;
        let url = format!("http://{}/api/password", s.http.unwrap());
        let http = reqwest::Client::new();
        let body = json!({ "name": "nobody", "password": "x", "new_password": "y" });
        for forged in ["192.0.2.1", "192.0.2.2", "192.0.2.3"] {
            let header = format!("{forged}, 198.51.100.7");
            http.post(&url).header("x-forwarded-for", header).json(&body).send().await.unwrap();
        }
        let mut hub = s.server.hub.lock();
        assert_eq!(hub.too_many_failures("198.51.100.7".parse().unwrap()), behind_proxy);
        assert_eq!(hub.too_many_failures("127.0.0.1".parse().unwrap()), !behind_proxy);
        assert!(!hub.too_many_failures("192.0.2.1".parse().unwrap()));
    }
}

#[tokio::test]
async fn signing_up_is_limited() {
    let s = start(|config| config.accounts.max_signups = 2).await;
    let mut client = Client::connect(s.address).await;
    assert!(client.create("one1", "x").await && client.create("two2", "x").await);
    assert!(!client.create("three3", "x").await);
}

#[tokio::test]
async fn settings_can_change_while_running() {
    let s = start(|_| {}).await;
    let mut a = Client::chatter(s.address, "Alice").await;
    let mut config = (*s.server.config()).clone();
    config.accounts.allow_new = false;
    config.chat.max_line = 5;
    s.server.reconfigure(config);
    a.say("too long").await;
    assert!(a.events().await.iter().any(|e| e.2.contains("length quota")));
    assert!(!Client::connect(s.address).await.create("late", "x").await);
}
