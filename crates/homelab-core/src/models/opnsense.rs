use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

use crate::utils::expire_to_hours_remaining;

//generic result response for opnsense api endpoints
#[derive(Debug, Deserialize)]
pub struct SearchResponse<T> {
    pub total: u32,
    #[serde(rename = "rowCount")]
    pub row_count: u32,
    pub current: u32,
    pub rows: Vec<T>, // can be any type. a list of dchp leases, etc. config struct for whichever type you need
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhcpLease {
    #[serde(rename = "address")]
    pub ip: Ipv4Addr,
    #[serde(rename = "hwaddr")]
    pub mac: String,
    pub hostname: Option<String>,
    #[serde(rename = "if")]
    pub interface: String,
    #[serde(rename = "if_descr")]
    pub interface_desc: String,
    #[serde(rename = "expire", deserialize_with = "deserialize_expire")]
    pub expire: String,
}

// not most efficient but using for learning custom deserialization
// 'de is the lifetime parameter for the deserializer,
// D is the geeneric deserializer type, allowing the function to work with any deserializer
// (e.g. serde_json, serde_yaml) that has the deserialize trait implemented
fn deserialize_expire<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let timestamp = u64::deserialize(d)?; //deserialize the timestamp from the deserializer
    Ok(expire_to_hours_remaining(timestamp)) //pass the timestamp to the expire_to_days_remaining function
}

//may or may not use below..

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpSuggestion {
    pub suggested_ip: Ipv4Addr,
    pub vlan: u16,
    pub reasoning: String,
}
