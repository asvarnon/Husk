use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::redis::StoredMessage;

// Circuit breaker on the tool-calling cycle: each iteration is one model round-trip, looping
// only when the model requests tools (it reads the results next round). A model that never
// settles on a final answer — endlessly calling tools — would otherwise loop forever, so cap
// the rounds and error out. Normal replies finish in 1-2 rounds, well under this.
const MAX_TOOL_ROUNDS: usize = 5;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    // OpenAI lets `content` be null on an assistant message that carries tool_calls, so it must
    // be optional in both directions: absent when we serialize, null-tolerant when we parse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    // Links a tool-result message back to the assistant tool_call it answers — OpenAI requires it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    // OpenAI assigns each tool call an id; the matching tool-result message must echo it back.
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type", default = "default_tool_call_type")]
    pub kind: String,
    pub function: ToolFunction,
}

fn default_tool_call_type() -> String {
    "function".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolFunction {
    pub name: String,
    // OpenAI-compat servers encode the call arguments as a JSON *string* (not a nested object,
    // as Ollama's native API did), so they arrive here as text to be parsed.
    pub arguments: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage>,
    tools: Vec<serde_json::Value>,
    stream: bool,
}

// OpenAI returns the reply nested under `choices[0].message`, not a top-level `message`.
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
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
) -> Vec<ChatMessage> {
    let mut msgs = vec![ChatMessage {
        role: "system".to_string(),
        content: Some(system_prompt.to_string()),
        tool_calls: None,
        tool_call_id: None,
    }];

    let convo: Vec<ChatMessage> = history
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
            ChatMessage {
                role: m.role.clone(),
                content: Some(content),
                tool_calls: None,
                tool_call_id: None,
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
            ChatMessage {
                role: "user".to_string(),
                content: Some(block.to_string()),
                tool_calls: None,
                tool_call_id: None,
            },
        );
        msgs.extend(convo);
    } else {
        msgs.extend(convo);
    }

    msgs
}

#[allow(clippy::too_many_arguments)]
pub async fn run_chat(
    client: &Client,
    base_url: &str,
    model: &str,
    history: &[StoredMessage],
    system_prompt: &str,
    searxng_url: Option<&str>,
    memory_block: Option<&str>,
    api_key: Option<&str>,
) -> Result<String> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let mut messages = build_messages(history, system_prompt, memory_block);

    // Only advertise web_search when SearXNG is configured, so the model never calls a tool
    // that isn't available.
    let tools: Vec<serde_json::Value> = match searxng_url {
        Some(_) => vec![web_search_tool_def()],
        None => vec![],
    };

    for _ in 0..MAX_TOOL_ROUNDS {
        let req = ChatRequest {
            model,
            messages: messages.clone(),
            tools: tools.clone(),
            stream: false,
        };

        // Attach the bearer token only when configured — local runners need no auth.
        let mut request = client.post(&url).json(&req);
        if let Some(key) = api_key {
            request = request.bearer_auth(key);
        }
        let resp: ChatResponse = request.send().await?.json().await?;

        let assistant_msg = resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("chat response had no choices"))?
            .message;

        // No tool calls → the assistant has given its final answer.
        let tool_calls = assistant_msg.tool_calls.clone().unwrap_or_default();
        if tool_calls.is_empty() {
            return Ok(assistant_msg.content.unwrap_or_default());
        }

        // Echo the assistant turn that requested the tools — pushed once; it carries every
        // tool_call, and OpenAI requires each to be answered by its own tool-role message below.
        messages.push(assistant_msg);

        // Answer EVERY call (a model may emit several in one turn), each linked back by its
        // tool_call_id. Dispatch is a hardcoded match for now — `web_search` is the only tool;
        // a general tool registry is tracked separately (see issue).
        for call in tool_calls {
            if call.function.name != "web_search" {
                return Err(anyhow!(
                    "model requested unknown tool '{}'",
                    call.function.name
                ));
            }

            // Arguments arrive as a JSON string, so parse it before reading fields.
            let args: serde_json::Value = serde_json::from_str(&call.function.arguments)
                .map_err(|e| anyhow!("web_search arguments were not valid JSON: {e}"))?;
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("web_search tool call missing query argument"))?
                .to_string();

            // Defensive: web_search is only advertised when configured, but if a call
            // arrives while it's None, feed back a tool result instead of erroring.
            let results = match searxng_url {
                Some(searxng_url) => {
                    tracing::info!("web_search: query={:?} model={}", query, model);
                    crate::search::web_search(client, searxng_url, &query).await?
                }
                None => {
                    tracing::warn!("web_search called but SearXNG is not configured");
                    "web search is not available".to_string()
                }
            };

            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: Some(results),
                tool_calls: None,
                tool_call_id: Some(call.id),
            });
        }
        // Falls through to the next loop iteration so the model can use the tool results.
    }

    Err(anyhow!("tool call loop exceeded maximum iterations"))
}
