use homelab_core::tools::proxmox::scan_nodes;
use homelab_core::{Config, HomelabClient, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = std::env::var("HOMELAB_CONFIG").unwrap_or_else(|_| "config.toml".to_string());

    let config = Config::load(config_path)?;
    let client = HomelabClient::new(config);
    let nodes = scan_nodes(&client).await?;

    println!("{}", serde_json::to_string_pretty(&nodes)?);

    Ok(())
}
