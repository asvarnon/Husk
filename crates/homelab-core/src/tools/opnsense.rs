use crate::client::HomelabClient;
use crate::error::Result;
use crate::models::opnsense::{DhcpLease, SearchResponse};

pub async fn get_dhcp_leases(client: &HomelabClient) -> Result<SearchResponse<DhcpLease>> {
    client
        .get_json("opnsense", "api/dnsmasq/leases/search/")
        .await
}
