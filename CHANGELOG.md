# Changelog

All notable changes to Husk are recorded here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions match the `v*` git
tags that build the `ghcr.io/asvarnon/husk` image.

## [Unreleased]

### Added
- Runner-agnostic chat: the live chat/tool-call loop now speaks the OpenAI
  Chat Completions API (`/v1/chat/completions`), so Husk works against any
  OpenAI-compatible backend — Ollama, llama.cpp (`llama-server`), LM Studio, etc.
  Switch backends with config + restart, no rebuild. (#6)
- `LLM_BASE_URL` / `LLM_MODEL` config (runner-neutral). The legacy
  `OLLAMA_HOST` / `OLLAMA_MODEL` names still work as aliases (`LLM_*` wins if
  both are set). Base URL is accepted with or without a trailing `/v1`.
- `LLM_API_KEY` (optional): bearer token for the chat endpoint (LM Studio /
  hosted gateways). Local runners ignore it. Chat only — the distiller path has
  no auth.

### Fixed
- Tool calls: all tool calls in an assistant turn are now answered (each linked
  by its `tool_call_id`), instead of only the first. An unrecognized tool name
  now surfaces an explicit error instead of returning an empty reply.

### Changed
- Renamed the `ollama` module to `llm` and the `OllamaMessage` type to
  `ChatMessage` to match the backend-agnostic behavior. (No config or wire
  impact.)

## [0.1.4] - 2026-06-21

### Added
- Chunked distillation: the distiller is wrapped in context-forge 0.5.0's
  `ChunkingDistiller` (8K-char budget, `Structural` reduce), so a long thread is
  distilled in bounded pieces instead of one oversized prompt. Fixes the
  host-RAM OOM that was killing Ollama on the shared gamehost. Long transcripts
  over the inner 100K-char cap are no longer head-truncated either.
- CI workflow (`fmt` / `clippy -D warnings` / `build` / `test`) on pull requests
  to `main` and pushes to `main`.

### Changed
- Idle auto-distill timer lowered from 2h to 30 min (`IDLE_SECS`).
- Distiller request timeout raised to 600s, for hosts where a competing workload
  starves inference of CPU.
- Bumped `context-forge` pin `=0.5.0-beta.4` → `=0.5.0`.

### Fixed
- `!remember` / idle re-distill: a thread that gained new messages after an
  earlier distill is now re-distilled (only the new messages) via a per-thread
  high-water mark, replacing the permanent dedup marker that silently reported
  "nothing new" and dropped the added context.

## [0.1.3] - 2026-06-13

### Fixed
- Don't panic at startup when SearXNG isn't configured — web search is simply
  disabled instead.

## [0.1.2] - 2026-06-13

### Added
- Initial standalone release: Husk extracted from the `homelab-rs` workspace into
  its own repo and image (`ghcr.io/asvarnon/husk`). Discord bot with local-LLM
  chat (Ollama), Redis hot conversation history, long-term memory via
  context-forge (distillation, server-wide recall, `!remember`), and SearXNG web
  search — all configured via environment variables.
