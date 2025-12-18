use ai_lib::response_parser::MarkdownSectionParser;
use ai_lib::ChatCompletionRequest;
use futures::StreamExt;

use crate::prompts::{build_judge_prompt, build_side_prompt};
use crate::types::{client_for_side, AppState, DebatePhase, Position};

/// Execute one debate round (single side, single phase) and return content stream with provider name
pub async fn execute_one_round(
    state: &std::sync::Arc<AppState>,
    side: Position,
    phase: DebatePhase,
    topic: &str,
    transcript: &[(Position, DebatePhase, String, String)],
) -> anyhow::Result<(
    std::pin::Pin<Box<dyn futures::Stream<Item = anyhow::Result<String>> + Send>>,
    String,
)> {
    let client_info = client_for_side(state, side);
    let prompt = build_side_prompt(side, phase, topic, transcript);

    // (Placeholder for future capability-aware model choice)
    let model = client_info.default_model.clone();

    let req = ChatCompletionRequest::new(model, prompt)
        .with_temperature(0.7)
        .with_max_tokens(2048);

    let stream = client_info
        .client
        .chat_completion_stream(req)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start stream for {}: {}", client_info.name, e))?;

    // Map the stream to just delta content strings
    let output_stream = stream.map(|chunk_res| match chunk_res {
        Ok(chunk) => {
            let delta = chunk
                .choices
                .first()
                .and_then(|c| c.delta.content.clone())
                .unwrap_or_default();
            Ok(delta)
        }
        Err(e) => Err(anyhow::anyhow!("Stream error: {}", e)),
    });

    Ok((Box::pin(output_stream), client_info.name.clone()))
}

/// Execute judge round with reasoning analysis
pub async fn execute_judge_round(
    state: &std::sync::Arc<AppState>,
    topic: &str,
    transcript: &[(Position, DebatePhase, String, String)],
) -> anyhow::Result<(String, String)> {
    let judge = &state.judge;
    let prompt = build_judge_prompt(topic, transcript);

    // Prefer reasoning-capable models when available (left simple; future: capability-based)
    let model = judge.default_model.clone();

    let req = ChatCompletionRequest::new(model, prompt)
        .with_temperature(0.3) // Lower temperature for consistent judgment
        .with_max_tokens(1024);

    // Parse using generic MarkdownSectionParser
    // Expecting sections: ## Reasoning, ## Verdict
    let parser = MarkdownSectionParser::new();
    let sections = judge
        .client
        .chat_completion_parsed(req, parser)
        .await
        .map_err(|e| anyhow::anyhow!("Judge execution failed: {}", e))?;

    let reasoning = sections.get("Reasoning").cloned().unwrap_or_default();
    let verdict = sections
        .get("Verdict")
        .cloned()
        .unwrap_or_else(|| "No verdict provided.".to_string());

    let final_output = format!("## Reasoning\n{}\n\n## Verdict\n{}", reasoning, verdict);

    Ok((final_output, judge.name.clone()))
}
