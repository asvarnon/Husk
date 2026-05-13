use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Deserialize)]
pub struct SearchResponse<T> {
    pub total: u32,
    #[serde(rename = "rowCount")]
    pub row_count: u32,
    pub current: u32,
    pub rows: Vec<T>,
}


//may or may not use below..

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
