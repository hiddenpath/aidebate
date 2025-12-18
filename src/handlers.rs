use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, Method};
use axum::response::{Html, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::StreamExt;
use serde_json::json;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer};
use tracing::info;

use crate::app_metrics::SimpleMetrics;
use crate::debate::{execute_judge_round, execute_one_round};
use crate::storage::{fetch_history, save_message};
use crate::types::{
    client_for_side, AppState, DebatePhase, DebateRequest, HistoryMessage, HistoryQuery, Position,
};

/// Build the Axum router and shared state.
pub async fn build_app(
    db: sqlx::SqlitePool,
    clients: (
        crate::types::ClientInfo,
        crate::types::ClientInfo,
        crate::types::ClientInfo,
    ),
) -> Router {
    let (pro, con, judge) = clients;
    let state = Arc::new(AppState {
        db,
        pro,
        con,
        judge,
        start_time: Instant::now(),
        rate_limits: tokio::sync::RwLock::new(HashMap::new()),
        metrics: SimpleMetrics::new(),
    });

    Router::new()
        .route("/", get(index))
        .route("/js/marked.min.js", get(serve_marked_js))
        .route("/debate/stream", post(debate_stream))
        .route("/history", get(get_history).post(get_history_post))
        .route("/health", get(health))
        .layer(TimeoutLayer::new(Duration::from_secs(420)))
        .layer(
            CorsLayer::new()
                .allow_origin("*".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST])
                .allow_headers([axum::http::header::CONTENT_TYPE]),
        )
        .with_state(state)
}

pub async fn serve(listener: TcpListener, app: Router) -> anyhow::Result<()> {
    info!("🚀 ai-debate running at http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}

// --- HTTP handlers -----------------------------------------------------------

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn serve_marked_js() -> Response<String> {
    Response::builder()
        .header("Content-Type", "application/javascript")
        .header("Cache-Control", "public, max-age=86400")
        .body(include_str!("../static/js/marked.min.js").to_string())
        .unwrap()
}

async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "pro": state.pro.name,
        "con": state.con.name,
        "judge": state.judge.name,
    }))
}

async fn get_history(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HistoryQuery>,
) -> Json<serde_json::Value> {
    let rows: Vec<HistoryMessage> = fetch_history(&state.db, &q.user_id, &q.session_id).await;
    Json(json!({ "history": rows }))
}

async fn get_history_post(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DebateRequest>,
) -> Json<serde_json::Value> {
    let rows: Vec<HistoryMessage> =
        fetch_history(&state.db, &payload.user_id, &payload.session_id).await;
    Json(json!({ "history": rows }))
}

