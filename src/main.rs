mod handler;
mod ollama;
mod redis;
mod search;

use context_forge::distill::openai_compat::OpenAiCompatDistiller;
use context_forge::{Config, ContextForge};
use handler::{distill_thread, BotData, Handler};
use redis::{now_unix, RedisState};
use serenity::all::GatewayIntents;
use serenity::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

// Distill a thread ~2h after it goes quiet, while Redis is still warm.
const IDLE_SECS: i64 = 7_200;
// Check for idle threads every 5 minutes.
const SWEEP_EVERY_SECS: u64 = 300;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    let ollama_host = std::env::var("OLLAMA_HOST").expect("OLLAMA_HOST not set");
    let ollama_model = std::env::var("OLLAMA_MODEL").expect("OLLAMA_MODEL not set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL not set ");
    let searxng_url = std::env::var("SEARXNG_URL").expect("SEARXNG_URL not set");
    let system_prompt: String = std::env::var("PERSONA").expect("No Persona set.");

    // Long-term memory store (context-forge). Durable path on the host running the bot.
    let db_path = std::env::var("CONTEXT_FORGE_DB").unwrap_or_else(|_| {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        format!("{home}/.context-forge/discord.db")
    });
    if let Some(parent) = PathBuf::from(&db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Config is #[non_exhaustive] — must build via Default then mutate (no struct literal).
    #[allow(clippy::field_reassign_with_default)]
    let cf_config = {
        let mut c = Config::default();
        c.db_path = PathBuf::from(&db_path);
        c.recency_half_life_secs = 2_592_000.0; // 30d — historian recall, not recency-biased
        c.max_entries = 50_000;
        c
    };
    let cf = Arc::new(ContextForge::open(cf_config)?);

    // Distiller points at the SAME Ollama the bot chats with (its OpenAI-compat /v1 endpoint),
    // so distillation adds no infra and the model stays warm.
    let distiller = Arc::new(OpenAiCompatDistiller::new(
        format!("{}/v1", ollama_host.trim_end_matches('/')),
        ollama_model.clone(),
    )?);

    let redis_state = RedisState::connect(&redis_url).await?;

    let data = Arc::new(BotData {
        redis: Mutex::new(redis_state),
        ollama_host,
        ollama_model,
        searxng_url,
        system_prompt,
        http: reqwest::Client::new(),
        cf,
        distiller,
    });

    // Idle sweep: the PRIMARY distill trigger. Distills threads that went quiet ~2h ago while
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
