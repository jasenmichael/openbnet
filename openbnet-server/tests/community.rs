//! Ladders, friends, topics, banners, and the other older games.

mod common;

use common::{CLIENT_TOKEN, Client, server, start};
use openbnet::store::{FinishedGame, Outcome, Store};
use openbnet::wire::{Reader, Writer, product};
use openbnet::xsha1::Hash;

const LADDER: u16 = 9;
const MELEE: u16 = 2;

/// Gives an account enough wins to play ladder games.
async fn qualify(store: &Store, name: &str) {
    let account = store.account(name).await.unwrap().unwrap().id;
    for _ in 0..10 {
        let game = FinishedGame {
            product: product::WARCRAFT2,
            name: "practice".into(),
            game_type: MELEE,
            ladder: 0,
            started_at: 1,
            results: vec![(account, Outcome::Win)],
            reports: vec![],
        };
        store.finish_game(&game).await.unwrap();
    }
}

async fn two_players(
    qualified: bool,
) -> (Client, Client, std::sync::Arc<openbnet::server::Server>, std::net::SocketAddr) {
    let (address, server) = server(|_| {}).await;
    let mut maker = Client::connect(address).await;
    for name in ["Alice", "Bob"] {
        assert!(maker.create(name, "secret").await);
        if qualified {
            qualify(&server.store, name).await;
        }
    }
    let mut players = Vec::new();
    for name in ["Alice", "Bob"] {
        let mut player = Client::connect(address).await;
        assert!(player.log_in(name, "secret").await);
        player.enter_chat().await;
        player.join("War2BNE").await;
        player.quiet().await;
        players.push(player);
    }
    let b = players.pop().unwrap();
    (players.pop().unwrap(), b, server, address)
}

/// A ladder reply's names with ratings and ranks.
async fn ladder(client: &mut Client, order: u32, start: u32, count: u32) -> Vec<(String, u32, u32)> {
    let body = Writer::new().raw(b"NB2W").u32(1).u32(order).u32(start).u32(count);
    client.send(0x2e, body).await;
    let reply = client.expect(0x2e).await;
    let mut r = Reader::new(&reply);
    r.skip(16).unwrap();
    let count = r.u32().unwrap();
    (0..count)
        .map(|_| {
            r.skip(12).unwrap();
            let (rating, rank) = (r.u32().unwrap(), r.u32().unwrap());
            r.skip(60).unwrap();
            (String::from_utf8(r.cstr(20).unwrap().to_vec()).unwrap(), rating, rank)
        })
        .collect()
}

#[tokio::test]
async fn ladder_games_change_ratings_and_ranks() {
    let (mut a, mut b, _, _) = two_players(true).await;
    a.play(&mut b, "Ladder1", LADDER, &[("Alice", 2), ("Bob", 1)]).await;
    let keys = ["Record\\W2BN\\1\\rating", "Record\\W2BN\\1\\wins", "Record\\W2BN\\0\\wins"];
    assert_eq!(a.user_data("Bob", &keys).await, b"1025\x001\x0010\0");
    assert_eq!(a.user_data("Alice", &keys).await, b"975\x000\x0010\0");
    let place = |name: &str, rating, rank| (name.to_owned(), rating, rank);
    assert_eq!(ladder(&mut a, 0, 0, 3).await, [place("Bob", 1025, 0), place("Alice", 975, 1), place(" ", 0, 0)]);
    assert_eq!(ladder(&mut a, 0, 1, 1).await, [place("Alice", 975, 1)]);
    for (name, expected) in [("bob", 0u32), ("ALICE", 1), ("nobody", u32::MAX)] {
        a.send(0x2f, Writer::new().raw(b"NB2W").u32(1).u32(0).cstr(name)).await;
        assert_eq!(a.expect(0x2f).await, expected.to_le_bytes());
    }
    let shown = b.enter_chat().await;
    assert!(shown.windows(24).any(|w| w == b"NB2W 1025 1 10 0 0 1025 "), "{:?}", String::from_utf8_lossy(&shown));
    a.say("/stats bob").await;
    assert!(a.events().await.iter().any(|e| e.2 == "Ladder games: 1-0-0-0, rating 1025, rank 1"));
}

#[tokio::test]
async fn beginners_cannot_play_ladder_games() {
    let (mut a, _b, _, _) = two_players(false).await;
    let body = Writer::new().u32(4).u32(0).u16(LADDER).u16(1).u32(0).u32(0);
    a.send(0x08, body.cstr("Ladder1").cstr("").cstr("info")).await;
    assert_eq!(a.expect(0x08).await[0], 0);
}

#[tokio::test]
async fn ordinary_games_have_no_ratings() {
    let (mut a, mut b, _, _) = two_players(false).await;
    a.play(&mut b, "Melee1", MELEE, &[("Alice", 1), ("Bob", 2)]).await;
    let keys = ["Record\\W2BN\\0\\rating", "Record\\W2BN\\0\\wins"];
    assert_eq!(b.user_data("Alice", &keys).await, b"0\x001\0");
    assert_eq!(ladder(&mut a, 0, 0, 1).await[0].0, " ");
}

