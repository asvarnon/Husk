use crate::client::HomelabClient;
use crate::error::Result;
use crate::models::proxmox::{NodeSummary, ProxmoxData, RunState};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RawProxmoxNode {
    node: String,
    status: String,
    #[serde(default)]
    cpu: f32,
    #[serde(default)]
    mem: u64,
    #[serde(default)]
    maxmem: u64,
    #[serde(default)]
    uptime: u64,
}

pub async fn scan_nodes(client: &HomelabClient) -> Result<Vec<NodeSummary>> {
    let response = client
        .get_json::<ProxmoxData<Vec<RawProxmoxNode>>>("proxmox", "/api2/json/nodes")
        .await?;

    Ok(response
        .data
        .into_iter()
        .map(|node| NodeSummary {
            node: node.node,
            status: RunState::from(node.status.as_str()),
            cpu_usage_percent: node.cpu * 100.0,
            memory_used_mb: bytes_to_mb(node.mem),
            memory_total_mb: bytes_to_mb(node.maxmem),
            uptime_seconds: node.uptime,
        })
        .collect())
}

fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / 1024 / 1024
}
