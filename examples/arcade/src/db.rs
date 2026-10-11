use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub struct ScoreRow {
    pub username: String,
    pub score: i64,
    pub played_at: String,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open() -> Result<Self> {
        let path = db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("create arcade data dir")?;
        }
        let conn = Connection::open(&path).context("open sqlite db")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS scores (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL,
                game TEXT NOT NULL,
                score INTEGER NOT NULL,
                played_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_scores_game_score ON scores (game, score DESC);",
        )
        .context("create scores table")?;
        Ok(Self { conn })
    }

    pub fn save_score(&self, username: &str, game: &str, score: i64) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO scores (username, game, score, played_at) VALUES (?1, ?2, ?3, ?4)",
                params![username, game, score, Utc::now().to_rfc3339()],
            )
            .context("insert score")?;
        Ok(())
    }

    /// Best score per user for a game, top `limit` rows.
    pub fn top_for_game(&self, game: &str, limit: i64) -> Result<Vec<ScoreRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT username, MAX(score) as score, MAX(played_at) as played_at
             FROM scores WHERE game = ?1 GROUP BY username ORDER BY score DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![game, limit], |row| {
                Ok(ScoreRow {
                    username: row.get(0)?,
                    score: row.get(1)?,
                    played_at: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn best(&self, username: &str, game: &str) -> Result<i64> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(MAX(score), 0) FROM scores WHERE username = ?1 AND game = ?2",
        )?;
        let v: i64 = stmt.query_row(params![username, game], |r| r.get(0))?;
        Ok(v)
    }
}

fn db_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("SSHOXY_ARCADE_DB") {
        return Ok(PathBuf::from(p));
    }
    let dirs = directories::ProjectDirs::from("com", "sshoxy", "arcade")
        .context("resolve project data dir")?;
    Ok(dirs.data_dir().join("leaderboard.db"))
}

/// Username the player logged in with. sshoxy exports SSHOXY_USER.
pub fn username() -> String {
    for key in ["SSHOXY_USER", "USER", "USERNAME"] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return v;
            }
        }
    }
    "player".to_string()
}