#[tokio::test]
async fn contradictory_results_do_not_count() {
    let (mut a, mut b, _, _) = two_players(false).await;
    a.play(&mut b, "Melee1", MELEE, &[("Alice", 1), ("Bob", 3)]).await;
    let keys = ["Record\\W2BN\\0\\wins", "Record\\W2BN\\0\\draws"];
    assert_eq!(b.user_data("Alice", &keys).await, b"0\x000\0");
    assert_eq!(b.user_data("Bob", &keys).await, b"0\x000\0");
}

#[tokio::test]
async fn friends_see_each_other() {
    let (mut a, mut b, _, _) = two_players(false).await;
    a.say("/f add nobody").await;
    assert!(a.events().await.iter().any(|e| e.2 == "That user does not exist."));
    a.say("/f add alice").await;
    assert!(a.events().await.iter().any(|e| e.2.contains("yourself")));
    a.say("/f add bob").await;
    let added = a.expect(0x67).await;
    assert!(added.starts_with(b"Bob\0\x00\x02NB2W") && added.ends_with(b"War2BNE-1\0"), "{added:?}");
    assert!(b.events().await.iter().any(|e| e.2.contains("Alice added you")));
    a.say("/f msg hi").await;
    assert!(a.events().await.iter().any(|e| e.2.contains("offline")), "not friends back yet");
    b.say("/friends add Alice").await;
    b.quiet().await;
    a.send(0x65, Writer::new()).await;
    assert!(a.expect(0x65).await.starts_with(b"\x01Bob\0\x01\x02NB2W"));
    b.say("/dnd busy").await;
    b.quiet().await;
    a.send(0x66, Writer::new().raw([0])).await;
    assert!(a.expect(0x66).await.starts_with(b"\x00\x03\x02NB2W"));
    b.say("/dnd").await;
    b.quiet().await;
    a.say("/f msg hi").await;
    assert!(b.events().await.iter().any(|e| e.0 == 4 && e.1 == "Alice" && e.2 == "hi"));
    a.say("/f remove bob").await;
    assert_eq!(a.expect(0x68).await, [0]);
    a.send(0x65, Writer::new()).await;
    assert_eq!(a.expect(0x65).await, [0]);
    a.send(0x66, Writer::new().raw([9])).await;
    assert!(a.quiet().await.is_empty());
}

#[tokio::test]
async fn topics_are_set_by_operators_and_kept() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}/t.db?mode=rwc", dir.path().display());
    let first = start(|config| config.database.url = url.clone()).await;
    let mut a = Client::chatter(first.address, "Alice").await;
    a.say("/topic mine").await;
    assert!(a.events().await.iter().any(|e| e.2.contains("at least a Channel Operator")));
    let alice = first.server.store.account("alice").await.unwrap().unwrap().id;
    first.server.store.set_role(alice, openbnet::store::Role::Admin).await.unwrap();
    first.server.hub.lock().set_role(alice, openbnet::store::Role::Admin);
    a.quiet().await;
    a.say("/topic For the Horde").await;
    assert!(a.events().await.iter().any(|e| e.2 == "War2BNE-1 topic: For the Horde"));
    let second = start(|config| config.database.url = url).await;
    let mut b = Client::connect(second.address).await;
    assert!(b.log_in("Alice", "secret").await);
    b.enter_chat().await;
    b.join("War2BNE").await;
    assert!(b.events().await.iter().any(|e| e.2 == "War2BNE-1 topic: For the Horde"), "after a restart");
}

#[tokio::test]
async fn banners_take_turns() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["one.pcx", "two.smk"] {
        std::fs::write(dir.path().join(name), b"x").unwrap();
    }
    let (address, _) = server(|config| {
        config.files.directory = dir.path().into();
        let ad = |file: &str, product: Option<&str>| openbnet::config::Ad {
            file: file.into(),
            url: format!("https://example.com/{file}"),
            product: product.map(Into::into),
        };
        config.ads = vec![ad("one.pcx", None), ad("star.pcx", Some("STAR")), ad("two.smk", Some("W2BN"))];
    })
    .await;
    let mut a = Client::chatter(address, "Alice").await;
    for (previous, number, extension) in [(0u32, 1, b".pcx"), (1, 3, b".smk"), (3, 1, b".pcx")] {
        a.send(0x15, Writer::new().raw(b"68XI").raw(b"NB2W").u32(previous).u32(0)).await;
        let ad = a.expect(0x15).await;
        assert_eq!((ad[0], &ad[4..8]), (number, &extension[..]));
    }
    a.send(0x41, Writer::new().u32(3)).await;
    assert!(a.expect(0x41).await.ends_with(b"https://example.com/two.smk\0"));
}

