//! The database queries against a real PostgreSQL. See docs/TESTING.md.

use openbnet::store::{Created, FinishedGame, LadderOrder, Outcome, Role, Store};
use openbnet::wire::product::WARCRAFT2;
use openbnet::xsha1::Hash;

#[tokio::test]
#[ignore = "needs an empty PostgreSQL database"]
async fn postgresql_works_like_sqlite() {
    let url = std::env::var("OPENBNET_POSTGRES").expect("OPENBNET_POSTGRES");
    let store = Store::open(&url).await.unwrap();
    let hash = Hash::of(b"x");
    let Created::Yes(a) = store.create_account("Alice", hash).await.unwrap() else { panic!() };
    let Created::Yes(b) = store.create_account("Bob", hash).await.unwrap() else { panic!() };
    assert_eq!(store.create_account("ALICE", hash).await.unwrap(), Created::NameTaken);
    assert_eq!(store.password(a).await.unwrap(), Some(hash));
    store.set_password(a, Hash::of(b"y")).await.unwrap();
    assert_eq!(store.password(a).await.unwrap(), Some(Hash::of(b"y")));
    store.set_role(a, Role::Admin).await.unwrap();
    store.set_locked(a, true).await.unwrap();
    store.set_muted(a, true).await.unwrap();
    store.note_login(a, WARCRAFT2, "192.0.2.1").await.unwrap();
    let account = store.account("alice").await.unwrap().unwrap();
    assert_eq!((account.role, account.locked, account.muted), (Role::Admin, true, true));
    assert!(account.last_login_at > 0);
    store.set_profile(a, "sex", "orc").await.unwrap();
    store.set_profile(a, "sex", "\u{e9}lan").await.unwrap();
    assert_eq!(store.profile(a).await.unwrap()["sex"], "\u{e9}lan");
    let game = FinishedGame {
        product: WARCRAFT2,
        name: "game".into(),
        game_type: 2,
        ladder: 0,
        started_at: 1,
        results: vec![(a, Outcome::Win), (b, Outcome::Loss)],
        reports: vec![(a, b, Outcome::Loss)],
    };
    store.finish_game(&game).await.unwrap();
    let ladder_game = FinishedGame { ladder: 1, game_type: 9, ..game.clone() };
    store.finish_game(&ladder_game).await.unwrap();
    for order in [LadderOrder::Rating, LadderOrder::Wins, LadderOrder::Games] {
        let list = store.ladder(WARCRAFT2, 1, order, 0, 10).await.unwrap();
        assert_eq!((list[0].0.as_str(), list[0].1.rating, list[0].1.rank), ("Alice", 1025, 1));
        assert_eq!(store.ladder_place(WARCRAFT2, 1, order, "BOB").await.unwrap(), Some(1));
    }
    assert!(store.add_friend(a, b).await.unwrap());
    assert!(!store.add_friend(a, b).await.unwrap());
    assert!(!store.friends(a).await.unwrap()[0].mutual);
    store.remove_friend(a, b).await.unwrap();
    assert!(store.friends(a).await.unwrap().is_empty());
    store.set_topic("Chat", "one").await.unwrap();
    store.set_topic("chat", "two").await.unwrap();
    assert_eq!(store.topics().await.unwrap(), [("chat".to_owned(), "two".to_owned())]);
    drop(store);
    // Ids carry on after a restart.
    let store = Store::open(&url).await.unwrap();
    store.finish_game(&game).await.unwrap();
    assert_eq!(store.records(a, WARCRAFT2).await.unwrap()[&0].wins, 2);
    assert!(matches!(store.create_account("Carol", hash).await.unwrap(), Created::Yes(id) if id > b));
}
