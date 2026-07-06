mod handler;
mod llm;
mod redis;
mod search;

use context_forge::distill::openai_compat::OpenAiCompatDistiller;
use context_forge::{
    bootstrap_prompt, ChunkingDistiller, Config, ConfigLexiconScorer, ContextForge,
};
use handler::{distill_thread, BotData, Handler, HotSwapScorer};
use redis::{now_unix, RedisState};
use serenity::all::GatewayIntents;
use serenity::Client;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

// Distill a thread ~30 min after it goes quiet, while Redis is still warm.
const IDLE_SECS: i64 = 1_800;
// Check for idle threads every 5 minutes.
const SWEEP_EVERY_SECS: u64 = 300;

// Chunk budget for distillation (chars). The distiller is wrapped in a `ChunkingDistiller`
// that splits a transcript into pieces of at most this size and distills each independently,
// so a long thread no longer arrives at llama-server as one giant prompt. Peak prefill — and
// the host-RAM prompt-cache buffers sized to it — is what was tripping the OOM killer on the
// shared gamehost; bounding the per-call prompt bounds that. ~8K chars ≈ 2K tokens.
const DISTILL_CHUNK_CHARS: usize = 8_000;
// Per-request distiller timeout. With bounded chunks each call is small, but the gamehost's
// game-server process contends for CPU, so a chunk can still be slow — give it generous
// headroom rather than failing and retrying next sweep (Husk issue #4).
const DISTILL_TIMEOUT_SECS: u64 = 600;

/// First of `primary` then `alias` that is set and non-blank (trimmed). Lets the runner-neutral
/// `LLM_*` names take precedence while keeping the legacy `OLLAMA_*` names working.
fn env_alias(primary: &str, alias: &str) -> Option<String> {
    [primary, alias]
        .into_iter()
        .filter_map(|k| std::env::var(k).ok())
        .map(|v| v.trim().to_string())
        .find(|v| !v.is_empty())
}

