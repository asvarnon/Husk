# Homelab RS

Rust workspace for homelab automation tooling via MCP.

## Structure

- `homelab-core`: Pure capabilities, endpoint registry, and HTTP client.
- `homelab-mcp`: MCP adapter for Claude/AI agents.

## Getting Started

1.  Copy `.env.example` to `.env` and fill in your credentials.
2.  Configure `config.toml` with your endpoint URLs.
3.  Build the project:

```bash
cargo build
```