async fn debate_stream(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DebateRequest>,
) -> Response {
    let timer = state.metrics.start_timer("debate_stream").await;

    if is_rate_limited(&state, &payload.user_id).await {
        return sse_error("rate_limited", timer);
    }

    if payload.topic.trim().is_empty() || payload.topic.len() > 2000 {
        return sse_error("invalid_topic", timer);
    }

    let topic = payload.topic.clone();
    let user_id = payload.user_id.clone();
    let session_id = payload.session_id.clone();
    let state = state.clone();

    let stream = async_stream::stream! {
        yield sse_json(&json!({"type":"phase","phase":"init","message":"辩论开始"}));

        let mut transcript = Vec::new();

        // Four rounds: pro then con each phase
        let debate_phases = [
            DebatePhase::Opening,
            DebatePhase::Rebuttal,
            DebatePhase::Defense,
            DebatePhase::Closing,
        ];

        for phase in debate_phases {
            for side in [Position::Pro, Position::Con] {
                let client_info = client_for_side(&state, side);

                // Send phase start event
                yield sse_json(&json!({"type":"phase_start","phase":phase.as_str(),"side":side.role_str(),"title":phase.title(),"provider":client_info.name}));

                // Execute the round
                match execute_one_round(&state, side, phase, &topic, &transcript).await {
                    Ok((mut stream, provider)) => {
                        let mut full_content = String::new();

                        // Stream the content as deltas for UI updates
                        while let Some(chunk_res) = stream.next().await {
                            match chunk_res {
                                Ok(delta) => {
                                    if !delta.is_empty() {
                                        yield sse_json(&json!({"type":"delta","side":side.role_str(),"phase":phase.as_str(),"provider":provider,"content":delta}));
                                        full_content.push_str(&delta);
                                    }
                                }
                                Err(e) => {
                                    yield sse_json(&json!({"type":"error","message": format!("Stream error: {}", e)}));
                                    // Don't break immediately, maybe try to salvage what we have?
                                    // For now, simple return is safer to stop broken state.
                                    return;
                                }
                            }
                        }

                        transcript.push((side, phase, full_content.clone(), provider.clone()));
                        let _ = save_message(&state.db, &user_id, &session_id, side, phase, Some(&provider), &full_content).await;
                        yield sse_json(&json!({"type":"phase_done","phase":phase.as_str(),"side":side.role_str(),"provider":provider}));
                    }
                    Err(e) => {
                        yield sse_json(&json!({"type":"error","message": format!("辩论轮次失败: {}", e)}));
                        return;
                    }
                }
            }
        }

        // Judge round
        {
            let judge_info = &state.judge;
            yield sse_json(&json!({"type":"phase_start","phase":"judgement","side":"judge","title":DebatePhase::Judgement.title(),"provider":judge_info.name}));

            match execute_judge_round(&state, &topic, &transcript).await {
                Ok((content, provider)) => {
                    // Stream the judge content as deltas
                    for chunk in content.chars().collect::<Vec<char>>().chunks(10) {
                        let delta: String = chunk.iter().collect();
                        yield sse_json(&json!({"type":"delta","side":"judge","phase":"judgement","provider":provider,"content":delta}));
                    }

                    transcript.push((Position::Judge, DebatePhase::Judgement, content.clone(), provider.clone()));
                    let _ = save_message(&state.db, &user_id, &session_id, Position::Judge, DebatePhase::Judgement, Some(&provider), &content).await;
                    yield sse_json(&json!({"type":"phase_done","phase":"judgement","side":"judge","provider":provider}));
                }
                Err(e) => {
                    yield sse_json(&json!({"type":"error","message": format!("裁判阶段失败: {}", e)}));
                    return;
                }
            }
        }

        yield "data: {\"type\":\"done\"}\n\n".to_string();
    };

    let body_stream = stream.map(|chunk| Ok::<_, std::io::Error>(chunk));
    if let Some(t) = timer {
        t.stop();
    }
    Response::builder()
        .status(200)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .body(Body::from_stream(body_stream))
        .unwrap()
}

// --- Helpers ----------------------------------------------------------------

async fn is_rate_limited(state: &Arc<AppState>, user_id: &str) -> bool {
    let now = Instant::now();
    let (window, max_requests) = crate::types::rate_limit_window();
    let mut guard = state.rate_limits.write().await;
    let entry = guard.entry(user_id.to_string()).or_insert_with(Vec::new);
    entry.retain(|t| now.duration_since(*t) < window);
    if entry.len() >= max_requests {
        true
    } else {
        entry.push(now);
        false
    }
}

fn sse_error(msg: &str, timer: Option<Box<MetricsTimer>>) -> Response {
    if let Some(t) = timer {
        t.stop();
    }
    Response::builder()
        .status(200)
        .header("Content-Type", "text/event-stream")
        .body(Body::from(format!(
            "data: {{\"type\":\"error\",\"message\":\"{}\"}}\n\n",
            msg
        )))
        .unwrap()
}

fn sse_json(v: &serde_json::Value) -> String {
    format!("data: {}\n\n", v.to_string())
}

// Small alias to avoid retyping trait object type.
type MetricsTimer = dyn ai_lib::metrics::Timer + Send;
