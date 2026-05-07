use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ProxmoxNode {
    pub node: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ProxmoxVm {
    pub vmid: u32,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ProxmoxLxc {
    pub vmid: u32,
    pub name: String,
    pub status: String,
}
