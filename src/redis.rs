use anyhow::Result;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// 26h — a couple hours past Discord's 24h archive timer, so the archive backstop still finds
// history if the idle sweep somehow missed the thread.
const HISTORY_TTL: u64 = 93_600;
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
        // Keep the distilled high-water mark alive in lockstep with the history it indexes:
        // while a thread is active both are refreshed; once it goes quiet both lapse together,
        // so a later conversation reusing the thread id distills from zero rather than against a
        // stale index. EXPIRE is a harmless no-op when the mark doesn't exist (never distilled).
        let watch_key = format!("discord:distilled_upto:{}", thread_id);
        self.conn
            .expire::<_, ()>(&watch_key, HISTORY_TTL as i64)
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

    /// How many of this thread's messages have already been distilled into long-term memory —
    /// the high-water mark. `0` if the thread has never been distilled. Lives in lockstep with
    /// the history blob (same TTL, refreshed together by `save_history`), so the mark can never
    /// outlive the messages it indexes and `mark <= history.len()` always holds while both exist.
    pub async fn distilled_upto(&mut self, thread_id: u64) -> Result<usize> {
        let key = format!("discord:distilled_upto:{}", thread_id);
        let n: Option<usize> = self.conn.get(&key).await?;
        Ok(n.unwrap_or(0))
    }

    /// Advance the distilled high-water mark to `count` messages. Called only after a successful
    /// save, so a failed distill never records progress — the thread is retried whole next time.
    /// Shares the history TTL so the two expire together.
    pub async fn set_distilled_upto(&mut self, thread_id: u64, count: usize) -> Result<()> {
        let key = format!("discord:distilled_upto:{}", thread_id);
        self.conn
            .set_ex::<_, _, ()>(&key, count, HISTORY_TTL)
            .await?;
        Ok(())
    }
}

/// Parse a `"guild:thread"` idle-watch member back into ids.
fn parse_member(member: &str) -> Option<(u64, u64)> {
    let (g, t) = member.split_once(':')?;
    Some((g.parse().ok()?, t.parse().ok()?))
}
