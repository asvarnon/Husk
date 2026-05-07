use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OllamaPsResponse {
    pub name: String,
    pub size: u64,
}
