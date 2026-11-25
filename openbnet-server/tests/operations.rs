//! Running a server: limits, bans, stopping, health and accepted builds.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::{Client, Started, start};
use openbnet::store::{Role, Store};
use openbnet::wire::Writer;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn with_api(change: impl FnOnce(&mut openbnet::config::Config)) -> Started {
    start(|config| {
        config.http.listen = Some("127.0.0.1:0".parse().unwrap());
        config.http.admin_token = "token".into();
        config.http.behind_proxy = true;
        change(config);
    })
    .await
}

fn is_closed(result: std::io::Result<usize>) -> bool {
    matches!(result, Ok(0) | Err(_))
}

async fn closes(stream: &mut TcpStream, within: Duration) -> bool {
    tokio::time::timeout(within, stream.read(&mut [0; 256])).await.is_ok_and(is_closed)
}

#[tokio::test]
async fn one_address_cannot_take_every_connection() {
    let s = start(|config| config.server.max_connections_per_ip = 2).await;
    let mut first = TcpStream::connect(s.address).await.unwrap();
    let _second = TcpStream::connect(s.address).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut third = TcpStream::connect(s.address).await.unwrap();
    assert!(closes(&mut third, Duration::from_secs(2)).await, "even before saying anything");
    first.shutdown().await.unwrap();
    drop(first);
    tokio::time::sleep(Duration::from_millis(100)).await;
    Client::connect(s.address).await;
}

#[tokio::test]
async fn the_api_limits_connections_and_slow_requests() {
    let s = with_api(|config| config.http.max_connections = 2).await;
    let address = s.http.unwrap();
    let mut slow = TcpStream::connect(address).await.unwrap();
    slow.write_all(b"GET /api/status HTTP/1.1\r\nHost: x\r\n").await.unwrap();
    let _second = TcpStream::connect(address).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut third = TcpStream::connect(address).await.unwrap();
    assert!(closes(&mut third, Duration::from_secs(2)).await);
    assert!(Client::connect(s.address).await.create("still", "x").await, "games are unaffected");
    // Unfinished headers are given up on after 15 seconds.
    let mut answer = Vec::new();
    let ended = tokio::time::timeout(Duration::from_secs(20), slow.read_to_end(&mut answer)).await;
    assert!(ended.is_ok(), "a slow request is ended");
}

#[tokio::test]
async fn bans_keep_addresses_out() {
    let s = with_api(|_| {}).await;
    let base = format!("http://{}/api", s.http.unwrap());
    let http = reqwest::Client::new();
    let status = async |ip: &str| {
        let body = json!({ "name": "nobody", "password": "x", "new_password": "y" });
        let sent = http.post(format!("{base}/password")).header("x-forwarded-for", ip).json(&body);
        sent.send().await.unwrap().status().as_u16()
    };
    let ban = async |body: Value| {
        let sent = http.post(format!("{base}/admin/bans")).bearer_auth("token").json(&body);
        sent.send().await.unwrap().status().as_u16()
    };
    assert_eq!(status("198.51.100.7").await, 401);
    assert_eq!(ban(json!({ "target": "198.51.100.0/24", "reason": "rude" })).await, 200);
    assert_eq!(status("198.51.100.7").await, 403);
    assert_eq!(status("198.51.101.7").await, 401);
    for silly in ["127.0.0.1", "0.0.0.0/0", "10.0.0.0/8", "nobody"] {
        assert_eq!(ban(json!({ "target": silly })).await, 400, "{silly}");
    }
    let list: Value =
        http.get(format!("{base}/admin/bans")).bearer_auth("token").send().await.unwrap().json().await.unwrap();
    assert_eq!(list[0]["network"], "198.51.100.0/24");
    assert_eq!(http.get(format!("{base}/admin/bans")).send().await.unwrap().status(), 401);
    let lift = http.delete(format!("{base}/admin/bans")).bearer_auth("token");
    lift.json(&json!({ "network": "198.51.100.0/24" })).send().await.unwrap();
    assert_eq!(status("198.51.100.7").await, 401);
}

#[tokio::test]
async fn bans_survive_a_restart_and_expire() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}/t.db?mode=rwc", dir.path().display());
    let store = Store::open(&url).await.unwrap();
    let ban = |network: &str, until| openbnet::store::IpBan {
        range: network.parse().unwrap(),
        reason: String::new(),
        by: "x".into(),
        until,
    };
    store.add_ip_ban(&ban("192.0.2.1", 0)).await.unwrap();
    store.add_ip_ban(&ban("192.0.2.2", 1)).await.unwrap();
    store.close().await;
    let s = start(|config| config.database.url = url).await;
    let hub = s.server.hub.lock();
    assert!(hub.is_banned("192.0.2.1".parse().unwrap()));
    assert!(!hub.is_banned("192.0.2.2".parse().unwrap()), "expired");
    assert_eq!(hub.bans().len(), 1);
}

