use std::sync::Arc;

use ai_lib_rust::AiClientBuilder;
use tracing::info;

use crate::types::ClientInfo;

pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .try_init();
}

/// Fixed role assignments: 正方=DeepSeek, 反方=智谱 GLM, 裁判=Groq
/// Env override: PRO_MODEL_ID, CON_MODEL_ID, JUDGE_MODEL_ID (e.g. deepseek/deepseek-chat)
const PRO_DEFAULT_MODEL_ID: &str = "deepseek/deepseek-chat";
const CON_DEFAULT_MODEL_ID: &str = "zhipu/glm-4-plus";
const JUDGE_DEFAULT_MODEL_ID: &str = "groq/llama-3.3-70b-versatile";

/// Fallback models using Mistral (provider id: mistral, env: MISTRAL_API_KEY)
/// One fallback per role when primary fails. Mistral verified working in connectivity tests.
const PRO_FALLBACK_MODEL_ID: &str = "mistral/mistral-small-latest";
const CON_FALLBACK_MODEL_ID: &str = "mistral/mistral-small-latest";
const JUDGE_FALLBACK_MODEL_ID: &str = "mistral/mistral-small-latest";

fn role_env_key(role: &str) -> &'static str {
    match role {
        "pro" => "PRO_MODEL_ID",
        "con" => "CON_MODEL_ID",
        "judge" => "JUDGE_MODEL_ID",
        _ => "MODEL_ID",
    }
}

fn model_id_for_role(role: &str, default: &str) -> String {
    std::env::var(role_env_key(role)).unwrap_or_else(|_| default.to_string())
}

/// Local ai-protocol directories to check (in order of preference, cross-platform).
const LOCAL_AI_PROTOCOL_PATHS: &[&str] = &[
    "ai-protocol",
    "../ai-protocol",
    "../../ai-protocol",
];

/// Fallback: GitHub raw URL (used only if no local directory found).
const AI_PROTOCOL_GITHUB_RAW: &str =
    "https://raw.githubusercontent.com/hiddenpath/ai-protocol/main";

pub async fn init_clients() -> anyhow::Result<(ClientInfo, ClientInfo, ClientInfo)> {
    // Prefer local ai-protocol directory for reliability (avoid network timeouts).
    // Only fall back to GitHub if no local directory exists.
    if std::env::var("AI_PROTOCOL_DIR").is_err() && std::env::var("AI_PROTOCOL_PATH").is_err() {
        let mut found_local = false;
        for path in LOCAL_AI_PROTOCOL_PATHS {
            if std::path::Path::new(path).join("v1/providers").exists() {
                std::env::set_var("AI_PROTOCOL_DIR", *path);
                info!("📁 Using local AI-Protocol: {}", path);
                found_local = true;
                break;
            }
        }
        if !found_local {
            info!("🌐 Using remote AI-Protocol from GitHub (may be slow)");
            std::env::set_var("AI_PROTOCOL_DIR", AI_PROTOCOL_GITHUB_RAW);
        }
    }
    let pro_model = model_id_for_role("pro", PRO_DEFAULT_MODEL_ID);
    let con_model = model_id_for_role("con", CON_DEFAULT_MODEL_ID);
    let judge_model = model_id_for_role("judge", JUDGE_DEFAULT_MODEL_ID);

    // Debug: Check if keys are present (do not log actual keys)
    let check_key = |name: &str, env_var: &str| match std::env::var(env_var) {
        Ok(val) => {
            let mask = if val.len() > 4 {
                format!("{}...", &val[..4])
            } else {
                "***".to_string()
            };
            info!(
                "🔑 Key check: {} ({}) is SET (Starts with: {})",
                name, env_var, mask
            );
        }
        Err(_) => info!("❌ Key check: {} ({}) is MISSING", name, env_var),
    };
    check_key("DeepSeek", "DEEPSEEK_API_KEY");
    check_key("Zhipu (智谱)", "ZHIPU_API_KEY");
    check_key("Groq", "GROQ_API_KEY");
    check_key("Mistral (fallback)", "MISTRAL_API_KEY");

    let pro = build_client(&pro_model, "pro").await?;
    let con = build_client(&con_model, "con").await?;
    let judge = build_client(&judge_model, "judge").await?;

    Ok((pro, con, judge))
}

fn provider_name_from_model_id(model_id: &str) -> &str {
    model_id.split('/').next().unwrap_or(model_id)
}

fn use_resilience() -> bool {
    std::env::var("AI_DEBATE_RESILIENCE")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn fallback_for_role(role: &str) -> Vec<String> {
    let fallback = match role {
        "pro" => PRO_FALLBACK_MODEL_ID,
        "con" => CON_FALLBACK_MODEL_ID,
        "judge" => JUDGE_FALLBACK_MODEL_ID,
        _ => return Vec::new(),
    };
    vec![fallback.to_string()]
}

async fn build_client(model_id: &str, role: &str) -> anyhow::Result<ClientInfo> {
    let name = provider_name_from_model_id(model_id);
    let mut builder = AiClientBuilder::new();

    if use_resilience() {
        builder = builder
            .circuit_breaker_default()
            .max_inflight(4);
    }

    let fallbacks = fallback_for_role(role);
    if !fallbacks.is_empty() {
        builder = builder.with_fallbacks(fallbacks.clone());
        info!("🔄 Fallback for {}: {:?}", role, fallbacks);
    }

    let client = builder
        .build(model_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build client for {}: {}", name, e))?;

    info!("✅ Provider [{}] ready, model: {}", name, model_id);

    Ok(ClientInfo {
        name: name.to_string(),
        model_id: model_id.to_string(),
        client: Arc::new(client),
    })
}
