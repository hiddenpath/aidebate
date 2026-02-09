//! Web search tool for evidence-backed debates.
//!
//! Uses Tavily API for web search. Enabled when TAVILY_API_KEY is set.
//! When disabled, debates proceed without tool calling (no behavior change).

use ai_lib_rust::types::tool::{FunctionDefinition, ToolDefinition};
use anyhow::Result;
use serde_json::json;
use tracing::info;

/// Check if the web search tool is available (TAVILY_API_KEY is set).
pub fn is_search_enabled() -> bool {
    std::env::var("TAVILY_API_KEY").is_ok()
}

/// Build the tool definition for web search (OpenAI-compatible function schema).
pub fn search_tool_definition() -> ToolDefinition {
    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: "web_search".to_string(),
            description: Some(
                "Search the web for factual evidence, statistics, news, or data to support your argument. Use specific, factual queries."
                    .to_string(),
            ),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query - be specific and factual, e.g. 'AI job displacement statistics 2025'"
                    }
                },
                "required": ["query"]
            })),
        },
    }
}

/// Search result from a web search tool call.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub query: String,
    pub results: String,
}

/// Execute a web search via the Tavily API.
pub async fn execute_web_search(query: &str) -> Result<SearchResult> {
    let api_key = std::env::var("TAVILY_API_KEY")
        .map_err(|_| anyhow::anyhow!("TAVILY_API_KEY not set"))?;

    info!("Web search: {}", query);

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.tavily.com/search")
        .json(&json!({
            "api_key": api_key,
            "query": query,
            "search_depth": "basic",
            "include_answer": true,
            "max_results": 3
        }))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Search request failed: {}", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| anyhow::anyhow!("Search response parse failed: {}", e))?;

    // Format results for model consumption
    let mut formatted = Vec::new();

    // Include Tavily's direct answer if available
    if let Some(answer) = resp["answer"].as_str() {
        if !answer.is_empty() {
            formatted.push(format!("Direct Answer: {}\n", answer));
        }
    }

    // Format individual results
    if let Some(results) = resp["results"].as_array() {
        for r in results {
            let title = r["title"].as_str().unwrap_or("");
            let content: String = r["content"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(300)
                .collect();
            let url = r["url"].as_str().unwrap_or("");
            formatted.push(format!("Source: {}\n{}\nURL: {}\n", title, content, url));
        }
    }

    let results_text = if formatted.is_empty() {
        "No relevant results found.".to_string()
    } else {
        formatted.join("\n")
    };

    Ok(SearchResult {
        query: query.to_string(),
        results: results_text,
    })
}
