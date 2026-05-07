use crate::config::{AuthConfig, Config};
use crate::error::{HomelabError, Result};
use anyhow;
use reqwest::{header, Client};

pub struct HomelabClient {
    client: Client,
    config: Config,
}

impl HomelabClient {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn fetch_endpoint<T: serde::de::DeserializeOwned>(&self, name: &str) -> Result<T> {
        let endpoint = self.config.endpoints.get(name).ok_or_else(|| {
            HomelabError::EndpointError(format!("Endpoint '{}' not found in config", name))
        })?;

        let mut request = self.client.get(&endpoint.url);

        // Apply auth
        match &endpoint.auth {
            AuthConfig::ApiToken { id_env, secret_env } => {
                let id = self.get_env_var(&id_env)?;
                let secret = self.get_env_var(&secret_env)?;
                let token = format!("PVEAPIToken={}={}", id, secret);
                request = request.header(header::AUTHORIZATION, token);
            }
            AuthConfig::Basic { user_env, pass_env } => {
                let user = self.get_env_var(&user_env)?;
                let pass = self.get_env_var(&pass_env)?;
                request = request.basic_auth(user, Some(pass));
                // Note: This is a simplification. Real basic auth might be different.
                // Or maybe the env var is just the whole header?
                // Let's assume env_var provides the value to use.
            }
            AuthConfig::None => {}
        }

        let response = request.send().await?;
        let data = response.json::<T>().await?;
        Ok(data)
    }

    fn get_env_var(&self, var_name: &str) -> Result<String> {
        std::env::var(var_name)
            .map_err(|_| HomelabError::ConfigError(format!("Env var '{}' not set", var_name)))
    }
}
