//! What a Warcraft II player can do, over real sockets.

mod common;

use common::{Client, server};
use openbnet::store::Role;
use openbnet::wire::Writer;
use tokio::net::UdpSocket;

const INFO: u32 = 0x12;
const ERROR: u32 = 0x13;
const OPEN: u32 = 0x04;
const STARTED: u32 = 0x0e;

fn has(events: &[(u32, String, String)], kind: u32, name: &str, text: &str) -> bool {
    events.iter().any(|e| e.0 == kind && e.1 == name && e.2.contains(text))
}

#[tokio::test]
async fn accounts_log_in_only_with_their_password() {
    let (address, _) = server(|_| {}).await;
    let mut client = Client::connect(address).await;
    assert!(client.create("Thrall", "Secret").await);
    assert!(!client.create("thrall", "other").await, "names ignore case");
    assert!(!client.log_in("Thrall", "wrong").await);
    assert!(!client.log_in("Nobody", "Secret").await);
    assert!(client.log_in("THRALL", "secret").await, "passwords ignore case, as in the games");
}

#[tokio::test]
async fn bad_names_are_refused() {
    let (address, _) = server(|_| {}).await;
    let mut client = Client::connect(address).await;
    for name in ["", "x", "sixteencharacter", "a b", "a/b", "a\\b", "caf\u{e9}", "#1x"] {
        assert!(!client.create(name, "secret").await, "{name:?}");
    }
}

#[tokio::test]
async fn an_account_without_a_usable_password_cannot_be_entered() {
    let (address, server) = server(|_| {}).await;
    let mut client = Client::connect(address).await;
    assert!(client.create("orc", "secret").await);
    server.store.break_password_for_test("orc").await;
    assert!(!client.log_in("orc", "secret").await);
    assert!(!client.log_in("orc", "").await);
    let change = client.proof("anything").raw([7; 20]).cstr("orc");
    client.send(0x31, change).await;
    assert_eq!(client.expect(0x31).await[0], 0, "PvPGN lets anyone set a password here");
}

#[tokio::test]
async fn a_proof_for_another_session_is_refused() {
    let (address, _) = server(|_| {}).await;
    let mut first = Client::connect(address).await;
    assert!(first.create("orc", "secret").await);
    let mut second = Client::connect(address).await;
    second.send(0x29, first.proof("secret").cstr("orc")).await;
    assert_eq!(second.expect(0x29).await[0], 0);
}

#[tokio::test]
async fn guessing_passwords_is_limited() {
    let (address, _) = server(|config| config.accounts.max_failures = 3).await;
    let mut client = Client::connect(address).await;
    assert!(client.create("orc", "secret").await);
    for _ in 0..3 {
        assert!(!client.log_in("orc", "wrong").await);
    }
    assert!(!client.log_in("orc", "secret").await, "even the right password must wait");
}

#[tokio::test]
async fn passwords_can_be_changed() {
    let (address, _) = server(|_| {}).await;
    let mut client = Client::connect(address).await;
    assert!(client.create("orc", "old").await);
    let new = openbnet::xsha1::Hash::of_password("new").to_wire();
    client.send(0x31, client.proof("wrong").raw(new).cstr("orc")).await;
    assert_eq!(client.expect(0x31).await[0], 0);
    client.send(0x31, client.proof("old").raw(new).cstr("orc")).await;
    assert_eq!(client.expect(0x31).await[0], 1);
    assert!(!client.log_in("orc", "old").await);
    assert!(client.log_in("orc", "new").await);
}

#[tokio::test]
async fn a_second_login_replaces_the_first() {
    let (address, _) = server(|_| {}).await;
    let mut first = Client::player(address, "orc").await;
    let mut second = Client::connect(address).await;
    assert!(second.log_in("orc", "secret").await);
    assert!(first.closed().await);
}

