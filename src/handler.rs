use crate::ollama;
use crate::redis::{RedisState, StoredMessage};
use anyhow::Result;
use async_trait::async_trait;
use serenity::all::{
    AutoArchiveDuration, Channel, ChannelType, Context, CreateMessage, CreateThread, EventHandler,
    Message,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

pub struct BotData {
    pub redis: Mutex<RedisState>,
    pub ollama_host: String,
    pub ollama_model: String,
    pub searxng_url: String,
    pub system_prompt: String,
    pub http: reqwest::Client,
}

pub struct Handler {
    pub data: Arc<BotData>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let bot_id = ctx.cache.current_user().id;
        let thread_id = msg.channel_id.get();
        let history = self
            .data
            .redis
            .lock()
            .await
            .load_history(thread_id)
            .await
            .unwrap_or_default();
        if history.is_empty() && !msg.mentions.iter().any(|u| u.id == bot_id) {
            return;
        }

        if let Err(e) = self.handle_mention(&ctx, &msg, history).await {
            error!(
                "error handling mention in channel {}: {:?}",
                msg.channel_id, e
            );
        }
    }
}

impl Handler {
    async fn handle_mention(
        &self,
        ctx: &Context,
        msg: &Message,
        mut history: Vec<StoredMessage>,
    ) -> Result<()> {
        let thread_channel_id = self.resolve_thread(ctx, msg).await?;

        let content = strip_bot_mention(&msg.content, ctx.cache.current_user().id.get());

        let author = msg
            .member
            .as_ref()
            .and_then(|x| x.nick.clone())
            .unwrap_or_else(|| msg.author.name.clone());

        history.push(StoredMessage {
            role: "user".to_string(),
            content,
            name: Some(author),
        });

        let _ = thread_channel_id.broadcast_typing(&ctx.http).await;

        let response = ollama::run_chat(
            &self.data.http,
            &self.data.ollama_host,
            &self.data.ollama_model,
            &history,
            &self.data.system_prompt,
            &self.data.searxng_url,
        )
        .await?;

        thread_channel_id
            .send_message(&ctx.http, CreateMessage::new().content(&response))
            .await?;

        history.push(StoredMessage {
            role: "assistant".to_string(),
            content: response,
            name: None,
        });

        let mut redis = self.data.redis.lock().await;
        redis
            .save_history(thread_channel_id.get(), &history)
            .await?;

        Ok(())
    }

    async fn resolve_thread(
        &self,
        ctx: &Context,
        msg: &Message,
    ) -> Result<serenity::all::ChannelId> {
        let channel = msg.channel_id.to_channel(&ctx.http).await?;

        let already_thread = match &channel {
            Channel::Guild(gc) => matches!(
                gc.kind,
                ChannelType::PublicThread | ChannelType::PrivateThread
            ),
            _ => false,
        };

        if already_thread {
            return Ok(msg.channel_id);
        }

        let thread_name = truncate(
            &strip_bot_mention(&msg.content, ctx.cache.current_user().id.get()),
            80,
        );
        let thread_name = if thread_name.is_empty() {
            "New conversation".to_string()
        } else {
            thread_name
        };

        let thread = msg
            .channel_id
            .create_thread_from_message(
                &ctx.http,
                msg.id,
                CreateThread::new(thread_name).auto_archive_duration(AutoArchiveDuration::OneDay),
            )
            .await?;

        Ok(thread.id)
    }
}

fn truncate(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed.chars().take(max).collect::<String>())
    }
}

fn strip_bot_mention(content: &str, bot_id: u64) -> String {
    let mention = format!("<@{}>", bot_id);
    let mention_nick = format!("<@!{}>", bot_id);
    content
        .replace(&mention, "")
        .replace(&mention_nick, "")
        .trim()
        .to_string()
}
