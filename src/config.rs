use std::sync::Arc;
use std::time::Duration;

use ai_lib::{AiClient, AiClientBuilder, Provider};
use tracing::info;

use crate::types::ClientInfo;

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .try_init();
}

pub fn init_clients() -> anyhow::Result<(ClientInfo, ClientInfo, ClientInfo)> {
    // 1. Available candidates map
    let mut candidates: std::collections::HashMap<String, Provider> =
        std::collections::HashMap::new();
    let supported = [
        ("groq", Provider::Groq, "GROQ_API_KEY"),
        ("mistral", Provider::Mistral, "MISTRAL_API_KEY"),
        ("deepseek", Provider::DeepSeek, "DEEPSEEK_API_KEY"),
        ("gemini", Provider::Gemini, "GOOGLE_API_KEY"),
        ("openai", Provider::OpenAI, "OPENAI_API_KEY"),
        ("qwen", Provider::Qwen, "DASHSCOPE_API_KEY"),
        ("zhipu", Provider::ZhipuAI, "ZHIPU_API_KEY"),
        ("anthropic", Provider::Anthropic, "ANTHROPIC_API_KEY"),
    ];

    for (key, provider, env_var) in supported {
        if std::env::var(env_var).is_ok() {
            candidates.insert(key.to_string(), provider);
        }
    }

    if candidates.is_empty() {
        anyhow::bail!("未检测到任何可用的 API Key。请设置 (OPENAI_API_KEY, GOOGLE_API_KEY 等)");
    }

    // 2. Helper to get client for a role, with fallback
    let get_client = |role_env: &str,
                      default_fallback_idx: usize,
                      used_providers: &mut Vec<String>|
     -> anyhow::Result<ClientInfo> {
        // Try precise env var first: PRO_PROVIDER=deepseek
        if let Ok(p_name) = std::env::var(role_env) {
            let p_key = p_name.to_lowercase();
            if let Some(provider) = candidates.get(&p_key) {
                return build_client(&p_key, *provider);
            }
            info!(
                "指定的 {}={} 不可用 (无 API Key 或不支持)，将尝试自动回退",
                role_env, p_name
            );
        }

        // Fallback: Pick first available that hasn't been heavily used if possible,
        // but for simplicity just pick from available list skipping if we want round-robin
        // Simple logic: Convert map to sorted vec to be deterministic
        let mut sorted_keys: Vec<&String> = candidates.keys().collect();
        sorted_keys.sort();

        // Round robin index based on previously used count
        // If we have enough providers, pick a unique one.
        // If not, just cycle.
        let idx = (default_fallback_idx + used_providers.len()) % sorted_keys.len();
        let key = sorted_keys[idx];
        used_providers.push(key.clone());

        let provider = candidates[key];
        build_client(key, provider)
    };

    let mut used = Vec::new();
    let pro = get_client("PRO_PROVIDER", 0, &mut used)?;
    let con = get_client("CON_PROVIDER", 1, &mut used)?;
    let judge = get_client("JUDGE_PROVIDER", 2, &mut used)?;

    Ok((pro, con, judge))
}

fn build_client(name: &str, provider: Provider) -> anyhow::Result<ClientInfo> {
    let mut builder = AiClientBuilder::new(provider).with_timeout(Duration::from_secs(180));
    if let Ok(proxy) = std::env::var("PROXY_URL") {
        builder = builder.with_proxy(Some(&proxy));
    }
    let client: AiClient = builder.build()?;
    let default_model = client.default_chat_model();
    info!(
        "✅ Provider [{}] ready, default model: {}",
        name, default_model
    );
    Ok(ClientInfo {
        name: name.to_string(),
        client: Arc::new(client),
        default_model,
    })
}