#[tokio::test]
async fn starcraft_logs_in_the_newer_way() {
    let (address, _) = server(|_| {}).await;
    let mut a = Client::connect_newer(address, b"STAR", 0xd3).await;
    a.send(0x3d, Writer::new().raw(Hash::of_password("secret").to_wire()).cstr("Kerrigan")).await;
    assert_eq!(a.expect(0x3d).await, [0; 4]);
    for (name, password, answer) in [("Nobody", "secret", 1), ("Kerrigan", "wrong", 2), ("Kerrigan", "secret", 0)]
    {
        let proof = Hash::of_password(password).proof(CLIENT_TOKEN, a.token);
        a.send(0x3a, Writer::new().u32(CLIENT_TOKEN).u32(a.token).raw(proof.to_wire()).cstr(name)).await;
        assert_eq!(a.expect(0x3a).await[0], answer, "{name} {password}");
    }
    let shown = a.enter_chat().await;
    assert_eq!(shown, b"Kerrigan\0RATS 0 0 0 0 0 0 0 0 RATS\0Kerrigan\0");
    a.join("Starcraft").await;
    assert!(a.events().await.iter().any(|e| e.0 == 7 && e.2 == "Starcraft-1"));
}

#[tokio::test]
async fn diablo_players_show_their_characters_and_see_every_game() {
    let (address, _) = server(|_| {}).await;
    let mut players = Vec::new();
    for name in ["Griswold", "Wirt"] {
        let mut player = Client::connect_newer(address, b"DRTL", 0x2a).await;
        assert!(player.create(name, "secret").await);
        assert!(player.log_in(name, "secret").await);
        players.push(player);
    }
    let mut b = players.pop().unwrap();
    let mut a = players.pop().unwrap();
    a.send(0x0a, Writer::new().cstr("Griswold").cstr("LTRD 30 1 2 100 50 60 70 8000 0")).await;
    assert!(a.expect(0x0a).await.windows(8).any(|w| w == b"LTRD 30 "));
    a.send(0x0a, Writer::new().cstr("").cstr("NB2W 9999 1 1 0 0 0 0 0")).await;
    assert!(a.expect(0x0a).await.windows(8).any(|w| w == b"LTRD 30 "), "only for their own game");
    let body = Writer::new().u32(4).u32(0).u16(3).u16(1).u32(0).u32(0);
    a.send(0x08, body.cstr("Cellar").cstr("").cstr("x")).await;
    assert_eq!(a.expect(0x08).await[0], 1);
    b.send(0x09, Writer::new().u16(1).u16(0).u32(0).u32(0).u32(25).cstr("").cstr("")).await;
    assert_eq!(b.expect(0x09).await[0], 1, "whatever difficulty they ask for");
    let mut w2 = Client::chatter(address, "Thrall").await;
    assert_eq!(w2.games("", "").await[0], 0, "games of other products aren't listed");
}

#[tokio::test]
async fn diablo_2_plays_on_open_battle_net() {
    let (address, _) = server(|_| {}).await;
    let mut players = Vec::new();
    for (name, product) in [("Deckard", b"D2XP"), ("Akara", b"D2XP"), ("Charsi", b"D2DV")] {
        let mut player = Client::connect_newer(address, product, 0x0e).await;
        player.send(0x3d, Writer::new().raw(Hash::of_password("secret").to_wire()).cstr(name)).await;
        assert_eq!(player.expect(0x3d).await, [0; 4]);
        let proof = Hash::of_password("secret").proof(CLIENT_TOKEN, player.token);
        player.send(0x3a, Writer::new().u32(CLIENT_TOKEN).u32(player.token).raw(proof.to_wire()).cstr(name)).await;
        assert_eq!(player.expect(0x3a).await, [0; 4]);
        players.push(player);
    }
    let [a, b, c] = &mut players[..] else { panic!() };
    for id in [0x34, 0x40] {
        a.send(id, Writer::new().u32(0).u32(0)).await;
        assert_eq!(a.expect(id).await, [0; 8], "no realms");
    }
    assert!(a.enter_chat().await.starts_with(b"Deckard\0PX2D\0"));
    a.join("Diablo II").await;
    assert!(a.events().await.iter().any(|e| e.0 == 7 && e.2 == "Diablo II-1"));
    let start = Writer::new().u16(0).u16(0).u32(0).u16(9).u16(0).u32(0).u32(0);
    a.send(0x1c, start.cstr("Act 1").cstr("").cstr("info")).await;
    assert_eq!(a.expect(0x1c).await, [0; 4]);
    b.send(0x09, Writer::new().u16(0).u16(0).u32(0).u32(0).u32(25).cstr("").cstr("")).await;
    assert_eq!(b.expect(0x09).await[0], 1);
    c.send(0x09, Writer::new().u16(0).u16(0).u32(0).u32(0).u32(25).cstr("").cstr("")).await;
    assert_eq!(c.expect(0x09).await[0], 0, "the expansion's games aren't the classic game's");
}
