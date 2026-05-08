use serde::{Deserialize, Serialize};

use super::llama::LlamaStatus;
use super::ollama::OllamaStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceStatus {
    pub llama: LlamaStatus,
    pub ollama: OllamaStatus,
}
