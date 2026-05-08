use crate::client::HomelabClient;
use crate::error::Result;
use crate::models::llama::LlamaHealth;

pub async fn get_inference_health(client: &HomelabClient) -> Result<LlamaHealth> {
    client.get_json("llama", "/health").await
}
