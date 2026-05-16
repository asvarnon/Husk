use crate::config::{AuthConfig, Config};
use crate::error::{HomelabError, Result};
use reqwest::{header, Client};
use serde::de::DeserializeOwned;

pub struct HomelabClient {
    client: Client,
    config: Config,
}

impl HomelabClient {
    pub fn new(config: Config) -> Self {
        let tls_insecure = config
            .endpoints
            .values()
            .any(|endpoint| endpoint.tls_insecure);
        let client = Client::builder()
            .danger_accept_invalid_certs(tls_insecure)
            .build()
            .expect("failed to build reqwest client");

        Self { client, config }
    }

    pub async fn get_json<T: DeserializeOwned>(
        &self,
        endpoint_name: &str,
        path: &str,
    ) -> Result<T> {
        let endpoint = self.config.endpoints.get(endpoint_name).ok_or_else(|| {
            HomelabError::EndpointError(format!("Endpoint '{}' not found in config", endpoint_name))
        })?;

        let url = format!(
            "{}/{}",
            endpoint.url.trim_end_matches('/'),
            path.trim_start_matches('/'),
        );
        dbg!(&url);

        let mut request = self.client.get(&url);

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
            AuthConfig::Bearer { token_env } => {
                let token = self.get_env_var(token_env)?;
                request = request.bearer_auth(token);
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
