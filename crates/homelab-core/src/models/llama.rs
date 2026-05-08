use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaStatus {
    pub healthy: bool,
    pub model_loaded: Option<String>,
}

/// Raw-ish llama.cpp `/health` response shape.
/// Tool code can convert this into the domain-level `LlamaStatus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaHealth {
    pub status: String,
}
