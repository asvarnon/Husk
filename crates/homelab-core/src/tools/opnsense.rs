use crate::client::HomelabClient;
use crate::error::Result;
use crate::models::opnsense::DhcpLease;

pub async fn get_dhcp_leases(client: &HomelabClient) -> Result<Vec<DhcpLease>> {
    client.fetch_endpoint("opnsense_leases").await
}
