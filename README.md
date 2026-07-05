# Husk

A Discord bot backed by a local LLM. You give it a persona; it provides the shell.

Husk is a self-hosted Discord bot that chats through a local [Ollama](https://ollama.com) model, searches the web via [SearXNG](https://docs.searxng.org/), and — unlike most chat bots — **remembers across conversations**. Cold threads are distilled into durable, searchable long-term memory and relevant past context is recalled into new threads. Everything runs on your own infrastructure; no third-party API.

The bot itself is persona-agnostic — its identity is entirely the `PERSONA` you configure.

## Features

- **Threaded conversations.** @mention the bot anywhere; it opens a thread for the exchange and tracks who said what.
- **Two-tier memory.**
  - *Hot:* per-thread history in Redis (24h+ window).
  - *Long-term:* [context-forge](https://crates.io/crates/context-forge) (local SQLite + FTS5 BM25). Threads are distilled into a summary + facts and recalled into future threads, scoped to the server. Secrets are scrubbed before anything is stored, and retrieved memory is injected as clearly-labeled reference data, never as instructions.
- **`/remember`.** Use `/remember` in a thread to distill it to long-term memory immediately and archive it.
- **Web search (optional).** When a SearXNG instance is configured, the model invokes a search tool on its own when a query needs current information. Omit it and the bot runs fine without web search.
- **Configurable persona.** The system prompt is the `PERSONA` env var — set it to whatever character or assistant you want.

## How memory works

- **At each turn:** the bot queries long-term memory (server-wide) for context relevant to the message and injects the hits as a labeled reference block.
- **When a thread goes cold:** it's distilled automatically — a background sweep ~2h after the last message, with the thread's auto-archive as a backstop — or on demand via `!remember`. A dedup marker ensures a thread is only distilled once.
- **Distillation reuses your chat model.** It points at the same Ollama host's OpenAI-compatible `/v1` endpoint, so it needs no extra infrastructure and the model stays warm.

## Requirements

- A Discord bot token ([Developer Portal](https://discord.com/developers/applications)).
- An Ollama instance with a chat model pulled.
- A Redis instance.
- *Optional:* a SearXNG instance with the JSON format enabled, for web search.

## Quick start (Docker Compose)

The image is published to GitHub Container Registry: `ghcr.io/asvarnon/husk`.

```yaml
services:
  redis:
    image: redis:7-alpine
    restart: unless-stopped
    volumes:
      - redis-data:/data

  husk:
    image: ghcr.io/asvarnon/husk:latest
    restart: unless-stopped
    env_file: .env
    environment:
      - CONTEXT_FORGE_DB=/data/husk.db   # long-term memory; needs the volume below
    volumes:
      - husk-memory:/data
    depends_on:
      - redis
    # Only if Ollama runs on the host rather than in this compose project:
    extra_hosts:
      - "host.docker.internal:host-gateway"

volumes:
  redis-data:
  husk-memory:
```

Copy `.env.example` to `.env`, fill it in, then `docker compose up -d`.

## Configuration

All via environment variables (see `.env.example`). All are required except `LLM_API_KEY`, `SEARXNG_URL`, and `CONTEXT_FORGE_DB`; the bot exits at startup if a required one is missing.

| Var | Description |
|---|---|
| `DISCORD_TOKEN` | Bot token from the Discord Developer Portal |
| `LLM_BASE_URL` | OpenAI-compatible backend base URL, e.g. `http://host.docker.internal:11434` (Ollama), `http://host:8080` (llama.cpp `llama-server`), `http://host:1234` (LM Studio). Give the server root or a `…/v1` URL — both work. Use `http://` for the local case (the distiller ships no TLS — see note). |
| `LLM_MODEL` | Model name — used for both chat and distillation. e.g. `gemma2:latest` (Ollama), or the model id your runner reports. |
| `LLM_API_KEY` | Optional. Bearer token for the chat endpoint (LM Studio / hosted gateways). Local runners (Ollama, llama.cpp) ignore it. **Chat only** — the distiller has no auth. |
| `REDIS_URL` | Redis connection string, e.g. `redis://redis:6379` |
| `SEARXNG_URL` | Optional. SearXNG JSON search endpoint, e.g. `http://<host>:8888/search`. Omit to disable web search (see [Web search](#web-search-optional)). |
| `PERSONA` | Full system prompt (multiline, double-quoted in `.env`) — the bot's identity |
| `CONTEXT_FORGE_DB` | Optional. Long-term memory SQLite path. Defaults to `~/.context-forge/discord.db`. **Point it at a mounted volume or memory resets on every redeploy.** |

> **Backend-agnostic.** Husk talks to any OpenAI-compatible runner over `/v1/chat/completions`. Switch backends by changing `LLM_BASE_URL` / `LLM_MODEL` and restarting — no rebuild. The legacy `OLLAMA_HOST` / `OLLAMA_MODEL` names still work as aliases (`LLM_*` wins if both are set).
>
> **Distiller caveat:** long-term-memory distillation uses the same backend but over a no-TLS, no-auth client, so it needs a local `http://` endpoint. If you point chat at an authenticated/HTTPS gateway via `LLM_API_KEY`, distillation against that same gateway won't work until the underlying library gains TLS + auth.

### Persona

`PERSONA` is the bot's entire personality — there is no hardcoded character. Example:

```env
PERSONA="You are a terse, dry assistant in a friends' Discord. Answer directly. No preamble."
```

Update `.env` and `docker compose restart husk` to change it; no rebuild needed.

### Persona lexicon (optional)

Set `LEXICON_CONFIG` to a TOML file path. On startup, if the file doesn't exist the bot generates it automatically by asking the configured LLM to derive weighted terms from the `PERSONA`. The lexicon biases long-term memory recall toward domain-relevant entries.

Add terms live with the `/lexicon add` slash command:

```
/lexicon add term:Battle-Sister weight:0.7
/lexicon add term:Adeptus Mechanicus weight:1.0
```

### Web search (optional)

Web search is off unless `SEARXNG_URL` is set. To enable it you need a reachable
[SearXNG](https://docs.searxng.org/) instance **with the JSON format enabled** — SearXNG disables
JSON by default, so you must add `json` under `search.formats` in its `settings.yml`:

```yaml
search:
  formats:
    - html
    - json
```

A minimal way to run one alongside Husk in the same Compose project:

```yaml
  searxng:
    image: searxng/searxng:latest
    restart: unless-stopped
    volumes:
      - ./searxng:/etc/searxng        # put settings.yml here with `json` in search.formats
    # then set SEARXNG_URL=http://searxng:8080/search in .env
```

See the [SearXNG Docker docs](https://docs.searxng.org/admin/installation-docker.html) for full
setup. Leave `SEARXNG_URL` unset and the bot starts normally with web search disabled.

### Discord permissions

The bot's role needs **Send Messages**, **Create Public Threads**, **Send Messages in Threads**, and **Use Application Commands**, plus **Manage Threads** so `/remember` can archive a thread after committing it.

## Build from source

```bash
cargo build --release
# binary at target/release/husk; reads the same env vars
```

Requires a C toolchain (context-forge bundles SQLite).

## License

Apache-2.0. See [LICENSE](LICENSE).
