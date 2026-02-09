use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ai_lib_rust::AiClient;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::app_metrics::Metrics;

#[derive(Clone)]
pub struct ClientInfo {
    pub name: String,
    pub model_id: String,
    pub client: Arc<AiClient>,
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
    /// Optional model override for Pro side (e.g. "deepseek/deepseek-chat")
    pub pro_model: Option<String>,
    /// Optional model override for Con side (e.g. "zhipu/glm-4-plus")
    pub con_model: Option<String>,
    /// Optional model override for Judge (e.g. "groq/llama-3.3-70b-versatile")
    pub judge_model: Option<String>,
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

/// Provider information returned by the /api/models endpoint.
#[derive(Serialize, Clone)]
pub struct AvailableProvider {
    pub provider: String,
    pub display_name: String,
    pub env_var: String,
    pub has_key: bool,
    pub models: Vec<AvailableModel>,
}

/// Model information returned by the /api/models endpoint.
#[derive(Serialize, Clone)]
pub struct AvailableModel {
    pub model_id: String,
    pub display_name: String,
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

pub fn rate_limit_window() -> (Duration, usize) {
    // 8 requests / 10s window
    (Duration::from_secs(10), 8)
}
