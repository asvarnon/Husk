use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::redis::StoredMessage;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub function: ToolFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage>,
    tools: Vec<serde_json::Value>,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: OllamaMessage,
}

fn web_search_tool_def() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "web_search",
            "description": "Search the web for current information",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "The search query"}
                },
                "required": ["query"]
            }
        }
    })
}

fn build_messages(
    history: &[StoredMessage],
    system_prompt: &str,
    memory_block: Option<&str>,
) -> Vec<OllamaMessage> {
    let mut msgs = vec![OllamaMessage {
        role: "system".to_string(),
        content: system_prompt.to_string(),
        tool_calls: None,
    }];

    let convo: Vec<OllamaMessage> = history
        .iter()
        .map(|m| {
            let content = if m.role == "user" {
                match &m.name {
                    Some(name) => format!("{}: {}", name, m.content),
                    None => m.content.clone(),
                }
            } else {
                m.content.clone()
            };
            OllamaMessage {
                role: m.role.clone(),
                content,
                tool_calls: None,
            }
        })
        .collect();

    // Long-term memory is untrusted recall, not instruction: inject it as a labeled user-role
    // reference block right before the latest user turn (most relevant position) — never into
    // the system message itself.
    if let Some(block) = memory_block {
        let mut convo = convo;
        let pos = convo.len().saturating_sub(1);
        convo.insert(
            pos,
            OllamaMessage {
                role: "user".to_string(),
                content: block.to_string(),
                tool_calls: None,
            },
        );
        msgs.extend(convo);
    } else {
        msgs.extend(convo);
    }

    msgs
}

pub async fn run_chat(
    client: &Client,
    host: &str,
    model: &str,
    history: &[StoredMessage],
    system_prompt: &str,
    searxng_url: &str,
    memory_block: Option<&str>,
) -> Result<String> {
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    let mut messages = build_messages(history, system_prompt, memory_block);

    for _ in 0..5 {
        let req = ChatRequest {
            model,
            messages: messages.clone(),
            tools: vec![web_search_tool_def()],
            stream: false,
        };

        let resp: ChatResponse = client.post(&url).json(&req).send().await?.json().await?;

        let assistant_msg = resp.message;

        if let Some(calls) = &assistant_msg.tool_calls {
            if let Some(call) = calls.first() {
                if call.function.name == "web_search" {
                    let query = call
                        .function
                        .arguments
                        .get("query")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow!("web_search tool call missing query argument"))?;
                    tracing::info!("web_search: query={:?} model={}", query, model);
                    let results = crate::search::web_search(client, searxng_url, query).await?;

                    messages.push(OllamaMessage {
                        role: "assistant".to_string(),
                        content: String::new(),
                        tool_calls: assistant_msg.tool_calls.clone(),
                    });
                    messages.push(OllamaMessage {
                        role: "tool".to_string(),
                        content: results,
                        tool_calls: None,
                    });
                    continue;
                }
            }
        }

        return Ok(assistant_msg.content);
    }

    Err(anyhow!("tool call loop exceeded maximum iterations"))
}
