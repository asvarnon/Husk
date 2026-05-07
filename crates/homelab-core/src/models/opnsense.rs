use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DhcpLease {
    pub ipaddr: String,
    pub hostname: Option<String>,
    pub mac: String,
}
