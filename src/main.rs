mod handler;
mod ollama;
mod redis;
mod search;

use handler::{BotData, Handler};
use redis::RedisState;
use serenity::all::GatewayIntents;
use serenity::Client;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    let ollama_host =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://10.50.50.10:11434".to_string());
    let ollama_model =
        std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma4:12b-it-q4_K_M".to_string());
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let searxng_url = std::env::var("SEARXNG_URL")
        .unwrap_or_else(|_| "http://10.50.50.20:8888/search".to_string());
    let system_prompt: String = std::env::var("PERSONA").expect("No Persona set.");

    let redis_state = RedisState::connect(&redis_url).await?;

    let data = Arc::new(BotData {
        redis: Mutex::new(redis_state),
        ollama_host,
        ollama_model,
        searxng_url,
        system_prompt,
        http: reqwest::Client::new(),
    });

    let intents =
        GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILDS;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { data })
        .await?;

    tracing::info!("Cawl Inferior awakens");
    client.start().await?;

    Ok(())
}
