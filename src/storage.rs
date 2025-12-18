use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::str::FromStr;

use crate::types::{DebatePhase, HistoryMessage, Position};

pub async fn init_db(db_url: &str) -> anyhow::Result<SqlitePool> {
    // Ensure database file is created
    use sqlx::sqlite::SqliteConnectOptions;
    let opts = SqliteConnectOptions::from_str(db_url)?.create_if_missing(true);
    let db = SqlitePoolOptions::new().connect_with(opts).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS debate_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            phase TEXT NOT NULL,
            provider TEXT,
            content TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&db)
    .await?;

    Ok(db)
}

pub async fn save_message(
    db: &SqlitePool,
    user_id: &str,
    session_id: &str,
    role: Position,
    phase: DebatePhase,
    provider: Option<&str>,
    content: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO debate_messages (user_id, session_id, role, phase, provider, content) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(user_id)
    .bind(session_id)
    .bind(role.role_str())
    .bind(phase.as_str())
    .bind(provider)
    .bind(content)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn fetch_history(
    db: &SqlitePool,
    user_id: &str,
    session_id: &str,
) -> Vec<HistoryMessage> {
    let mut rows = sqlx::query_as::<_, HistoryMessage>(
        "SELECT role, phase, provider, content FROM debate_messages WHERE user_id = ?1 AND session_id = ?2 ORDER BY id DESC LIMIT 50",
    )
    .bind(user_id)
    .bind(session_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    rows.reverse();
    rows
}


