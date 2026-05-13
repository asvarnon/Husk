use serde::Deserialize;

use crate::client::HomelabClient;
use crate::error::Result;
use crate::models::opnsense::DhcpLease;

pub async fn get_dhcp_leases(client: &HomelabClient) -> Result<Vec<DhcpLease>> {
    client
        .get_json("opnsense", "/api/dhcpv4/leases/searchLease")
        .await
}

#[derive(Debug, Deserialize)]
struct ConfigCollector {
    name: String,
    module: String,
    controller: String,
    command: String,
    description: String,
    parameters: Option<Vec<ToolParam>>,
}

#[derive(Debug, Deserialize)]
struct ToolParam {
    name: String,
    data_type: String,
    required: bool,
}
