use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct SearxResults {
    results: Vec<SearxHit>,
}

#[derive(Deserialize)]
struct SearxHit {
    title: String,
    url: String,
    content: Option<String>,
}

pub async fn web_search(client: &Client, base_url: &str, query: &str) -> Result<String> {
    let resp: SearxResults = client
        .get(base_url)
        .query(&[("q", query), ("format", "json")])
        .send()
        .await?
        .json()
        .await?;

    if resp.results.is_empty() {
        return Ok("No results found.".to_string());
    }

    let formatted = resp
        .results
        .iter()
        .take(5)
        .map(|r| {
            format!(
                "{}\n{}\n{}",
                r.title,
                r.url,
                r.content.as_deref().unwrap_or("").trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(formatted)
}