/// Reduce a configured base URL to the server root, so `{base}/v1...` always composes whether
/// the user gave `http://host:8080` or the OpenAI-style `http://host:8080/v1`.
fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    trimmed.strip_suffix("/v1").unwrap_or(trimmed).to_string()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    // Runner-neutral config: any OpenAI-compatible backend (Ollama, llama.cpp, LM Studio, …).
    // The legacy `OLLAMA_*` names still work as aliases so existing deployments don't break.
    let llm_base_url = env_alias("LLM_BASE_URL", "OLLAMA_HOST")
        .map(|s| normalize_base_url(&s))
        .expect("Set LLM_BASE_URL (or legacy OLLAMA_HOST)");
    let llm_model =
        env_alias("LLM_MODEL", "OLLAMA_MODEL").expect("Set LLM_MODEL (or legacy OLLAMA_MODEL)");
    // Optional bearer token for the chat endpoint (LM Studio / hosted gateways). Local runners
    // ignore it. The distiller path has no auth (see distiller construction below).
    let llm_api_key = std::env::var("LLM_API_KEY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL not set ");
    // Optional: without SEARXNG_URL the bot runs fine, it just doesn't offer web search.
    // Treat a blank value (e.g. `SEARXNG_URL=` in .env) the same as absent.
    let searxng_url = std::env::var("SEARXNG_URL")
        .ok()
        .filter(|s| !s.trim().is_empty());
    if searxng_url.is_none() {
        tracing::info!("SEARXNG_URL not set — web search disabled");
    }
    let system_prompt: String = std::env::var("PERSONA").expect("No Persona set.");

    let lexicon_path: Option<PathBuf> = std::env::var("LEXICON_CONFIG")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from);

    let http = reqwest::Client::new();

    // Long-term memory store (context-forge). Durable path on the host running the bot.
    let db_path = std::env::var("CONTEXT_FORGE_DB").unwrap_or_else(|_| {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        format!("{home}/.context-forge/discord.db")
    });
    let db_path_buf = PathBuf::from(&db_path);
    if let Some(parent) = db_path_buf.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let model_cache = db_path_buf
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("models");
    // Config is #[non_exhaustive] — must build via Default then mutate (no struct literal).
    #[allow(clippy::field_reassign_with_default)]
    let cf_config = {
        let mut c = Config::default();
        c.db_path = db_path_buf;
        c.recency_half_life_secs = 2_592_000.0; // 30d — historian recall, not recency-biased
        c.max_entries = 50_000;
        c
    };
    let persona_scorer: Option<ConfigLexiconScorer> = if let Some(ref path) = lexicon_path {
        if path.exists() {
            match ConfigLexiconScorer::from_file(path) {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!("failed to load lexicon from {path:?}: {e}");
                    None
                }
            }
        } else {
            let prompt = bootstrap_prompt(&system_prompt);
            let result: anyhow::Result<ConfigLexiconScorer> = async {
                let body = serde_json::json!({
                    "model": &llm_model,
                    "messages": [{"role": "user", "content": prompt}],
                    "stream": false
                });
                let mut req = http
                    .post(format!("{llm_base_url}/v1/chat/completions"))
                    .json(&body);
                if let Some(ref key) = llm_api_key {
                    req = req.bearer_auth(key);
                }
                let val: serde_json::Value = req.send().await?.json().await?;
                let content = val["choices"][0]["message"]["content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("no content in bootstrap response"))?;
                let toml_str = {
                    let mut in_block = false;
                    let mut lines: Vec<&str> = Vec::new();
                    for line in content.lines() {
                        if !in_block && line.trim_start().starts_with("```toml") {
                            in_block = true;
                            continue;
                        }
                        if in_block {
                            if line.trim() == "```" {
                                break;
                            }
                            lines.push(line);
                        }
                    }
                    if lines.is_empty() {
                        return Err(anyhow::anyhow!("no TOML block in bootstrap response"));
                    }
                    lines.join("\n")
                };
                let scorer = toml_str
                    .parse::<ConfigLexiconScorer>()
                    .map_err(|e| anyhow::anyhow!("lexicon parse error: {e}"))?;
                std::fs::write(path, &toml_str)?;
                tracing::info!("lexicon bootstrapped and saved to {path:?}");
                Ok(scorer)
            }
            .await;
            match result {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!(
                        "lexicon bootstrap failed, continuing without persona scorer: {e}"
                    );
                    None
                }
            }
        }
    } else {
        None
    };

    let live_scorer = Arc::new(RwLock::new(persona_scorer));
    let cf = Arc::new(
        ContextForge::builder(cf_config)
            .with_embedding_model(&model_cache)
            .with_persona_scorer(HotSwapScorer(live_scorer.clone()))
            .build()
            .await?,
    );
    let embedded = cf
        .backfill_embeddings(32, |done, total| {
            tracing::info!(done, total, "embedding backfill");
        })
        .await?;
    if embedded > 0 {
        tracing::info!(embedded, "backfill complete");
    }

    // Distiller points at the SAME backend the bot chats with (its OpenAI-compat /v1 endpoint),
    // so distillation adds no infra and the model stays warm. Wrapped in a `ChunkingDistiller`:
    // the chunk budget is the caller's policy (deployment-specific — it's our host's RAM, not the
    // library's concern), so Husk supplies it here. The default `Structural` reduce keeps the
    // merge deterministic and model-free — no extra prompt, no extra OOM risk.
    // `llm_base_url` is already normalized to the server root, so `{base}/v1` is the
    // OpenAI-compat endpoint. Since context-forge 0.8.2 the distiller supports TLS (rustls) and
    // bearer auth, so `LLM_API_KEY` applies to distillation too — a hosted https:// gateway works.
    let mut inner = OpenAiCompatDistiller::new(format!("{llm_base_url}/v1"), llm_model.clone())?
        .with_timeout_secs(DISTILL_TIMEOUT_SECS);
    if let Some(ref key) = llm_api_key {
        inner = inner.with_api_key(key.as_str());
    }
    let distiller = Arc::new(ChunkingDistiller::new(inner, DISTILL_CHUNK_CHARS));

    let redis_state = RedisState::connect(&redis_url).await?;

    let data = Arc::new(BotData {
        redis: Mutex::new(redis_state),
        llm_base_url,
        llm_model,
        llm_api_key,
        searxng_url,
        system_prompt,
        http,
        cf,
        distiller,
        lexicon_path,
        live_scorer,
    });

    // Idle sweep: the PRIMARY distill trigger. Distills threads that went quiet ~30 min ago while
    // Redis still holds their history, so neither the 24h TTL nor the archive event is load-bearing.
    {
        let data = data.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(SWEEP_EVERY_SECS));
            loop {
                tick.tick().await;
                let cutoff = now_unix() - IDLE_SECS;
                let due = {
                    let mut redis = data.redis.lock().await;
                    redis.idle_threads(cutoff).await.unwrap_or_default()
                };
                for (guild_id, thread_id) in due {
                    match distill_thread(&data, guild_id, thread_id).await {
                        Ok(Some(n)) => {
                            tracing::info!("idle-distilled thread {thread_id}: {n} entries")
                        }
                        Ok(None) => {}
                        Err(e) => tracing::warn!("idle distill failed for {thread_id}: {e:?}"),
                    }
                }
            }
        });
    }

    let intents =
        GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILDS;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { data })
        .await?;

    tracing::info!("Cawl Inferior awakens");
    client.start().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_bare_url_unchanged() {
        assert_eq!(
            normalize_base_url("http://localhost:8080"),
            "http://localhost:8080"
        );
    }

    #[test]
    fn normalize_strips_trailing_slash() {
        assert_eq!(
            normalize_base_url("http://localhost:8080/"),
            "http://localhost:8080"
        );
    }

    #[test]
    fn normalize_strips_v1_suffix() {
        assert_eq!(
            normalize_base_url("http://localhost:8080/v1"),
            "http://localhost:8080"
        );
    }

    #[test]
    fn normalize_strips_v1_and_trailing_slash() {
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/"),
            "https://api.openai.com"
        );
    }

    #[test]
    fn normalize_trims_whitespace() {
        assert_eq!(
            normalize_base_url("  http://localhost:11434  "),
            "http://localhost:11434"
        );
    }
}