#[tokio::test]
async fn moderators_ban_players_but_not_themselves_or_each_other() {
    let s = start(|_| {}).await;
    let mut mod_ = Client::chatter(s.address, "Alice").await;
    mod_.say("/ipban 192.0.2.9").await;
    assert!(mod_.events().await.iter().any(|e| e.2.contains("reserved")));
    let alice = s.server.store.account("alice").await.unwrap().unwrap().id;
    s.server.store.set_role(alice, Role::Moderator).await.unwrap();
    s.server.hub.lock().set_role(alice, Role::Moderator);
    mod_.quiet().await;
    mod_.say("/ipban 192.0.2.9 2 being rude").await;
    assert!(mod_.events().await.iter().any(|e| e.2 == "192.0.2.9 is now banned."));
    mod_.say("/ipbans").await;
    assert!(mod_.events().await.iter().any(|e| e.2.contains("by Alice being rude") && e.2.contains("until")));
    // Someone seen from elsewhere, now offline, is banned by name.
    let mut maker = Client::connect(s.address).await;
    assert!(maker.create("Rogue", "x").await);
    let rogue = s.server.store.account("rogue").await.unwrap().unwrap().id;
    s.server.store.note_login(rogue, openbnet::wire::product::WARCRAFT2, "192.0.2.77").await.unwrap();
    mod_.say("/ipban rogue").await;
    assert!(mod_.events().await.iter().any(|e| e.2 == "192.0.2.77 is now banned."));
    for (target, why) in [("alice", "own computer"), ("127.0.0.1", "own computer"), ("203.0.0.0/8", "too many")] {
        mod_.say(&format!("/ipban {target}")).await;
        assert!(mod_.events().await.iter().any(|e| e.2.contains(why)), "{target}");
    }
    mod_.say("/ipunban 192.0.2.9").await;
    assert!(mod_.events().await.iter().any(|e| e.2 == "192.0.2.9 is no longer banned."));
}

#[tokio::test]
async fn stopping_warns_players_and_keeps_results() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}/t.db?mode=rwc", dir.path().display());
    let s = start(|config| {
        config.database.url = url.clone();
        config.server.shutdown_secs = 1;
    })
    .await;
    let mut a = Client::chatter(s.address, "Alice").await;
    let mut b = Client::chatter(s.address, "Bob").await;
    assert!(a.host("Duel", "", 4).await);
    b.join_game("Duel", "").await;
    b.quiet().await;
    a.set_status("Duel", "", 0x0e).await;
    // One has reported; the other is still playing when the server stops.
    a.report(&[("Alice", 1), ("Bob", 2)]).await;
    a.quiet().await;
    let server = s.server.clone();
    let stopping = tokio::spawn(async move { server.stop_gently().await });
    assert!(b.events().await.iter().any(|e| e.2 == "The server is shutting down in 1 seconds."));
    stopping.await.unwrap();
    assert!(a.closed().await && b.closed().await);
    let ended = tokio::time::timeout(Duration::from_secs(10), s.running).await;
    ended.expect("stops in time").unwrap().unwrap();
    assert!(TcpStream::connect(s.address).await.is_err(), "no longer listening");
    let store = Store::open(&url).await.unwrap();
    let alice = store.account("alice").await.unwrap().unwrap().id;
    assert_eq!(store.records(alice, openbnet::wire::product::WARCRAFT2).await.unwrap()[&0].wins, 1);
}

#[tokio::test]
async fn health_tells_whether_the_server_answers() {
    let s = with_api(|_| {}).await;
    let mut config = (*s.server.config()).clone();
    config.server.listen = s.address;
    openbnet::health::check(&config).await.unwrap();
    let url = format!("http://{}/api/health", s.http.unwrap());
    assert_eq!(reqwest::get(&url).await.unwrap().status(), 200);
    s.server.stop.cancel();
    s.running.await.unwrap().unwrap();
    assert!(openbnet::health::check(&config).await.is_err());
    config.server.listen = SocketAddr::from(([0, 0, 0, 0], s.address.port()));
    assert!(openbnet::health::check(&config).await.is_err());
}

#[tokio::test]
async fn strict_servers_accept_listed_builds_only() {
    let s = start(|config| {
        config.server.strict_versions = true;
        config.server.products = vec!["W2BN".into()];
    })
    .await;
    let identity = || Writer::new().raw(b"68XI").raw(b"NB2W").u32(0x4f);
    for (checksum, accepted) in [(0x270d_fc2b, true), (0xb52b_ad87, true), (0x1234_5678, false)] {
        let mut client = Client::open(s.address).await;
        client.send(0x06, identity().u32(0)).await;
        client.expect(0x06).await;
        client.send(0x07, identity().u32(0x0200_0200).u32(checksum).cstr("x")).await;
        assert_eq!(client.expect(0x07).await[0] == 2, accepted, "{checksum:#x}");
    }
}

#[tokio::test]
async fn games_not_allowed_are_turned_away() {
    let s = start(|config| config.server.products = vec!["W2BN".into()]).await;
    Client::connect(s.address).await;
    let mut other = Client::open(s.address).await;
    other.send(0x06, Writer::new().raw(b"68XI").raw(b"RATS").u32(0xd3).u32(0)).await;
    assert!(other.closed().await);
}