#[tokio::test]
async fn strict_servers_refuse_modified_games() {
    let (address, _) = server(|config| config.server.strict_versions = true).await;
    let identity = || Writer::new().raw(b"68XI").raw(b"NB2W").u32(0x4f);
    let mut client = Client::open(address).await;
    client.send(0x06, identity().u32(0)).await;
    client.expect(0x06).await;
    client.send(0x07, identity().u32(0x0200_0200).u32(1).cstr("x")).await;
    assert_eq!(client.expect(0x07).await[0], 0);
    let mut good = Client::connect(address).await;
    assert!(good.create("orc", "secret").await);
    assert!(good.log_in("orc", "secret").await);
    let mut skipped = Client::open(address).await;
    skipped.send(0x1e, Writer::new().raw([0; 20]).cstr("").cstr("")).await;
    let token = skipped.expect(0x1d).await;
    skipped.token = u32::from_le_bytes(token[4..8].try_into().unwrap());
    assert!(!skipped.log_in("orc", "secret").await, "skipping the check doesn't help");
}

#[tokio::test]
async fn the_version_check_is_skipped_without_its_file() {
    let versioning = || Writer::new().raw(b"68XI").raw(b"NB2W").u32(0x4f).u32(0);
    let dir = tempfile::tempdir().unwrap();
    for (with_file, strict, answer) in [(false, false, 0x07), (true, false, 0x06), (false, true, 0x06)] {
        if with_file {
            std::fs::write(dir.path().join("ver-IX86-1.mpq"), b"stand-in").unwrap();
        } else {
            let _ = std::fs::remove_file(dir.path().join("ver-IX86-1.mpq"));
        }
        let (address, _) = server(|config| {
            config.files.directory = dir.path().into();
            config.files.allow_unknown = true;
            config.server.strict_versions = strict;
        })
        .await;
        let mut client = Client::open(address).await;
        client.send(0x06, versioning()).await;
        let (id, body) = client.next_of(&[0x06, 0x07]).await;
        assert_eq!(id, answer, "file {with_file}, strict {strict}");
        if id == 0x07 {
            assert_eq!(body, [2, 0, 0, 0, 0, 0]);
        }
    }
}

#[tokio::test]
async fn players_chat_whisper_and_emote() {
    let (address, _) = server(|_| {}).await;
    let mut a = Client::chatter(address, "Alice").await;
    let mut b = Client::chatter(address, "Bob").await;
    assert!(has(&a.events().await, 0x02, "Bob", "NB2W"));
    a.say("hello").await;
    assert!(has(&b.events().await, 0x05, "Alice", "hello"));
    assert!(a.events().await.is_empty(), "the client shows its own talk");
    b.say("/me waves").await;
    assert!(has(&a.events().await, 0x17, "Bob", "waves"));
    a.say("/w bob psst").await;
    assert!(has(&b.events().await, 0x04, "Alice", "psst"));
    assert!(has(&a.events().await, 0x0a, "Bob", "psst"));
    b.say("/r what").await;
    assert!(has(&a.events().await, 0x04, "Bob", "what"));
    b.say("/ignore alice").await;
    b.quiet().await;
    a.say("still there?").await;
    a.say("/w bob still there?").await;
    assert!(b.events().await.is_empty());
    drop(b);
    assert!(has(&a.events().await, 0x03, "Bob", ""));
}

#[tokio::test]
async fn flooding_is_stopped() {
    let (address, _) = server(|config| {
        config.chat.flood_lines = 5;
        config.chat.flood_disconnect_lines = 10;
    })
    .await;
    let mut a = Client::chatter(address, "Alice").await;
    for _ in 0..6 {
        a.say("spam").await;
    }
    assert!(has(&a.events().await, ERROR, "", "quota"));
    for _ in 0..6 {
        a.say("/w alice spam").await;
    }
    assert!(a.closed().await);
}

#[tokio::test]
async fn the_first_into_a_new_channel_runs_it() {
    let (address, _) = server(|_| {}).await;
    let mut a = Client::chatter(address, "Alice").await;
    let mut b = Client::chatter(address, "Bob").await;
    a.join("My Place").await;
    assert!(has(&a.events().await, INFO, "", "tempOP"));
    b.join("my place").await;
    b.quiet().await;
    b.say("/kick alice").await;
    assert!(has(&b.events().await, ERROR, "", "Operator"));
    a.say("/ban bob rude").await;
    let events = b.events().await;
    assert!(has(&events, INFO, "", "Bob has been banned by Alice (rude)."));
    assert!(has(&events, 0x07, "Bob", "The Void"));
    b.join("My Place").await;
    assert!(has(&b.events().await, ERROR, "", "banned"));
}

