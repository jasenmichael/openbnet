//! Everything kept between runs. SQLite by default, PostgreSQL optionally, with the same schema and queries.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use sqlx::any::{AnyPoolOptions, AnyRow};
use sqlx::{AnyPool, Row};

use crate::ladder;
use crate::wire::Tag;
use crate::xsha1::Hash;

pub type AccountId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Player,
    Moderator,
    Admin,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Player => "player",
            Role::Moderator => "moderator",
            Role::Admin => "admin",
        }
    }

    pub fn parse(text: &str) -> Option<Role> {
        [Role::Player, Role::Moderator, Role::Admin].into_iter().find(|role| role.name() == text)
    }
}

#[derive(Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub role: Role,
    pub locked: bool,
    pub muted: bool,
    pub last_login_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Win,
    Loss,
    Draw,
    Disconnect,
}

impl Outcome {
    pub fn name(self) -> &'static str {
        match self {
            Outcome::Win => "WIN",
            Outcome::Loss => "LOSS",
            Outcome::Draw => "DRAW",
            Outcome::Disconnect => "DISCONNECT",
        }
    }
}

/// An account's results in one game and ladder. Ladder 0 is ordinary games.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub wins: i64,
    pub losses: i64,
    pub draws: i64,
    pub disconnects: i64,
    pub rating: i64,
    pub high_rating: i64,
    /// Place on the ladder, from 1. 0 means none.
    pub rank: i64,
    pub last_result: String,
    pub last_game_at: i64,
}

#[derive(Debug, Clone)]
pub struct FinishedGame {
    pub product: Tag,
    pub name: String,
    pub game_type: u16,
    pub ladder: i64,
    pub started_at: i64,
    pub results: Vec<(AccountId, Outcome)>,
    /// Who reported what about whom.
    pub reports: Vec<(AccountId, AccountId, Outcome)>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Created {
    Yes(AccountId),
    NameTaken,
}

pub struct Store {
    pool: AnyPool,
    next_game: AtomicI64,
}

impl Store {
    pub async fn open(url: &str) -> Result<Store> {
        sqlx::any::install_default_drivers();
        let sqlite = url.starts_with("sqlite");
        // SQLite allows one writer, and each in-memory connection would be its own database.
        let mut options = AnyPoolOptions::new().max_connections(if sqlite { 1 } else { 8 });
        if sqlite {
            // An in-memory database goes with its connection.
            options = options.idle_timeout(None).max_lifetime(None).after_connect(|connection, _| {
                Box::pin(async move {
                    // Safe with WAL: a power cut can lose the last moments, but can't damage the file.
                    for setting in ["journal_mode = WAL", "synchronous = NORMAL", "foreign_keys = ON"] {
                        sqlx::query(&format!("PRAGMA {setting}")).execute(&mut *connection).await?;
                    }
                    Ok(())
                })
            });
        }
        let pool = options.connect(url).await?;
        sqlx::migrate!().run(&pool).await?;
        let next = async |sql: &'static str| -> Result<AtomicI64> {
            Ok(AtomicI64::new(sqlx::query_scalar::<_, i64>(sql).fetch_one(&pool).await? + 1))
        };
        Ok(Store { next_game: next("SELECT COALESCE(MAX(id), 0) FROM games").await?, pool })
    }

    pub async fn create_account(&self, name: &str, password: Hash) -> Result<Created> {
        create_account(self.pool.clone(), name.to_owned(), password).await
    }

    pub async fn account(&self, name: &str) -> Result<Option<Account>> {
        let row =
            sqlx::query("SELECT id, name, role, locked, muted, last_login_at FROM accounts WHERE name_key = $1")
                .bind(name.to_ascii_lowercase())
                .fetch_optional(&self.pool)
                .await?;
        row.map(|row| read_account(&row)).transpose()
    }

    /// The stored password hash. A damaged one counts as missing, so it can never be matched.
    pub async fn password(&self, account: AccountId) -> Result<Option<Hash>> {
        let data: Option<String> =
            sqlx::query_scalar("SELECT data FROM credentials WHERE account_id = $1 AND scheme = 'xsha1'")
                .bind(account)
                .fetch_optional(&self.pool)
                .await?;
        Ok(data.as_deref().and_then(Hash::parse))
    }

