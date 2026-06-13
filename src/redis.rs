use anyhow::Result;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// 26h — a couple hours past Discord's 24h archive timer, so the archive backstop still finds
// history if the idle sweep somehow missed the thread.
const HISTORY_TTL: u64 = 93_600;
// 30d — how long the "already distilled" dedup marker persists.
const DISTILLED_TTL: u64 = 2_592_000;
// Sorted set: member "guild:thread" -> last-activity unix ts. Swept for idle threads.
const IDLE_WATCH_KEY: &str = "discord:idle_watch";

/// Seconds since the Unix epoch.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

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

    /// Record (or refresh) a thread's last-activity time in the idle-watch set so the
    /// background sweep can find it once it goes quiet. The member encodes the guild so the
    /// sweep — which only sees the set — can rebuild the guild-scoped distill options.
    pub async fn touch_idle_watch(&mut self, guild_id: u64, thread_id: u64) -> Result<()> {
        let member = format!("{}:{}", guild_id, thread_id);
        self.conn
            .zadd::<_, _, _, ()>(IDLE_WATCH_KEY, member, now_unix())
            .await?;
        Ok(())
    }

    /// Threads whose last activity is at or before `cutoff`, as `(guild_id, thread_id)`.
    pub async fn idle_threads(&mut self, cutoff: i64) -> Result<Vec<(u64, u64)>> {
        let members: Vec<String> = self
            .conn
            .zrangebyscore(IDLE_WATCH_KEY, "-inf", cutoff)
            .await?;
        Ok(members.iter().filter_map(|m| parse_member(m)).collect())
    }

    /// Stop watching a thread for idleness.
    pub async fn clear_idle_watch(&mut self, guild_id: u64, thread_id: u64) -> Result<()> {
        let member = format!("{}:{}", guild_id, thread_id);
        self.conn.zrem::<_, _, ()>(IDLE_WATCH_KEY, member).await?;
        Ok(())
    }

    /// Whether this thread has already been distilled into long-term memory.
    pub async fn is_distilled(&mut self, thread_id: u64) -> Result<bool> {
        let key = format!("discord:distilled:{}", thread_id);
        let exists: bool = self.conn.exists(&key).await?;
        Ok(exists)
    }

    /// Mark a thread as distilled so the other triggers skip it (dedup).
    pub async fn mark_distilled(&mut self, thread_id: u64) -> Result<()> {
        let key = format!("discord:distilled:{}", thread_id);
        self.conn
            .set_ex::<_, _, ()>(&key, 1u8, DISTILLED_TTL)
            .await?;
        Ok(())
    }
}

/// Parse a `"guild:thread"` idle-watch member back into ids.
fn parse_member(member: &str) -> Option<(u64, u64)> {
    let (g, t) = member.split_once(':')?;
    Some((g.parse().ok()?, t.parse().ok()?))
}