#[tokio::test]
async fn staff_channels_and_commands_need_a_role() {
    let (address, server) = server(|_| {}).await;
    let mut a = Client::chatter(address, "Alice").await;
    let mut b = Client::chatter(address, "Bob").await;
    a.quiet().await;
    a.join("Backstage").await;
    assert!(has(&a.events().await, ERROR, "", "Admins"));
    a.say("/kill bob").await;
    assert!(has(&a.events().await, ERROR, "", "reserved"));
    let alice = server.store.account("alice").await.unwrap().unwrap().id;
    server.store.set_role(alice, Role::Moderator).await.unwrap();
    server.hub.lock().set_role(alice, Role::Moderator);
    a.say("/whois bob").await;
    assert!(has(&a.events().await, INFO, "", "127.0.0.1"), "staff see addresses");
    b.say("/whois alice").await;
    assert!(!has(&b.events().await, INFO, "", "127.0.0.1"), "players don't");
    a.say("/lock bob").await;
    assert!(b.closed().await);
    let mut again = Client::connect(address).await;
    assert!(!again.log_in("Bob", "secret").await);
}

#[tokio::test]
async fn a_game_is_hosted_listed_joined_and_scored() {
    let (address, server) = server(|_| {}).await;
    let mut a = Client::chatter(address, "Alice").await;
    let mut b = Client::chatter(address, "Bob").await;
    assert!(a.host("Duel", "", OPEN).await);
    let list = b.games("", "").await;
    assert_eq!(list[..4], [1, 0, 0, 0]);
    assert_eq!(list[14..18], [0x17, 0xe0, 127, 0], "port 6112 and the host's address");
    assert!(list.ends_with(b"Duel\0\0info\0"));
    b.join_game("Duel", "").await;
    a.set_status("Duel", "", STARTED).await;
    b.quiet().await;
    assert_eq!(b.games("", "").await[..4], [0; 4], "started games are hidden");
    assert_eq!(b.games("Duel", "").await[4], 4, "and say so when asked for");
    a.report(&[("Alice", 1), ("Bob", 2)]).await;
    b.report(&[("Alice", 1), ("Bob", 2)]).await;
    b.quiet().await;
    let keys = ["Record\\W2BN\\0\\wins", "Record\\W2BN\\0\\losses"];
    assert_eq!(b.user_data("Alice", &keys).await, b"1\x000\0");
    assert_eq!(b.user_data("Bob", &keys).await, b"0\x001\0");
    assert_eq!(server.hub.lock().game_count(), 0);
}

#[tokio::test]
async fn private_games_need_their_password() {
    let (address, _) = server(|_| {}).await;
    let mut a = Client::chatter(address, "Alice").await;
    let mut b = Client::chatter(address, "Bob").await;
    assert!(a.host("Duel", "pw", OPEN).await);
    assert_eq!(b.games("", "").await[..4], [0; 4]);
    assert_eq!(b.games("Duel", "no").await[4], 2);
    assert_eq!(b.games("Duel", "pw").await[..4], [1, 0, 0, 0]);
}

#[tokio::test]
async fn a_leaving_host_takes_the_game_along() {
    let (address, _) = server(|_| {}).await;
    let mut a = Client::chatter(address, "Alice").await;
    let mut b = Client::chatter(address, "Bob").await;
    assert!(a.host("Duel", "", OPEN).await);
    drop(a);
    b.quiet().await;
    assert_eq!(b.games("", "").await[..4], [0; 4]);
}

#[tokio::test]
async fn udp_teaches_the_server_where_games_are() {
    let (address, server) = server(|_| {}).await;
    let mut a = Client::chatter(address, "Alice").await;
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let send = async |token| {
        udp.send_to(&Writer::new().u32(8).u32(token).finish(), address).await.unwrap();
    };
    send(a.token ^ 1).await;
    send(a.token).await;
    let mut reply = [0; 16];
    let (size, _) = udp.recv_from(&mut reply).await.unwrap();
    assert_eq!(&reply[..size], b"\x05\0\0\0tenb");
    assert!(a.host("Duel", "", OPEN).await);
    let hub = server.hub.lock();
    let session = hub.find_user("alice").unwrap();
    assert_eq!(session.game_address.port(), udp.local_addr().unwrap().port());
}

