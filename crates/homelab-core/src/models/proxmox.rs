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
            "running" | "online" | "available" | "ok" => Self::Running,
            "stopped" | "offline" | "unavailable" => Self::Stopped,
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
    pub uptime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmSummary {
    pub vmid: u32,
    pub name: String,
    pub status: RunState,
    pub cpus: u32,
    pub memory_mb: u64,
    pub max_memory_mb: u64,
    pub node: String,
    pub uptime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcSummary {
    pub vmid: u32,
    pub name: String,
    pub status: RunState,
    pub cpus: u32,
    pub memory_mb: u64,
    pub uptime: String,
    pub max_memory_mb: u64,
    pub node: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSummary {
    pub node: String,
    pub status: RunState,
    pub maxdisk: u64,
    pub disk: u64,
    pub storage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdnSummary {
    pub status: RunState,
    pub node: String,
    pub sdn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcStatus {
    pub vmid: u32,
    pub name: String,
    pub status: RunState,
    pub uptime: String,
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_used_gb: f32,
    pub disk_total_gb: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterResource {
    pub nodes: Vec<NodeSummary>,
    pub vms: Vec<VmSummary>,
    pub lxcs: Vec<LxcSummary>,
    // pub errors: Vec<ScanError>,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanError {
    pub subsystem: String,
    pub message: String,
}

// #[derive(Debug, Deserialize)]
// struct ClusterResource {
//     pub cluster_type: String,
//     pub node: String,
//     pub status: String,
//     pub vmid: Option<u64>,
//     pub name: Option<String>,
//     pub maxcpu: f32,
//     pub mem: u64,
//     pub maxmem: u64,
// }
