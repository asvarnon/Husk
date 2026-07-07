use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::redis::StoredMessage;

// Circuit breaker on the tool-calling cycle: each iteration is one model round-trip, looping
// only when the model requests tools (it reads the results next round). A model that never
// settles on a final answer — endlessly calling tools — would otherwise loop forever, so cap
// the rounds. On the LAST round we send `tool_choice: "none"` to force a text answer from what
// the model has already gathered, so an over-eager model produces a reply instead of erroring
// the user out. Normal replies finish in 1-2 rounds; eager reasoning models (e.g. GLM-5.2) can
// fan out many exploratory searches, so this leaves several free rounds before the forced answer.
const MAX_TOOL_ROUNDS: usize = 6;

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
    // Reasoning models (deepseek reasoning format) split their thinking trace into this field,
    // separate from `content`. Response-only: captured for diagnostics, never echoed back into a
    // request (`skip_serializing`), so a prior turn's reasoning never re-enters the context.
    #[serde(default, skip_serializing)]
    pub reasoning_content: Option<String>,
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
    // On the final tool round this is set to "none" to force the model to stop calling tools and
    // answer. Omitted on normal rounds so the default ("auto") lets it use tools freely.
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'a str>,
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
    // Why generation stopped: "stop" (natural EOS), "length" (hit token cap), "tool_calls", etc.
    #[serde(default)]
    finish_reason: Option<String>,
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
        reasoning_content: None,
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
                reasoning_content: None,
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
                reasoning_content: None,
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
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut messages = build_messages(history, system_prompt, memory_block);
    tracing::debug!(
        model,
        base_url,
        history_len = history.len(),
        memory_chars = memory_block.map(str::len).unwrap_or(0),
        "starting chat completion"
    );

    // Only advertise web_search when SearXNG is configured, so the model never calls a tool
    // that isn't available.
    let tools: Vec<serde_json::Value> = match searxng_url {
        Some(_) => vec![web_search_tool_def()],
        None => vec![],
    };

    for round in 0..MAX_TOOL_ROUNDS {
        // Last permitted round: forbid further tool calls so the model must produce a final
        // answer from the results it already has, instead of looping into the iteration cap.
        // Only meaningful when tools are advertised; skip it when SearXNG is unconfigured so we
        // never send `tool_choice` with an empty `tools` array (some servers reject that).
        let force_answer = round + 1 == MAX_TOOL_ROUNDS && !tools.is_empty();
        let req = ChatRequest {
            model,
            messages: messages.clone(),
            tools: tools.clone(),
            tool_choice: if force_answer { Some("none") } else { None },
            stream: false,
        };

        // Diagnostic: the exact wire body, to diff against a known-good curl when a runner's
        // chat template misbehaves. Serialized only when DEBUG is on, so prod pays nothing.
        if tracing::enabled!(tracing::Level::DEBUG) {
            if let Ok(body) = serde_json::to_string(&req) {
                tracing::debug!("LLM request body: {body}");
            }
        }

        tracing::debug!(
            round = round + 1,
            max_rounds = MAX_TOOL_ROUNDS,
            message_count = req.messages.len(),
            tools_advertised = req.tools.len(),
            force_answer,
            "sending LLM request"
        );

        // Attach the bearer token only when configured — local runners need no auth.
        let started = Instant::now();
        let mut request = client.post(&url).json(&req);
        if let Some(key) = api_key {
            request = request.bearer_auth(key);
        }
        let resp: ChatResponse = request.send().await?.json().await?;
        tracing::debug!(
            round = round + 1,
            elapsed_ms = started.elapsed().as_millis(),
            "received LLM response"
        );

        let choice = resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("chat response had no choices"))?;
        let finish_reason = choice.finish_reason;
        let mut assistant_msg = choice.message;

        // No tool calls → the assistant has given its final answer.
        let tool_calls = assistant_msg.tool_calls.clone().unwrap_or_default();
        if tool_calls.is_empty() {
            let content = assistant_msg.content.unwrap_or_default();
            // Guard an empty final answer. Returning it would POST an empty Discord message —
            // "Cannot send an empty message". Known failure mode (seen on the gamehost q4 12b):
            // a reasoning model dumps its substance into `reasoning_content` and leaves `content`
            // empty. Husk reads `content` (correct — the thinking trace is not the answer), so log
            // the split for diagnosis, then surface an error rather than sending nothing.
            if content.trim().is_empty() {
                let reasoning_chars = assistant_msg
                    .reasoning_content
                    .as_deref()
                    .map(|r| r.trim().chars().count())
                    .unwrap_or(0);
                tracing::warn!(
                    "empty content from model (finish_reason={:?}, reasoning_content={} chars); \
                     the reply collapsed into reasoning with no final answer — usually a model / \
                     quant / reasoning-format issue on the serving backend",
                    finish_reason,
                    reasoning_chars
                );
                return Err(anyhow!("model returned an empty response"));
            }
            tracing::debug!(
                round = round + 1,
                finish_reason = ?finish_reason,
                response_chars = content.chars().count(),
                "LLM produced final answer"
            );
            return Ok(content);
        }

        // Echo the assistant turn that requested the tools — pushed once; it carries every
        // tool_call, and OpenAI requires each to be answered by its own tool-role message below.
        // OpenAI convention: an assistant turn with tool_calls carries *null* content; some runners
        // emit an empty string instead. Normalize empty/whitespace to None so the echoed turn
        // matches the canonical shape — spec hygiene for picky chat templates. (NOTE: this was
        // *not* confirmed to cause the empty-EOS bug — live-tested against gemma4-coding, empty,
        // null, and non-empty preamble content all produced valid follow-ups. That bug remains
        // unreproduced and appears gamehost-environment-specific; see the empty-response guard
        // above and the request-body debug log.)
        if assistant_msg
            .content
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            assistant_msg.content = None;
        }
        messages.push(assistant_msg);

        // Answer EVERY call (a model may emit several in one turn), each linked back by its
        // tool_call_id. Dispatch is a hardcoded match for now — `web_search` is the only tool;
        // a general tool registry is tracked separately (see issue).
        tracing::debug!(
            round = round + 1,
            tool_calls = tool_calls.len(),
            "LLM requested tool calls"
        );

        for call in tool_calls {
            tracing::debug!(
                tool_call_id = %call.id,
                tool_name = %call.function.name,
                "dispatching tool call"
            );
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
                    let started = Instant::now();
                    let results = crate::search::web_search(client, searxng_url, &query).await?;
                    tracing::debug!(
                        elapsed_ms = started.elapsed().as_millis(),
                        result_chars = results.chars().count(),
                        "web_search completed"
                    );
                    results
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
                reasoning_content: None,
            });
        }
        // Falls through to the next loop iteration so the model can use the tool results.
    }

    Err(anyhow!("tool call loop exceeded maximum iterations"))
}