#[tokio::test]
async fn profiles_belong_to_their_owner() {
    let (address, _) = server(|_| {}).await;
    let mut a = Client::chatter(address, "Alice").await;
    let mut b = Client::chatter(address, "Bob").await;
    let update = Writer::new().u32(1).u32(2).cstr("Bob").cstr("profile\\location").cstr("BNET\\auth\\admin");
    a.send(0x27, update.cstr("Lordaeron").cstr("true")).await;
    a.quiet().await;
    let keys = ["profile\\location", "BNET\\auth\\admin", "BNET\\acct\\passhash1"];
    assert_eq!(b.user_data("Alice", &keys).await, b"Lordaeron\0\0\0");
    assert_eq!(a.user_data("Alice", &keys).await, b"Lordaeron\0\0\0", "not even the owner sees a hash");
    assert_eq!(b.user_data("Bob", &keys).await, b"\0\0\0");
}

#[tokio::test]
async fn missing_files_get_no_answer() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("icons.bni"), b"icons").unwrap();
    let (address, _) = server(|config| config.files.directory = dir.path().into()).await;
    assert!(Client::download(address, "icons.bni").await.ends_with(b"icons.bni\0icons"));
    // An empty file would be saved, and the game would then fail its version check.
    assert!(Client::download(address, "ver-IX86-1.mpq").await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn vanishing_clients_do_not_disturb_the_database() {
    let (address, server) = server(|config| config.accounts.max_failures = 0).await;
    let mut maker = Client::connect(address).await;
    assert!(maker.create("victim", "secret").await);
    let mut tasks = Vec::new();
    for n in 0..400u64 {
        tasks.push(tokio::spawn(async move {
            let mut client = Client::connect(address).await;
            let hash = openbnet::xsha1::Hash::of_password("x").to_wire();
            for part in 0..5 {
                client.send(0x2a, Writer::new().raw(hash).cstr(format!("gone{n}x{part}"))).await;
                client.send(0x29, client.proof("secret").cstr("victim")).await;
                client.send(0x26, Writer::new().u32(1).u32(1).u32(0).cstr("victim").cstr("profile\\sex")).await;
                client.send(0x00, Writer::new()).await;
            }
            tokio::time::sleep(std::time::Duration::from_micros(n * 37 % 5000)).await;
            client.reset();
        }));
        if n % 20 == 0 {
            assert!(Client::connect(address).await.create(&format!("stays{n}"), "x").await, "stays{n}");
        }
    }
    for task in tasks {
        let _ = task.await;
    }
    assert!(Client::connect(address).await.create("afterwards", "x").await);
    assert!(server.store.account("afterwards").await.unwrap().is_some());
}

#[tokio::test]
async fn hostile_packets_do_no_harm() {
    let (address, _) = server(|_| {}).await;
    for id in 0..=255u8 {
        let mut client = Client::connect(address).await;
        client.send(id, Writer::new()).await;
        client.send(id, Writer::new().u32(u32::MAX).u32(u32::MAX).u32(u32::MAX)).await;
        client.send(id, Writer::new().raw([0xff; 200])).await;
    }
    for id in 0..=255u8 {
        let mut client = Client::player(address, &format!("player{id}")).await;
        client.send(id, Writer::new()).await;
        client.send(id, Writer::new().u32(u32::MAX).u32(u32::MAX).u32(u32::MAX)).await;
        client.send(id, Writer::new().raw([0xff; 200])).await;
    }
    for junk in [&b"\xff\x00\x00\x00"[..], b"\xff\x00\xff\xff", b"GET / HTTP/1.0\r\n\r\n"] {
        let mut client = Client::open(address).await;
        client.send_bytes(junk).await;
        assert!(client.closed().await);
    }
    assert!(Client::connect(address).await.create("alive", "secret").await);
}
