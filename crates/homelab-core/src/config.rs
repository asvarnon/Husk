use crate::error::{HomelabError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")] //saves you from having to manually write #[serde(rename = "api-token")] on every single variant.
pub enum AuthConfig {
    ApiToken { id_env: String, secret_env: String },
    Basic { user_env: String, pass_env: String },
    Bearer { token_env: String },
    None,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EndpointConfig {
    pub url: String,
    pub name: String,
    pub auth: AuthConfig,
    #[serde(default)]
    pub tls_insecure: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConfigRaw {
    pub endpoints: Vec<EndpointConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub endpoints: HashMap<String, EndpointConfig>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content =
            std::fs::read_to_string(path).map_err(|e| HomelabError::ConfigError(e.to_string()))?;
        let raw: ConfigRaw =
            toml::from_str(&content).map_err(|e| HomelabError::ConfigError(e.to_string()))?;

        let mut endpoints = HashMap::new();
        for endpoint in raw.endpoints {
            endpoints.insert(endpoint.name.clone(), endpoint); //cloned because insert takes ownership.
        }
        Ok(Config { endpoints })
    }
}
