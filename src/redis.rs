use anyhow::Result;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

const HISTORY_TTL: u64 = 86400; // 24h — matches Discord's default thread archive timer

pub struct RedisState {
    conn: redis::aio::ConnectionManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl RedisState {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = redis::Client::open(url)?;
        let conn = redis::aio::ConnectionManager::new(client).await?;
        Ok(Self { conn })
    }

    pub async fn load_history(&mut self, thread_id: u64) -> Result<Vec<StoredMessage>> {
        let key = format!("discord:thread:{}", thread_id);
        let raw: Option<String> = self.conn.get(&key).await?;
        match raw {
            None => Ok(vec![]),
            Some(json) => Ok(serde_json::from_str(&json)?),
        }
    }

    pub async fn save_history(&mut self, thread_id: u64, history: &[StoredMessage]) -> Result<()> {
        let key = format!("discord:thread:{}", thread_id);
        let json = serde_json::to_string(history)?;
        self.conn
            .set_ex::<_, _, ()>(&key, json, HISTORY_TTL)
            .await?;
        Ok(())
    }
}
