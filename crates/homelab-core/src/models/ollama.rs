use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaStatus {
    pub healthy: bool,
    pub loaded_models: Vec<LoadedModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedModel {
    pub name: String,
    pub size_vram: Option<u64>,
    pub expires_at: Option<String>,
}

/// Raw-ish Ollama `/api/ps` response shape.
/// Tool code can convert this into the domain-level `OllamaStatus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaPs {
    pub models: Vec<OllamaRunningModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaRunningModel {
    pub name: String,
    pub size_vram: Option<u64>,
    pub expires_at: Option<String>,
}
