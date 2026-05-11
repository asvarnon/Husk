use crate::client::HomelabClient;
use crate::error::Result;
use crate::models::proxmox::{
    ClusterResource, LxcSummary, NodeSummary, ProxmoxData, RunState, SdnSummary, StorageSummary,
    VmSummary,
};
use crate::utils::{get_timestamp_local, seconds_to_human_readable};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RawProxmoxNode {
    //raw needs to match pre-existing Proxmox API response structure
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

#[derive(Debug, Deserialize)]
struct RawVm {
    //doesnt need type field, handled by RawClusterResource enum
    vmid: u32,
    name: Option<String>,
    node: String,
    status: String,
    #[serde(default)]
    maxcpu: u32,
    #[serde(default)]
    mem: u64,
    #[serde(default)]
    maxmem: u64,
    #[serde(default)]
    uptime: u64,
}
impl From<RawVm> for VmSummary {
    fn from(value: RawVm) -> Self {
        VmSummary {
            vmid: value.vmid,
            name: value.name.unwrap_or_default(),
            node: value.node,
            status: RunState::from(value.status.as_str()),
            cpus: value.maxcpu,
            memory_mb: bytes_to_mb(value.mem),
            max_memory_mb: bytes_to_mb(value.maxmem),
            uptime: seconds_to_human_readable(value.uptime),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawLxc {
    //doesnt need type field, handled by RawClusterResource enum
    vmid: u32,
    name: Option<String>,
    node: String,
    status: String,
    #[serde(default)]
    maxcpu: u32,
    #[serde(default)]
    mem: u64,
    #[serde(default)]
    maxmem: u64,
    #[serde(default)]
    uptime: u64,
}
impl From<RawLxc> for LxcSummary {
    fn from(value: RawLxc) -> Self {
        LxcSummary {
            vmid: value.vmid,
            name: value.name.unwrap_or_default(),
            node: value.node,
            status: RunState::from(value.status.as_str()),
            cpus: value.maxcpu,
            memory_mb: bytes_to_mb(value.mem),
            max_memory_mb: bytes_to_mb(value.maxmem),
            uptime: seconds_to_human_readable(value.uptime),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawNode {
    //doesnt need type field, handled by RawClusterResource enum
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
impl From<RawNode> for NodeSummary {
    fn from(value: RawNode) -> Self {
        NodeSummary {
            node: value.node,
            status: RunState::from(value.status.as_str()),
            cpu_usage_percent: value.cpu * 100.00,
            memory_used_mb: bytes_to_mb(value.mem),
            memory_total_mb: bytes_to_mb(value.maxmem),
            uptime: seconds_to_human_readable(value.uptime),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawStorage {
    //doesnt need type field, handled by RawClusterResource enum
    node: String,
    status: String,
    #[serde(default)]
    maxdisk: u64,
    #[serde(default)]
    disk: u64,
    #[serde(default)]
    storage: String,
}
impl From<RawStorage> for StorageSummary {
    fn from(value: RawStorage) -> Self {
        StorageSummary {
            node: value.node,
            status: RunState::from(value.status.as_str()),
            maxdisk: bytes_to_mb(value.maxdisk),
            disk: bytes_to_mb(value.disk),
            storage: value.storage,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawSdn {
    //doesnt need type field, handled by RawClusterResource enum
    node: String,
    status: String,
    #[serde(default)]
    sdn: String,
}
impl From<RawSdn> for SdnSummary {
    fn from(value: RawSdn) -> Self {
        SdnSummary {
            node: value.node,
            status: RunState::from(value.status.as_str()),
            sdn: value.sdn,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")] // type is in api field (must see this before hand)
enum RawClusterResource {
    Qemu(RawVm),
    Lxc(RawLxc),
    Node(RawNode),
    Storage(RawStorage),
    Sdn(RawSdn),
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
            uptime: seconds_to_human_readable(node.uptime),
        })
        .collect())
}

pub async fn scan_cluster(client: &HomelabClient) -> Result<ClusterResource> {
    let response = client
        .get_json::<ProxmoxData<Vec<RawClusterResource>>>("proxmox", "/api2/json/cluster/resources")
        .await?;

    let mut nodes: Vec<NodeSummary> = Vec::new();
    let mut lxcs: Vec<LxcSummary> = Vec::new();
    let mut vms: Vec<VmSummary> = Vec::new();
    // let mut errors: Vec<ScanError> = Vec::new();

    for cluster in response.data {
        match cluster {
            // use ::from in the from impl functions
            RawClusterResource::Node(node) => nodes.push(NodeSummary::from(node)),
            RawClusterResource::Lxc(lxc) => lxcs.push(LxcSummary::from(lxc)),
            RawClusterResource::Qemu(vm) => vms.push(VmSummary::from(vm)),
            _ => {}
        }
    }

    Ok(ClusterResource {
        nodes,
        vms,
        lxcs,
        captured_at: get_timestamp_local(),
    })
}

fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / 1024 / 1024
}
