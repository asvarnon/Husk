use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhcpLease {
    pub ip: Ipv4Addr,
    pub mac: String,
    pub hostname: Option<String>,
    pub interface: String,
    pub status: LeaseStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaseStatus {
    Active,
    Expired,
    Static,
    Reserved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpSuggestion {
    pub suggested_ip: Ipv4Addr,
    pub vlan: u16,
    pub reasoning: String,
}
