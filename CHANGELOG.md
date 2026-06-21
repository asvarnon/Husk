# Changelog

All notable changes to Husk are recorded here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions match the `v*` git
tags that build the `ghcr.io/asvarnon/husk` image.

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
