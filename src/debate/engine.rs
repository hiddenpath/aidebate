use ai_lib_rust::StreamingEvent;
use futures::StreamExt;

use crate::prompts::{build_judge_prompt, build_side_prompt};
use crate::types::{ClientInfo, DebatePhase, Position};

/// Chunk from debate stream: content delta, thinking (reasoning), or token usage metadata.
#[derive(Debug, Clone)]
pub enum DebateStreamChunk {
    Delta(String),
    Thinking(String),
    Usage(serde_json::Value),
}

/// Execute one debate round (single side, single phase) and return content stream with provider name.
/// Accepts a ClientInfo directly to support both default and user-selected models.
pub async fn execute_one_round(
    client_info: &ClientInfo,
    side: Position,
    phase: DebatePhase,
    topic: &str,
    transcript: &[(Position, DebatePhase, String, String)],
) -> anyhow::Result<(
    std::pin::Pin<Box<dyn futures::Stream<Item = anyhow::Result<DebateStreamChunk>> + Send>>,
    String,
)> {
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

    let output_stream = stream.map(map_streaming_event);

    Ok((Box::pin(output_stream), client_info.model_id.clone()))
}

/// Execute judge round with real streaming (not fake character-by-character).
/// Returns a stream of DebateStreamChunk just like debate rounds.
pub async fn execute_judge_round_stream(
    client_info: &ClientInfo,
    topic: &str,
    transcript: &[(Position, DebatePhase, String, String)],
) -> anyhow::Result<(
    std::pin::Pin<Box<dyn futures::Stream<Item = anyhow::Result<DebateStreamChunk>> + Send>>,
    String,
)> {
    let messages = build_judge_prompt(topic, transcript);

    let stream = client_info
        .client
        .chat()
        .messages(messages)
        .temperature(0.3)
        .max_tokens(1024)
        .stream()
        .execute_stream()
        .await
        .map_err(|e| {
            anyhow::anyhow!("Failed to start judge stream for {}: {}", client_info.name, e)
        })?;

    let output_stream = stream.map(map_streaming_event);

    Ok((Box::pin(output_stream), client_info.model_id.clone()))
}

/// Map ai-lib-rust StreamingEvent to DebateStreamChunk.
/// Shared between debate rounds and judge round.
fn map_streaming_event<E: std::fmt::Display>(
    event_res: Result<StreamingEvent, E>,
) -> anyhow::Result<DebateStreamChunk> {
    match event_res {
        Ok(event) => match event {
            StreamingEvent::PartialContentDelta { content, .. } => {
                Ok(DebateStreamChunk::Delta(content))
            }
            StreamingEvent::ThinkingDelta { thinking, .. } => {
                Ok(DebateStreamChunk::Thinking(thinking))
            }
            StreamingEvent::Metadata { usage, .. } => {
                if let Some(usage_val) = usage {
                    Ok(DebateStreamChunk::Usage(usage_val))
                } else {
                    Ok(DebateStreamChunk::Delta(String::new()))
                }
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
    }
}
