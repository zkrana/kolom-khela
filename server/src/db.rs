use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn connect(path: &str) -> SqlitePool {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{path}"))
        .expect("bad DB_PATH")
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .expect("cannot open the database")
}

pub async fn migrate(pool: &SqlitePool) {
    let stmts = [
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            pass_hash TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'player',
            blocked INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS sessions (
            token_hash TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            created_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS games (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            code TEXT NOT NULL,
            finished_at INTEGER NOT NULL,
            winner_id INTEGER,
            loser_id INTEGER
        )",
        "CREATE TABLE IF NOT EXISTS game_players (
            game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            coins INTEGER NOT NULL,
            rank INTEGER NOT NULL,
            looser INTEGER NOT NULL DEFAULT 0
        )",
        "CREATE INDEX IF NOT EXISTS idx_game_players_user ON game_players(user_id)",
        "CREATE TABLE IF NOT EXISTS avatars (
            user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
            mime TEXT NOT NULL,
            data BLOB NOT NULL,
            updated_at INTEGER NOT NULL
        )",
    ];
    for s in stmts {
        sqlx::query(s).execute(pool).await.expect("migration failed");
    }
    // Columns added after the first release. Running it twice is harmless: a duplicate column just errors and is ignored.
    for s in [
        "ALTER TABLE users ADD COLUMN coins INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE users ADD COLUMN pen TEXT NOT NULL DEFAULT 'econo'",
        "ALTER TABLE users ADD COLUMN pens TEXT NOT NULL DEFAULT 'econo'",
        "ALTER TABLE users ADD COLUMN guest INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE users ADD COLUMN contact TEXT",
    ] {
        let _ = sqlx::query(s).execute(pool).await;
    }
}

pub struct PlayerResult {
    pub user_id: i64,
    pub name: String,
    pub coins: i64,
    pub rank: i64,
    pub looser: bool,
}

/// Coins for a win, with a daily cap so two accounts cannot farm each other. Returns what was actually paid.
pub async fn reward_win(pool: &SqlitePool, user_id: i64) -> i64 {
    let recent: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE winner_id = ? AND finished_at > ?")
        .bind(user_id)
        .bind(now() - 86_400)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    if recent >= crate::pens::DAILY_REWARDED_WINS {
        return 0;
    }
    let _ = sqlx::query("UPDATE users SET coins = coins + ? WHERE id = ?").bind(crate::pens::WIN_REWARD).bind(user_id).execute(pool).await;
    crate::pens::WIN_REWARD
}

pub async fn save_game(pool: &SqlitePool, code: &str, results: &[PlayerResult]) {
    let winner = results.iter().find(|r| r.rank == 1).map(|r| r.user_id);
    let loser = results.iter().find(|r| r.looser).map(|r| r.user_id);
    let Ok(row) = sqlx::query("INSERT INTO games (code, finished_at, winner_id, loser_id) VALUES (?, ?, ?, ?)")
        .bind(code)
        .bind(now())
        .bind(winner)
        .bind(loser)
        .execute(pool)
        .await
    else {
        return;
    };
    let gid = row.last_insert_rowid();
    for r in results {
        let _ = sqlx::query("INSERT INTO game_players (game_id, user_id, name, coins, rank, looser) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(gid)
            .bind(r.user_id)
            .bind(&r.name)
            .bind(r.coins)
            .bind(r.rank)
            .bind(r.looser as i64)
            .execute(pool)
            .await;
    }
}
