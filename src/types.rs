use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ai_lib::metrics::Metrics;
use ai_lib::AiClient;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ClientInfo {
    pub name: String,
    pub client: Arc<AiClient>,
    pub default_model: String,
}

pub struct AppState {
    pub db: SqlitePool,
    pub pro: ClientInfo,
    pub con: ClientInfo,
    pub judge: ClientInfo,
    pub start_time: Instant,
    pub rate_limits: tokio::sync::RwLock<HashMap<String, Vec<Instant>>>,
    pub metrics: Arc<dyn Metrics>,
}

#[derive(Deserialize)]
pub struct DebateRequest {
    pub user_id: String,
    pub session_id: String,
    pub topic: String,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub user_id: String,
    pub session_id: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct HistoryMessage {
    pub role: String,
    pub phase: String,
    pub provider: Option<String>,
    pub content: String,
}

#[derive(Clone, Copy)]
pub enum Position {
    Pro,
    Con,
    Judge,
}

impl Position {
    pub fn role_str(&self) -> &'static str {
        match self {
            Position::Pro => "pro",
            Position::Con => "con",
            Position::Judge => "judge",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Position::Pro => "Pro",
            Position::Con => "Con",
            Position::Judge => "Judge",
        }
    }
}

#[derive(Clone, Copy)]
pub enum DebatePhase {
    Opening,
    Rebuttal,
    Defense,
    Closing,
    Judgement,
}

impl DebatePhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            DebatePhase::Opening => "opening",
            DebatePhase::Rebuttal => "rebuttal",
            DebatePhase::Defense => "defense",
            DebatePhase::Closing => "closing",
            DebatePhase::Judgement => "judgement",
        }
    }
    pub fn title(&self) -> &'static str {
        match self {
            DebatePhase::Opening => "一辩开篇",
            DebatePhase::Rebuttal => "二辩反驳",
            DebatePhase::Defense => "三辩防守",
            DebatePhase::Closing => "总结陈词",
            DebatePhase::Judgement => "裁判裁决",
        }
    }
}

/// Utility to pick client by side
pub fn client_for_side<'a>(state: &'a Arc<AppState>, side: Position) -> &'a ClientInfo {
    match side {
        Position::Pro => &state.pro,
        Position::Con => &state.con,
        Position::Judge => &state.judge,
    }
}

pub fn rate_limit_window() -> (Duration, usize) {
    // 8 requests / 10s window as before
    (Duration::from_secs(10), 8)
}


