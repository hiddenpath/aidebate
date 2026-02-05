use ai_lib_rust::StreamingEvent;
use futures::StreamExt;
use std::collections::HashMap;

use crate::prompts::{build_judge_prompt, build_side_prompt};
use crate::types::{client_for_side, AppState, DebatePhase, Position};

/// Chunk from debate stream: content delta or thinking (reasoning) from provider.
#[derive(Debug, Clone)]
pub enum DebateStreamChunk {
    Delta(String),
    Thinking(String),
}

/// Execute one debate round (single side, single phase) and return content stream with provider name.
/// Uses ai-lib-rust streaming API and forwards ThinkingDelta (reasoning) when present.
pub async fn execute_one_round(
    state: &std::sync::Arc<AppState>,
    side: Position,
    phase: DebatePhase,
    topic: &str,
    transcript: &[(Position, DebatePhase, String, String)],
) -> anyhow::Result<(
    std::pin::Pin<Box<dyn futures::Stream<Item = anyhow::Result<DebateStreamChunk>> + Send>>,
    String,
)> {
    let client_info = client_for_side(state, side);
    let messages = build_side_prompt(side, phase, topic, transcript);

    let stream = client_info
        .client
        .chat()
        .messages(messages)
        .temperature(0.7)
        .max_tokens(2048)
        .stream()
        .execute_stream()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start stream for {}: {}", client_info.name, e))?;

    let output_stream = stream.map(|event_res| match event_res {
        Ok(event) => match event {
            StreamingEvent::PartialContentDelta { content, .. } => {
                Ok(DebateStreamChunk::Delta(content))
            }
            StreamingEvent::ThinkingDelta { thinking, .. } => {
                Ok(DebateStreamChunk::Thinking(thinking))
            }
            StreamingEvent::StreamError { error, .. } => {
                let msg: String = error
                    .get("message")
                    .or_else(|| error.get("error"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| error.to_string());
                Err(anyhow::anyhow!("Stream error: {}", msg))
            }
            _ => Ok(DebateStreamChunk::Delta(String::new())),
        },
        Err(e) => Err(anyhow::anyhow!("Stream error: {}", e)),
    });

    Ok((Box::pin(output_stream), client_info.name.clone()))
}

/// Execute judge round with reasoning analysis.
/// Uses ai-lib-rust non-streaming execute() for a single round-trip and direct usage in response.
pub async fn execute_judge_round(
    state: &std::sync::Arc<AppState>,
    topic: &str,
    transcript: &[(Position, DebatePhase, String, String)],
) -> anyhow::Result<(String, String)> {
    let judge = &state.judge;
    let messages = build_judge_prompt(topic, transcript);

    let response = judge
        .client
        .chat()
        .messages(messages)
        .temperature(0.3)
        .max_tokens(1024)
        .execute()
        .await
        .map_err(|e| anyhow::anyhow!("Judge execution failed: {}", e))?;

    let full_response = response.content;

    // Parse markdown sections
    let sections = parse_markdown_sections(&full_response);

    let reasoning = sections.get("Reasoning").cloned().unwrap_or_default();
    let verdict = sections
        .get("Verdict")
        .cloned()
        .unwrap_or_else(|| "No verdict provided.".to_string());

    let final_output = format!("## Reasoning\n{}\n\n## Verdict\n{}", reasoning, verdict);

    Ok((final_output, judge.name.clone()))
}

/// Simple markdown section parser
/// Extracts content under ## headers
fn parse_markdown_sections(content: &str) -> HashMap<String, String> {
    let mut sections = HashMap::new();
    let mut current_section: Option<String> = None;
    let mut current_content = String::new();

    for line in content.lines() {
        if line.starts_with("## ") {
            // Save previous section
            if let Some(section_name) = current_section.take() {
                sections.insert(section_name, current_content.trim().to_string());
            }
            // Start new section
            current_section = Some(line[3..].trim().to_string());
            current_content = String::new();
        } else if current_section.is_some() {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Save last section
    if let Some(section_name) = current_section {
        sections.insert(section_name, current_content.trim().to_string());
    }

    sections
}
