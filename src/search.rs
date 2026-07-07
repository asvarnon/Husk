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
    tracing::debug!(base_url, query, "sending SearXNG request");
    let response = client
        .get(base_url)
        .query(&[("q", query), ("format", "json")])
        .send()
        .await?;
    let status = response.status();
    tracing::debug!(%status, "received SearXNG response");
    let resp: SearxResults = response.json().await?;
    tracing::debug!(hits = resp.results.len(), "parsed SearXNG results");

    let searched_at = chrono::Utc::now().to_rfc3339();
    if resp.results.is_empty() {
        return Ok(format!(
            "Search performed at {searched_at} UTC. No results found."
        ));
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

    Ok(format!(
        "Search performed at {searched_at} UTC. Results are search snippets and may contain stale page dates; do not present stale dates as current.\n\n{formatted}"
    ))
}
