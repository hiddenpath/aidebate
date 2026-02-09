use std::sync::Arc;

use ai_lib_rust::AiClientBuilder;
use tracing::info;

use crate::types::{AvailableModel, AvailableProvider, ClientInfo};

pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .try_init();
}

// ---------------------------------------------------------------------------
// Default model assignments
// ---------------------------------------------------------------------------

const PRO_DEFAULT_MODEL_ID: &str = "deepseek/deepseek-chat";
const CON_DEFAULT_MODEL_ID: &str = "zhipu/glm-4-plus";
const JUDGE_DEFAULT_MODEL_ID: &str = "groq/llama-3.3-70b-versatile";

const FALLBACK_MODEL_ID: &str = "mistral/mistral-small-latest";

// ---------------------------------------------------------------------------
// Provider registry for auto-detection
// ---------------------------------------------------------------------------

/// (provider_id, display_name, env_var_name, models[])
const PROVIDER_REGISTRY: &[(&str, &str, &str, &[(&str, &str)])] = &[
    (
        "deepseek",
        "DeepSeek",
        "DEEPSEEK_API_KEY",
        &[
            ("deepseek/deepseek-chat", "DeepSeek Chat"),
            ("deepseek/deepseek-reasoner", "DeepSeek Reasoner"),
        ],
    ),
    (
        "zhipu",
        "Zhipu (智谱)",
        "ZHIPU_API_KEY",
        &[
            ("zhipu/glm-4-plus", "GLM-4 Plus"),
            ("zhipu/glm-4-flash", "GLM-4 Flash"),
        ],
    ),
    (
        "groq",
        "Groq",
        "GROQ_API_KEY",
        &[
            ("groq/llama-3.3-70b-versatile", "Llama 3.3 70B"),
            ("groq/llama-3.1-8b-instant", "Llama 3.1 8B Instant"),
        ],
    ),
    (
        "mistral",
        "Mistral",
        "MISTRAL_API_KEY",
        &[
            ("mistral/mistral-small-latest", "Mistral Small"),
            ("mistral/mistral-large-latest", "Mistral Large"),
        ],
    ),
    (
        "openai",
        "OpenAI",
        "OPENAI_API_KEY",
        &[
            ("openai/gpt-4o", "GPT-4o"),
            ("openai/gpt-4o-mini", "GPT-4o Mini"),
        ],
    ),
    (
        "anthropic",
        "Anthropic",
        "ANTHROPIC_API_KEY",
        &[
            ("anthropic/claude-3-5-sonnet", "Claude 3.5 Sonnet"),
            ("anthropic/claude-3-5-haiku", "Claude 3.5 Haiku"),
        ],
    ),
    (
        "minimax",
        "MiniMax",
        "MINIMAX_API_KEY",
        &[("minimax/abab6.5s-chat", "ABAB 6.5s Chat")],
    ),
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Detect available providers by checking environment variables.
/// Returns a list of providers with their available models and key status.
pub fn detect_available_providers() -> Vec<AvailableProvider> {
    PROVIDER_REGISTRY
        .iter()
        .map(|(provider, display_name, env_var, models)| {
            let has_key = std::env::var(env_var).is_ok();
            AvailableProvider {
                provider: provider.to_string(),
                display_name: display_name.to_string(),
                env_var: env_var.to_string(),
                has_key,
                models: models
                    .iter()
                    .map(|(id, name)| AvailableModel {
                        model_id: id.to_string(),
                        display_name: name.to_string(),
                    })
                    .collect(),
            }
        })
        .collect()
}

/// Get default model IDs for each role.
pub fn default_models() -> (&'static str, &'static str, &'static str) {
    (PRO_DEFAULT_MODEL_ID, CON_DEFAULT_MODEL_ID, JUDGE_DEFAULT_MODEL_ID)
}

/// Build a client for any model_id (used for dynamic model selection).
/// Uses Mistral as universal fallback.
pub async fn build_client_for_model(model_id: &str) -> anyhow::Result<ClientInfo> {
    let name = provider_name_from_model_id(model_id);
    let mut builder = AiClientBuilder::new();

    if use_resilience() {
        builder = builder.circuit_breaker_default().max_inflight(4);
    }

    builder = builder.with_fallbacks(vec![FALLBACK_MODEL_ID.to_string()]);

    let client = builder
        .build(model_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build client for {}: {}", model_id, e))?;

    info!("✅ Dynamic client ready: {} ({})", model_id, name);

    Ok(ClientInfo {
        name: name.to_string(),
        model_id: model_id.to_string(),
        client: Arc::new(client),
    })
}

/// Initialize the AI protocol environment (local dir or GitHub fallback).
/// Must be called before building any clients.
pub fn init_protocol_env() {
    if std::env::var("AI_PROTOCOL_DIR").is_err() && std::env::var("AI_PROTOCOL_PATH").is_err() {
        let local_paths = ["ai-protocol", "../ai-protocol", "../../ai-protocol"];
        let mut found_local = false;
        for path in local_paths {
            if std::path::Path::new(path).join("v1/providers").exists() {
                std::env::set_var("AI_PROTOCOL_DIR", path);
                info!("Using local AI-Protocol: {}", path);
                found_local = true;
                break;
            }
        }
        if !found_local {
            info!("Using remote AI-Protocol from GitHub (may be slow)");
            std::env::set_var(
                "AI_PROTOCOL_DIR",
                "https://raw.githubusercontent.com/hiddenpath/ai-protocol/main",
            );
        }
    }
}

/// Initialize default clients for the three roles.
pub async fn init_clients() -> anyhow::Result<(ClientInfo, ClientInfo, ClientInfo)> {
    init_protocol_env();

    let pro_model = model_id_for_role("pro", PRO_DEFAULT_MODEL_ID);
    let con_model = model_id_for_role("con", CON_DEFAULT_MODEL_ID);
    let judge_model = model_id_for_role("judge", JUDGE_DEFAULT_MODEL_ID);

    // Log key availability (masked)
    let check_key = |name: &str, env_var: &str| match std::env::var(env_var) {
        Ok(val) => {
            let mask = if val.len() > 4 {
                format!("{}...", &val[..4])
            } else {
                "***".to_string()
            };
            info!("Key: {} ({}) SET [{}]", name, env_var, mask);
        }
        Err(_) => info!("Key: {} ({}) MISSING", name, env_var),
    };
    check_key("DeepSeek", "DEEPSEEK_API_KEY");
    check_key("Zhipu", "ZHIPU_API_KEY");
    check_key("Groq", "GROQ_API_KEY");
    check_key("Mistral", "MISTRAL_API_KEY");

    let pro = build_role_client(&pro_model, "pro").await?;
    let con = build_role_client(&con_model, "con").await?;
    let judge = build_role_client(&judge_model, "judge").await?;

    Ok((pro, con, judge))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn provider_name_from_model_id(model_id: &str) -> &str {
    model_id.split('/').next().unwrap_or(model_id)
}

fn model_id_for_role(role: &str, default: &str) -> String {
    let env_key = match role {
        "pro" => "PRO_MODEL_ID",
        "con" => "CON_MODEL_ID",
        "judge" => "JUDGE_MODEL_ID",
        _ => "MODEL_ID",
    };
    std::env::var(env_key).unwrap_or_else(|_| default.to_string())
}

fn use_resilience() -> bool {
    std::env::var("AI_DEBATE_RESILIENCE")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn fallback_for_role(_role: &str) -> Vec<String> {
    vec![FALLBACK_MODEL_ID.to_string()]
}

async fn build_role_client(model_id: &str, role: &str) -> anyhow::Result<ClientInfo> {
    let name = provider_name_from_model_id(model_id);
    let mut builder = AiClientBuilder::new();

    if use_resilience() {
        builder = builder.circuit_breaker_default().max_inflight(4);
    }

    let fallbacks = fallback_for_role(role);
    if !fallbacks.is_empty() {
        builder = builder.with_fallbacks(fallbacks.clone());
    }

    let client = builder
        .build(model_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build client for {}: {}", name, e))?;

    info!("Provider [{}] ready, model: {}, role: {}", name, model_id, role);

    Ok(ClientInfo {
        name: name.to_string(),
        model_id: model_id.to_string(),
        client: Arc::new(client),
    })
}
