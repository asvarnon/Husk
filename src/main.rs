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
    let ollama_host = std::env::var("OLLAMA_HOST").expect("OLLAMA_HOST not set");
    let ollama_model = std::env::var("OLLAMA_MODEL").expect("OLLAMA_MODEL not set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL not set ");
    let searxng_url = std::env::var("SEARXNG_URL").expect("SEARXNG_URL not set");
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
