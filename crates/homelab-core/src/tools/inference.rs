use crate::client::HomelabClient;
use crate::error::Result;
use crate::models::llama::LlamaHealth;

pub async fn get_inference_health(client: &HomelabClient) -> Result<LlamaHealth> {
    client.fetch_endpoint("llama").await
}
