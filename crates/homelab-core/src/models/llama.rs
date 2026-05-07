use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LlamaHealth {
    pub status: String,
}
