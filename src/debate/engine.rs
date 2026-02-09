use ai_lib_rust::StreamingEvent;
use futures::StreamExt;
use tracing::info;

use crate::prompts::{build_judge_prompt, build_side_prompt, build_side_prompt_with_tools};
use crate::tools::{self, SearchResult};
use crate::types::{ClientInfo, DebatePhase, Position};

/// Chunk from debate stream: content delta, thinking, usage metadata, or search activity.
#[derive(Debug, Clone)]
pub enum DebateStreamChunk {
    Delta(String),
    Thinking(String),
    Usage(serde_json::Value),
    /// A web search was performed. Contains query and formatted results.
    SearchPerformed(SearchResult),
}

/// Execute one debate round with streaming (no tool calling).
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

/// Execute one debate round WITH tool calling support.
///
/// Flow:
/// 1. Non-streaming execute() with web_search tool → model decides whether to search
/// 2. If tool calls: execute searches, yield SearchPerformed events, then stream with context
/// 3. If no tool calls: yield the response content directly
pub async fn execute_round_with_tools(
    client_info: &ClientInfo,
    side: Position,
    phase: DebatePhase,
    topic: &str,
    transcript: &[(Position, DebatePhase, String, String)],
) -> anyhow::Result<(
    std::pin::Pin<Box<dyn futures::Stream<Item = anyhow::Result<DebateStreamChunk>> + Send>>,
    String,
)> {
    let messages = build_side_prompt_with_tools(side, phase, topic, transcript, None);
    let tool_defs = vec![tools::search_tool_definition()];

    // Phase 1: Non-streaming call with tools - let the model decide whether to search
    let response = client_info
        .client
        .chat()
        .messages(messages)
        .tools(tool_defs)
        .temperature(0.7)
        .max_tokens(2048)
        .execute()
        .await
        .map_err(|e| {
            anyhow::anyhow!("Tool-enabled execute failed for {}: {}", client_info.name, e)
        })?;

    if response.tool_calls.is_empty() {
        // No tool calls - model responded directly
        // Return the content as a single-chunk stream
        let content = response.content;
        let model_id = client_info.model_id.clone();
        let stream = async_stream::stream! {
            if !content.is_empty() {
                yield Ok(DebateStreamChunk::Delta(content));
            }
            if let Some(usage) = response.usage {
                yield Ok(DebateStreamChunk::Usage(usage));
            }
        };
        return Ok((Box::pin(stream), model_id));
    }

    // Phase 2: Model wants to search - execute tool calls
    info!(
        "Model {} requested {} tool call(s)",
        client_info.model_id,
        response.tool_calls.len()
    );

    let mut search_results = Vec::new();
    for tool_call in &response.tool_calls {
        if tool_call.name == "web_search" {
            let query = tool_call
                .arguments
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if !query.is_empty() {
                match tools::execute_web_search(&query).await {
                    Ok(result) => search_results.push(result),
                    Err(e) => {
                        info!("Search failed for '{}': {}", query, e);
                        search_results.push(SearchResult {
                            query,
                            results: format!("Search failed: {}", e),
                        });
                    }
                }
            }
        }
    }

    // Phase 3: Build context with search results and stream the final response
    let search_context = search_results
        .iter()
        .map(|r| format!("### Search: {}\n{}", r.query, r.results))
        .collect::<Vec<_>>()
        .join("\n\n");

    let messages_with_context =
        build_side_prompt_with_tools(side, phase, topic, transcript, Some(&search_context));

    let final_stream = client_info
        .client
        .chat()
        .messages(messages_with_context)
        .temperature(0.7)
        .max_tokens(2048)
        .stream()
        .execute_stream()
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to start post-search stream for {}: {}",
                client_info.name,
                e
            )
        })?;

    // Combine: first yield search events, then stream the final response
    let model_id = client_info.model_id.clone();
    let combined_stream = async_stream::stream! {
        // Yield search events so the UI can display them
        for result in search_results {
            yield Ok(DebateStreamChunk::SearchPerformed(result));
        }

        // Stream the final response
        let mut stream = std::pin::pin!(final_stream);
        while let Some(event_res) = stream.next().await {
            yield map_streaming_event(event_res);
        }
    };

    Ok((Box::pin(combined_stream), model_id))
}

/// Execute judge round with real streaming (no tools - judge evaluates objectively).
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