    pub async fn set_password(&self, account: AccountId, password: Hash) -> Result<()> {
        sqlx::query(
            "INSERT INTO credentials (account_id, scheme, data) VALUES ($1, 'xsha1', $2) \
             ON CONFLICT (account_id, scheme) DO UPDATE SET data = excluded.data",
        )
        .bind(account)
        .bind(password.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Damages a stored password, as a bad import or manual edit could.
    #[doc(hidden)]
    pub async fn break_password_for_test(&self, name: &str) {
        let account = self.account(name).await.expect("database").expect("account");
        sqlx::query("UPDATE credentials SET data = 'broken' WHERE account_id = $1")
            .bind(account.id)
            .execute(&self.pool)
            .await
            .expect("database");
    }

    pub async fn set_role(&self, account: AccountId, role: Role) -> Result<()> {
        sqlx::query("UPDATE accounts SET role = $1 WHERE id = $2")
            .bind(role.name())
            .bind(account)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_locked(&self, account: AccountId, locked: bool) -> Result<()> {
        sqlx::query("UPDATE accounts SET locked = $1 WHERE id = $2")
            .bind(i64::from(locked))
            .bind(account)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_muted(&self, account: AccountId, muted: bool) -> Result<()> {
        sqlx::query("UPDATE accounts SET muted = $1 WHERE id = $2")
            .bind(i64::from(muted))
            .bind(account)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn note_login(&self, account: AccountId, product: Tag, ip: &str) -> Result<()> {
        sqlx::query(
            "UPDATE accounts SET last_login_at = $1, last_login_product = $2, last_login_ip = $3 WHERE id = $4",
        )
        .bind(unix_now())
        .bind(product.to_string())
        .bind(ip)
        .bind(account)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn profile(&self, account: AccountId) -> Result<HashMap<String, String>> {
        let rows = sqlx::query("SELECT name, content FROM profiles WHERE account_id = $1")
            .bind(account)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(|row| Ok((row.try_get(0)?, row.try_get(1)?))).collect()
    }

    pub async fn set_profile(&self, account: AccountId, name: &str, content: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO profiles (account_id, name, content) VALUES ($1, $2, $3) \
             ON CONFLICT (account_id, name) DO UPDATE SET content = excluded.content",
        )
        .bind(account)
        .bind(name)
        .bind(content)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// An account's records in one game, by ladder.
    pub async fn records(&self, account: AccountId, product: Tag) -> Result<HashMap<i64, Record>> {
        let sql = format!("SELECT ladder, {RECORD} FROM records r WHERE account_id = $1 AND product = $2");
        let rows = sqlx::query(&sql).bind(account).bind(product.to_string()).fetch_all(&self.pool).await?;
        rows.iter().map(|row| Ok((row.try_get(0)?, read_record(row, 1)?))).collect()
    }

    /// A stretch of a ladder, best first. `start` counts from 0.
    pub async fn ladder(
        &self,
        product: Tag,
        ladder: i64,
        order: LadderOrder,
        start: i64,
        count: i64,
    ) -> Result<Vec<(String, Record)>> {
        let sql = format!(
            "SELECT a.name, {RECORD} FROM records r JOIN accounts a ON a.id = r.account_id \
             WHERE r.product = $1 AND r.ladder = $2 AND r.rating > 0 AND {GAMES} > 0 \
             ORDER BY {}, r.account_id LIMIT $3 OFFSET $4",
            order.sql()
        );
        let rows = sqlx::query(&sql)
            .bind(product.to_string())
            .bind(ladder)
            .bind(count)
            .bind(start)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(|row| Ok((row.try_get(0)?, read_record(row, 1)?))).collect()
    }

    /// Where an account is on a ladder, from 0.
    pub async fn ladder_place(
        &self,
        product: Tag,
        ladder: i64,
        order: LadderOrder,
        name: &str,
    ) -> Result<Option<i64>> {
        let sql = format!(
            "SELECT place FROM (SELECT a.name_key AS name_key, ROW_NUMBER() OVER (ORDER BY {}, r.account_id) AS place \
             FROM records r JOIN accounts a ON a.id = r.account_id \
             WHERE r.product = $1 AND r.ladder = $2 AND r.rating > 0 AND {GAMES} > 0) places WHERE name_key = $3",
            order.sql()
        );
        let place: Option<i64> = sqlx::query_scalar(&sql)
            .bind(product.to_string())
            .bind(ladder)
            .bind(name.to_ascii_lowercase())
            .fetch_optional(&self.pool)
            .await?;
        Ok(place.map(|place| place - 1))
    }

    /// Stores a game, what each player reported, and the players' new totals and ratings.
    pub async fn finish_game(&self, game: &FinishedGame) -> Result<()> {
        let now = unix_now();
        let id = self.next_game.fetch_add(1, Ordering::Relaxed);
        let mut players = Vec::new();
        for (account, outcome) in &game.results {
            let mut record = self.records(*account, game.product).await?.remove(&game.ladder).unwrap_or_default();
            match outcome {
                Outcome::Win => record.wins += 1,
                Outcome::Loss => record.losses += 1,
                Outcome::Draw => record.draws += 1,
                Outcome::Disconnect => record.disconnects += 1,
            }
            if game.ladder > 0 && record.rating == 0 {
                record.rating = ladder::INITIAL_RATING;
            }
            players.push((record, *outcome));
        }
        if game.ladder > 0 {
            for (index, change) in ladder::changes(&players).into_iter().enumerate() {
                let record = &mut players[index].0;
                // A rating of 0 would mean none.
                record.rating = (record.rating + change).max(1);
                record.high_rating = record.high_rating.max(record.rating);
            }
        }
        write_game(self.pool.clone(), id, now, game.clone(), players).await
    }

    /// An account's friends, in the order they were added.
    pub async fn friends(&self, account: AccountId) -> Result<Vec<Friend>> {
        let rows = sqlx::query(
            "SELECT a.id, a.name, (SELECT COUNT(*) FROM friends back \
             WHERE back.account_id = f.friend_id AND back.friend_id = f.account_id) \
             FROM friends f JOIN accounts a ON a.id = f.friend_id \
             WHERE f.account_id = $1 ORDER BY f.added_at, a.id",
        )
        .bind(account)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(Friend {
                    account: row.try_get(0)?,
                    name: row.try_get(1)?,
                    mutual: row.try_get::<i64, _>(2)? > 0,
                })
            })
            .collect()
    }

    /// False if they already were a friend.
    pub async fn add_friend(&self, account: AccountId, friend: AccountId) -> Result<bool> {
        let added = sqlx::query(
            "INSERT INTO friends (account_id, friend_id, added_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(account)
        .bind(friend)
        .bind(unix_now())
        .execute(&self.pool)
        .await?;
        Ok(added.rows_affected() > 0)
    }

    pub async fn remove_friend(&self, account: AccountId, friend: AccountId) -> Result<()> {
        sqlx::query("DELETE FROM friends WHERE account_id = $1 AND friend_id = $2")
            .bind(account)
            .bind(friend)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The topics kept for permanent channels, by lower-case name.
    pub async fn topics(&self) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query("SELECT channel, topic FROM topics").fetch_all(&self.pool).await?;
        rows.iter().map(|row| Ok((row.try_get(0)?, row.try_get(1)?))).collect()
    }

    pub async fn set_topic(&self, channel: &str, topic: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO topics (channel, topic) VALUES ($1, $2) \
             ON CONFLICT (channel) DO UPDATE SET topic = excluded.topic",
        )
        .bind(channel.to_ascii_lowercase())
        .bind(topic)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

async fn create_account(pool: AnyPool, name: String, password: Hash) -> Result<Created> {
    let key = name.to_ascii_lowercase();
    // The command line may be making accounts too, so ids come from the database. Two at once can pick the same
    // one; then the loser tries again.
    for _ in 0..10 {
        let mut tx = pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO accounts (id, name, name_key, created_at) \
                 SELECT COALESCE(MAX(id), 0) + 1, $1, $2, $3 FROM accounts WHERE 1 = 1 ON CONFLICT DO NOTHING",
        )
        .bind(&name)
        .bind(&key)
        .bind(unix_now())
        .execute(&mut *tx)
        .await?;
        let id: Option<AccountId> = sqlx::query_scalar("SELECT id FROM accounts WHERE name_key = $1")
            .bind(&key)
            .fetch_optional(&mut *tx)
            .await?;
        match id {
            Some(id) if inserted.rows_affected() > 0 => {
                sqlx::query("INSERT INTO credentials (account_id, scheme, data) VALUES ($1, 'xsha1', $2)")
                    .bind(id)
                    .bind(password.to_string())
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                return Ok(Created::Yes(id));
            }
            Some(_) => return Ok(Created::NameTaken),
            None => {}
        }
    }
    anyhow::bail!("could not number a new account")
}

async fn write_game(
    pool: AnyPool,
    id: i64,
    now: i64,
    game: FinishedGame,
    players: Vec<(Record, Outcome)>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO games (id, product, name, game_type, ladder, started_at, ended_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(game.product.to_string())
    .bind(&game.name)
    .bind(i64::from(game.game_type))
    .bind(game.ladder)
    .bind(game.started_at)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    for ((account, outcome), (record, _)) in game.results.iter().zip(&players) {
        sqlx::query("INSERT INTO game_players (game_id, account_id, result) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(account)
            .bind(outcome.name())
            .execute(&mut *tx)
            .await?;
        sqlx::query(
                "INSERT INTO records (account_id, product, ladder, wins, losses, draws, disconnects, rating, \
                 high_rating, last_result, last_game_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
                 ON CONFLICT (account_id, product, ladder) DO UPDATE SET \
                 wins = excluded.wins, losses = excluded.losses, draws = excluded.draws, \
                 disconnects = excluded.disconnects, rating = excluded.rating, high_rating = excluded.high_rating, \
                 last_result = excluded.last_result, last_game_at = excluded.last_game_at",
            )
            .bind(account)
            .bind(game.product.to_string())
            .bind(game.ladder)
            .bind(record.wins)
            .bind(record.losses)
            .bind(record.draws)
            .bind(record.disconnects)
            .bind(record.rating)
            .bind(record.high_rating)
            .bind(outcome.name())
            .bind(now)
            .execute(&mut *tx)
            .await?;
    }
    for (reporter, subject, outcome) in &game.reports {
        sqlx::query("INSERT INTO game_reports (game_id, reporter_id, subject_id, result) VALUES ($1, $2, $3, $4)")
            .bind(id)
            .bind(reporter)
            .bind(subject)
            .bind(outcome.name())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

const GAMES: &str = "(r.wins + r.losses + r.draws + r.disconnects)";

/// A record's columns, with its rank, for `read_record`.
const RECORD: &str = "r.wins, r.losses, r.draws, r.disconnects, r.rating, r.high_rating, r.last_result, \
    r.last_game_at, CASE WHEN r.rating > 0 THEN 1 + (SELECT COUNT(*) FROM records o \
    WHERE o.product = r.product AND o.ladder = r.ladder AND o.rating > r.rating) ELSE 0 END";

fn read_record(row: &AnyRow, first: usize) -> Result<Record> {
    Ok(Record {
        wins: row.try_get(first)?,
        losses: row.try_get(first + 1)?,
        draws: row.try_get(first + 2)?,
        disconnects: row.try_get(first + 3)?,
        rating: row.try_get(first + 4)?,
        high_rating: row.try_get(first + 5)?,
        last_result: row.try_get(first + 6)?,
        last_game_at: row.try_get(first + 7)?,
        rank: row.try_get(first + 8)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderOrder {
    Rating,
    Wins,
    Games,
}

impl LadderOrder {
    fn sql(self) -> String {
        match self {
            LadderOrder::Rating => "r.rating DESC, r.wins DESC".into(),
            LadderOrder::Wins => "r.wins DESC, r.rating DESC".into(),
            LadderOrder::Games => format!("{GAMES} DESC, r.rating DESC"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Friend {
    pub account: AccountId,
    pub name: String,
    pub mutual: bool,
}

fn read_account(row: &AnyRow) -> Result<Account> {
    let role: String = row.try_get(2)?;
    Ok(Account {
        id: row.try_get(0)?,
        name: row.try_get(1)?,
        role: Role::parse(&role).unwrap_or(Role::Player),
        locked: row.try_get::<i64, _>(3)? != 0,
        muted: row.try_get::<i64, _>(4)? != 0,
        last_login_at: row.try_get(5)?,
    })
}

pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::product::WARCRAFT2;

    #[tokio::test]
    async fn names_are_unique_whatever_their_case() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        let hash = Hash::of(b"x");
        assert!(matches!(store.create_account("Thrall", hash).await.unwrap(), Created::Yes(_)));
        assert_eq!(store.create_account("THRALL", hash).await.unwrap(), Created::NameTaken);
        assert_eq!(store.account("thrall").await.unwrap().unwrap().name, "Thrall");
    }

    #[tokio::test]
    async fn accounts_can_be_made_while_the_server_runs() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}/t.db?mode=rwc", dir.path().display());
        let server = Store::open(&url).await.unwrap();
        let command = Store::open(&url).await.unwrap();
        for (store, name) in [(&command, "admin"), (&server, "one"), (&command, "two"), (&server, "three")] {
            assert!(
                matches!(store.create_account(name, Hash::of(b"x")).await.unwrap(), Created::Yes(_)),
                "{name}"
            );
        }
    }

    #[tokio::test]
    async fn a_damaged_password_is_never_usable() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        let Created::Yes(id) = store.create_account("orc", Hash::of(b"x")).await.unwrap() else { panic!() };
        sqlx::query("UPDATE credentials SET data = 'broken'").execute(&store.pool).await.unwrap();
        assert_eq!(store.password(id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn finished_games_add_to_records() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        let Created::Yes(a) = store.create_account("a1", Hash::of(b"x")).await.unwrap() else { panic!() };
        let Created::Yes(b) = store.create_account("b1", Hash::of(b"x")).await.unwrap() else { panic!() };
        let game = FinishedGame {
            product: WARCRAFT2,
            name: "game".into(),
            game_type: 2,
            ladder: 0,
            started_at: 1,
            results: vec![(a, Outcome::Win), (b, Outcome::Loss)],
            reports: vec![(a, b, Outcome::Loss), (b, a, Outcome::Win)],
        };
        store.finish_game(&game).await.unwrap();
        store.finish_game(&game).await.unwrap();
        assert_eq!(store.records(a, WARCRAFT2).await.unwrap()[&0].wins, 2);
        assert_eq!(store.records(b, WARCRAFT2).await.unwrap()[&0].losses, 2);
    }
}
