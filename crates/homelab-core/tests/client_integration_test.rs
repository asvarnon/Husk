use homelab_core::client::HomelabClient;
use homelab_core::config::{AuthConfig, Config, EndpointConfig};
use std::collections::HashMap;
use wiremock::matchers::{method, path, header};
use wiremock::{Mock, MockServer, ResponseTemplate};
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct TestResponse {
    message: String,
}

#[tokio::test]
async fn test_get_json_success() {
    // 1. Start a mock server
    let mock_server = MockServer::start().await;

    // 2. Setup the mock response
    let response_body = serde_json::json!({
        "message": "hello from mock"
    });

    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    // 3. Create a Config pointing to the mock server
    let mut endpoints = HashMap::new();
    endpoints.insert(
        "mock_endpoint".to_string(),
        EndpointConfig {
            name: "mock_endpoint".to_string(),
            url: mock_server.uri(),
            auth: AuthConfig::None,
            tls_insecure: false,
        },
    );

    let config = Config { endpoints };

    // 4. Create the client
    let client = HomelabClient::new(config);

    // 5. Execute the request
    let result: TestResponse = client
        .get_json("mock_endpoint", "/test")
        .await
        .expect("Failed to fetch");

    // 6. Assert the result
    assert_eq!(result.message, "hello from mock");
}

#[tokio::test]
async fn test_get_json_api_token_auth() {
    // 1. Start a mock server
    let mock_server = MockServer::start().await;

    // 2. Define env vars for the test
    let id_var = "TEST_PROXMOX_TOKEN_ID";
    let secret_var = "TEST_PROXMOX_TOKEN_SECRET";
    let id_val = "test-id";
    let secret_val = "test-secret";

    std::env::set_var(id_var, id_val);
    std::env::set_var(secret_var, secret_val);

    // 3. Setup the mock response with header verification
    // Proxmox API token header format: PVEAPIToken=USER@REALM!TOKENID=SECRET
    let expected_token = format!("PVEAPIToken={}={}", id_val, secret_val);

    Mock::given(method("GET"))
        .and(path("/api/nodes"))
        .and(header("Authorization", expected_token))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "ok"})))
        .mount(&mock_server)
        .await;

    // 4. Create a Config pointing to the mock server
    let mut endpoints = HashMap::new();
    endpoints.insert(
        "proxmox".to_string(),
        EndpointConfig {
            name: "proxmox".to_string(),
            url: mock_server.uri(),
            auth: AuthConfig::ApiToken {
                id_env: id_var.to_string(),
                secret_env: secret_var.to_string(),
            },
            tls_insecure: false,
        },
    );

    let config = Config { endpoints };

    // 5. Create the client
    let client = HomelabClient::new(config);

    // 6. Execute the request
    let result: serde_json::Value = client
        .get_json("proxmox", "/api/nodes")
        .await
        .expect("Failed to fetch with token");

    // 7. Assert the result
    assert_eq!(result["status"], "ok");

    // Cleanup env vars
    std::env::remove_var(id_var);
    std::env::remove_var(secret_var);
}
