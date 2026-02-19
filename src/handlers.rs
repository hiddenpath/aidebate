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

use crate::app_metrics::{SimpleMetrics, Timer};
use crate::config;
use crate::debate::{
    execute_judge_round_stream, execute_one_round, execute_round_with_tools, DebateStreamChunk,
};
use crate::storage::{fetch_history, save_message};
use crate::tools;
use crate::types::{
    AppState, ClientInfo, DebatePhase, DebateRequest, HistoryMessage, HistoryQuery, Position,
};

/// Build the Axum router and shared state.
pub async fn build_app(
    db: sqlx::SqlitePool,
    clients: (ClientInfo, ClientInfo, ClientInfo),
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
        .route("/favicon.svg", get(serve_favicon))
        .route("/api/models", get(get_models))
        .route("/debate/stream", post(debate_stream))
        .route("/history", get(get_history).post(get_history_post))
        .route("/health", get(health))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(420),
        ))
        .layer(
            CorsLayer::new()
                .allow_origin("*".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST])
                .allow_headers([axum::http::header::CONTENT_TYPE]),
        )
        .with_state(state)
}

pub async fn serve(listener: TcpListener, app: Router) -> anyhow::Result<()> {
    info!("ai-debate v0.2.0 running at http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}

// --- HTTP handlers -----------------------------------------------------------

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn serve_favicon() -> Response {
    Response::builder()
        .header("Content-Type", "image/svg+xml")
        .header("Cache-Control", "public, max-age=86400")
        .body(axum::body::Body::from(include_str!("../static/favicon.svg")))
        .unwrap()
}

/// Return available providers, models, and default selections.
/// Used by the frontend to populate model selection dropdowns.
async fn get_models(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let providers = config::detect_available_providers();
    let (default_pro, default_con, default_judge) = config::default_models();

    Json(json!({
        "providers": providers,
        "defaults": {
            "pro": state.pro.model_id,
            "con": state.con.model_id,
            "judge": state.judge.model_id,
        },
        "registered_defaults": {
            "pro": default_pro,
            "con": default_con,
            "judge": default_judge,
        },
        "features": {
            "web_search": tools::is_search_enabled(),
        }
    }))
}

async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "version": "0.2.0",
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "pro": state.pro.name,
        "pro_model": state.pro.model_id,
        "con": state.con.name,
        "con_model": state.con.model_id,
        "judge": state.judge.name,
        "judge_model": state.judge.model_id,
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

    // Resolve clients: use custom models if specified, otherwise use defaults.
    let pro_client = match resolve_client(&state, &payload.pro_model, "pro").await {
        Ok(c) => c,
        Err(e) => {
            return sse_error(&format!("Pro model init failed: {}", e), timer);
        }
    };
    let con_client = match resolve_client(&state, &payload.con_model, "con").await {
        Ok(c) => c,
        Err(e) => {
            return sse_error(&format!("Con model init failed: {}", e), timer);
        }
    };
    let judge_client = match resolve_client(&state, &payload.judge_model, "judge").await {
        Ok(c) => c,
        Err(e) => {
            return sse_error(&format!("Judge model init failed: {}", e), timer);
        }
    };

    let topic = payload.topic.clone();
    let user_id = payload.user_id.clone();
    let session_id = payload.session_id.clone();
    let state = state.clone();
    let mut timer = timer;

    let stream = async_stream::stream! {
        yield sse_json(&json!({
            "type": "phase",
            "phase": "init",
            "message": "Debate started",
            "models": {
                "pro": pro_client.model_id,
                "con": con_client.model_id,
                "judge": judge_client.model_id,
            }
        }));

        let mut transcript = Vec::new();

        // Four debate phases: pro then con each phase
        let debate_phases = [
            DebatePhase::Opening,
            DebatePhase::Rebuttal,
            DebatePhase::Defense,
            DebatePhase::Closing,
        ];

        for phase in debate_phases {
            for (side, client) in [
                (Position::Pro, &pro_client),
                (Position::Con, &con_client),
            ] {
                yield sse_json(&json!({
                    "type": "phase_start",
                    "phase": phase.as_str(),
                    "side": side.role_str(),
                    "title": phase.title(),
                    "provider": client.name,
                    "model": client.model_id,
                }));

                // Choose between tool-enabled and regular execution
                let search_enabled = tools::is_search_enabled();
                let round_result = if search_enabled {
                    execute_round_with_tools(client, side, phase, &topic, &transcript).await
                } else {
                    execute_one_round(client, side, phase, &topic, &transcript).await
                };

                match round_result {
                    Ok((mut stream, model_id)) => {
                        let mut full_content = String::new();

                        while let Some(chunk_res) = stream.next().await {
                            match chunk_res {
                                Ok(DebateStreamChunk::Delta(delta)) => {
                                    if !delta.is_empty() {
                                        yield sse_json(&json!({
                                            "type": "delta",
                                            "side": side.role_str(),
                                            "phase": phase.as_str(),
                                            "model": model_id,
                                            "content": delta,
                                        }));
                                        full_content.push_str(&delta);
                                    }
                                }
                                Ok(DebateStreamChunk::Thinking(thinking)) => {
                                    if !thinking.is_empty() {
                                        yield sse_json(&json!({
                                            "type": "thinking",
                                            "side": side.role_str(),
                                            "phase": phase.as_str(),
                                            "model": model_id,
                                            "content": thinking,
                                        }));
                                    }
                                }
                                Ok(DebateStreamChunk::Usage(usage)) => {
                                    yield sse_json(&json!({
                                        "type": "usage",
                                        "side": side.role_str(),
                                        "phase": phase.as_str(),
                                        "model": model_id,
                                        "usage": usage,
                                    }));
                                }
                                Ok(DebateStreamChunk::SearchPerformed(search_result)) => {
                                    yield sse_json(&json!({
                                        "type": "search",
                                        "side": side.role_str(),
                                        "phase": phase.as_str(),
                                        "model": model_id,
                                        "query": search_result.query,
                                        "results": search_result.results,
                                    }));
                                }
                                Err(e) => {
                                    if let Some(t) = timer.take() { t.stop(); }
                                    yield sse_json(&json!({"type":"error","message": format!("Stream error: {}", e)}));
                                    return;
                                }
                            }
                        }

                        transcript.push((side, phase, full_content.clone(), model_id.clone()));
                        let _ = save_message(
                            &state.db, &user_id, &session_id,
                            side, phase, Some(&model_id), &full_content,
                        ).await;
                        yield sse_json(&json!({
                            "type": "phase_done",
                            "phase": phase.as_str(),
                            "side": side.role_str(),
                            "model": model_id,
                        }));
                    }
                    Err(e) => {
                        if let Some(t) = timer.take() { t.stop(); }
                        yield sse_json(&json!({"type":"error","message": format!("Round failed: {}", e)}));
                        return;
                    }
                }
            }
        }

        // Judge round - now with real streaming
        {
            yield sse_json(&json!({
                "type": "phase_start",
                "phase": "judgement",
                "side": "judge",
                "title": DebatePhase::Judgement.title(),
                "provider": judge_client.name,
                "model": judge_client.model_id,
            }));

            match execute_judge_round_stream(&judge_client, &topic, &transcript).await {
                Ok((mut stream, model_id)) => {
                    let mut full_content = String::new();

                    while let Some(chunk_res) = stream.next().await {
                        match chunk_res {
                            Ok(DebateStreamChunk::Delta(delta)) => {
                                if !delta.is_empty() {
                                    yield sse_json(&json!({
                                        "type": "delta",
                                        "side": "judge",
                                        "phase": "judgement",
                                        "model": model_id,
                                        "content": delta,
                                    }));
                                    full_content.push_str(&delta);
                                }
                            }
                            Ok(DebateStreamChunk::Thinking(thinking)) => {
                                if !thinking.is_empty() {
                                    yield sse_json(&json!({
                                        "type": "thinking",
                                        "side": "judge",
                                        "phase": "judgement",
                                        "model": model_id,
                                        "content": thinking,
                                    }));
                                }
                            }
                            Ok(DebateStreamChunk::Usage(usage)) => {
                                yield sse_json(&json!({
                                    "type": "usage",
                                    "side": "judge",
                                    "phase": "judgement",
                                    "model": model_id,
                                    "usage": usage,
                                }));
                            }
                            Ok(DebateStreamChunk::SearchPerformed(_)) => {
                                // Judge doesn't use tools - ignore
                            }
                            Err(e) => {
                                if let Some(t) = timer.take() { t.stop(); }
                                yield sse_json(&json!({"type":"error","message": format!("Judge stream error: {}", e)}));
                                return;
                            }
                        }
                    }

                    transcript.push((Position::Judge, DebatePhase::Judgement, full_content.clone(), model_id.clone()));
                    let _ = save_message(
                        &state.db, &user_id, &session_id,
                        Position::Judge, DebatePhase::Judgement, Some(&model_id), &full_content,
                    ).await;
                    yield sse_json(&json!({
                        "type": "phase_done",
                        "phase": "judgement",
                        "side": "judge",
                        "model": model_id,
                    }));
                }
                Err(e) => {
                    if let Some(t) = timer.take() { t.stop(); }
                    yield sse_json(&json!({"type":"error","message": format!("Judge failed: {}", e)}));
                    return;
                }
            }
        }

        if let Some(t) = timer.take() {
            t.stop();
        }
        yield "data: {\"type\":\"done\"}\n\n".to_string();
    };

    let body_stream = stream.map(|chunk| Ok::<_, std::io::Error>(chunk));
    Response::builder()
        .status(200)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .body(Body::from_stream(body_stream))
        .unwrap()
}

// --- Helpers ----------------------------------------------------------------

/// Resolve a client for a given role. If a custom model is specified, build a new client.
/// Otherwise, use the default client from app state.
async fn resolve_client(
    state: &Arc<AppState>,
    custom_model: &Option<String>,
    role: &str,
) -> anyhow::Result<ClientInfo> {
    if let Some(model_id) = custom_model {
        let model_id = model_id.trim();
        if !model_id.is_empty() {
            return config::build_client_for_model(model_id).await;
        }
    }
    // Use default client for this role
    Ok(match role {
        "pro" => state.pro.clone(),
        "con" => state.con.clone(),
        "judge" => state.judge.clone(),
        _ => state.pro.clone(),
    })
}

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

fn sse_error(msg: &str, timer: Option<Box<dyn Timer + Send>>) -> Response {
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
