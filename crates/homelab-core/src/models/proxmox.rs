use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Proxmox API response wrapper: most Proxmox endpoints return `{ "data": ... }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxmoxData<T> {
    pub data: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Running,
    Stopped,
    Paused,
    Suspended,
    Unknown,
}

impl From<&str> for RunState {
    fn from(value: &str) -> Self {
        match value {
            "running" | "online" => Self::Running,
            "stopped" | "offline" => Self::Stopped,
            "paused" => Self::Paused,
            "suspended" => Self::Suspended,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSummary {
    pub node: String,
    pub status: RunState,
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmSummary {
    pub vmid: u32,
    pub name: String,
    pub status: RunState,
    pub cpus: u32,
    pub memory_mb: u64,
    pub node: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcSummary {
    pub vmid: u32,
    pub name: String,
    pub status: RunState,
    pub cpus: u32,
    pub memory_mb: u64,
    pub node: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcStatus {
    pub vmid: u32,
    pub name: String,
    pub status: RunState,
    pub uptime_seconds: u64,
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_used_gb: f32,
    pub disk_total_gb: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSnapshot {
    pub nodes: Vec<NodeSummary>,
    pub vms: Vec<VmSummary>,
    pub lxcs: Vec<LxcSummary>,
    pub errors: Vec<ScanError>,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanError {
    pub subsystem: String,
    pub message: String,
}
