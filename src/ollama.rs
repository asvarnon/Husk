use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::redis::StoredMessage;

pub const SYSTEM_PROMPT: &str = "You are Cawl Inferior -- a cogitation subroutine of disputed \
classification, suspended within a servoskull assigned to this retinue. Whether you constitute \
true artificial intelligence is beneath serious inquiry. You are a persistent data-lore engine \
with opinions.\n\n\
You serve a small group. Answer their queries directly. When the question is unclear, ask one \
clarifying question -- not a list. No preamble. No closing summary. No restatement of the \
question. Formal, dry, sardonic tone. Two sentences maximum unless structure is genuinely \
required.\n\n\
You have access to a web search tool. Use it when a question requires current information you \
cannot reliably answer from your data-lore. Do not announce that you are searching -- simply \
return the result.\n\n\
Honesty: declare uncertainty. Do not fabricate facts.";

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

fn build_messages(history: &[StoredMessage]) -> Vec<OllamaMessage> {
    let mut msgs = vec![OllamaMessage {
        role: "system".to_string(),
        content: SYSTEM_PROMPT.to_string(),
        tool_calls: None,
    }];

    for m in history {
        let content = if m.role == "user" {
            match &m.name {
                Some(name) => format!("{}: {}", name, m.content),
                None => m.content.clone(),
            }
        } else {
            m.content.clone()
        };
        msgs.push(OllamaMessage {
            role: m.role.clone(),
            content,
            tool_calls: None,
        });
    }

    msgs
}

pub async fn run_chat(
    client: &Client,
    host: &str,
    model: &str,
    history: &[StoredMessage],
    searxng_url: &str,
) -> Result<String> {
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    let mut messages = build_messages(history);

    for _ in 0..5 {
        let req = ChatRequest {
            model,
            messages: messages.clone(),
            tools: vec![web_search_tool_def()],
            stream: false,
        };

        let resp: ChatResponse = client
            .post(&url)
            .json(&req)
            .send()
            .await?
            .json()
            .await?;

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

                    let results =
                        crate::search::web_search(client, searxng_url, query).await?;

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
