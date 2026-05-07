use crate::client::HomelabClient;
use crate::error::Result;
use crate::models::proxmox::{ProxmoxNode, ProxmoxVm};

pub async fn scan_nodes(client: &HomelabClient) -> Result<Vec<ProxmoxNode>> {
    client.fetch_endpoint("proxmox").await
}

pub async fn get_vms(client: &HomelabClient) -> Result<Vec<ProxmoxVm>> {
    client.fetch_endpoint("proxmox_vms").await
}
