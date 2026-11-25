//! Many players at once. Run in release mode; see docs/TESTING.md.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use common::{Client, server};
use tokio::sync::Barrier;

async fn crowd(players: usize) {
    const PER_CHANNEL: usize = 40;
    const LINES: usize = 5;
    let directory = tempfile::tempdir().unwrap();
    let database = format!("sqlite://{}/load.db?mode=rwc", directory.path().display());
    let (address, server) = server(|config| {
        config.server.max_connections = players + 10;
        config.database.url = database;
    })
    .await;
    let started = Instant::now();
    let barrier = Arc::new(Barrier::new(players + 1));
    let heard = Arc::new(AtomicU64::new(0));
    let slowest = Arc::new(AtomicU64::new(0));
    let mut tasks = Vec::new();
    for n in 0..players {
        let (barrier, heard, slowest) = (barrier.clone(), heard.clone(), slowest.clone());
        tasks.push(tokio::spawn(async move {
            let mut client = Client::player(address, &format!("player{n}")).await;
            client.enter_chat().await;
            client.join(&format!("Room {}", n / PER_CHANNEL)).await;
            client.expect(0x0f).await;
            barrier.wait().await;
            barrier.wait().await;
            for line in 0..LINES {
                client.say(&format!("line {line} from {n}")).await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            let wanted = (PER_CHANNEL - 1) * LINES;
            let mut count = 0;
            while count < wanted {
                let Some((id, body)) = client.receive(Duration::from_secs(20)).await else { break };
                if id == 0x0f && body[0] == 5 {
                    count += 1;
                }
            }
            heard.fetch_add(count as u64, Ordering::Relaxed);
            // A round trip while everyone else is busy.
            let asked = Instant::now();
            client.games("", "").await;
            slowest.fetch_max(asked.elapsed().as_millis() as u64, Ordering::Relaxed);
        }));
    }
    if tokio::time::timeout(Duration::from_secs(60), barrier.wait()).await.is_err() {
        panic!("only {} of {players} players got in", server.hub.lock().user_count());
    }
    println!("{players} players in after {:?}", started.elapsed());
    assert_eq!(server.hub.lock().user_count(), players);
    barrier.wait().await;
    for task in tasks {
        task.await.unwrap();
    }
    let expected = (players * (PER_CHANNEL - 1) * LINES) as u64;
    println!("{} of {expected} lines delivered, slowest reply {:?} ms", heard.load(Ordering::Relaxed), slowest);
    assert_eq!(heard.load(Ordering::Relaxed), expected);
    assert!(slowest.load(Ordering::Relaxed) < 1000, "replies were slow");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "takes a while and many sockets"]
async fn eight_hundred_players() {
    crowd(800).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "takes a while and many sockets"]
async fn two_thousand_players() {
    crowd(2000).await;
}
