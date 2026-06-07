# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.1] - 2026-06-08

### Added
- `cdb_command_timeout_secs` and `cdb_init_timeout_secs` to TOML config — MCP CDB timeouts now configurable via `config.toml` (`[llm]` section or top-level) with env var overrides (`CDB_COMMAND_TIMEOUT`, `CDB_INIT_TIMEOUT`)
- `MCP_WINDBG_TIMEOUT` and `MCP_WINDBG_INIT_TIMEOUT` env var propagation from `web-dump-debugger` → `mcp_client.py` → `mcp-windbg-rs`

### Changed
- Symbol path resolution now checks `_NT_SYMBOL_PATH` (standard Windows debugger env var) before falling back to hardcoded default
- `prefetch_symbols.ps1` respects existing `_NT_SYMBOL_PATH` and parses cache/server from it

### Fixed
- CDB command timeout (30s) and init timeout (120s) were not configurable via `config.toml` and not propagated through the MCP pipeline, causing mismatches between configured timeouts and actual runtime behavior

## [0.4.0] - 2026-05-20

### Added
- Session storage under unified `sessions/` subdirectory (`{workspace_root}/sessions/{uuid}/`)
- Configurable `max_stored_sessions` limit (default: 50) — oldest sessions auto-cleaned when exceeded
- `MAX_STORED_SESSIONS` environment variable support
- Direct CDB.exe fallback in `mcp_client.py` — when MCP tool calls timeout, spawns one-shot `cdb.exe` process to retry the command
- Overflow cleanup in background cleanup task — enforces `max_stored_sessions` limit alongside TTL cleanup

### Fixed
- Progress SSE extraction timeout — replaced fixed 30-second retry loop with adaptive `SessionStatus` polling (waits while Extracting/Uploading, exits on Failed, 10-minute hard ceiling)
- Race condition where large archive extraction (>30s) caused progress stream to incorrectly report "process failed to start"

## [0.3.0] - 2026-05-20

### Added
- Web Dump Debugger (`web-dump-debugger` binary) — HTTP-based crash dump analysis platform with web UI
- Upload crash dump archives (.zip, .7z, .tar.gz, .tgz) via browser, auto-extract and analyze with LLM orchestration
- Real-time analysis progress bar — parses AI turn progress and streams percentage via SSE (extracting→15%, analyzing→25%, per-turn→25-95%, complete→100%)
- Dual-view Chinese reports — AI generates "研发分析报告" (R&D technical analysis) and "售后支持报告" (customer support summary) with tab navigation
- Chinese web UI localization — all page text, status messages, and AI prompts in Chinese
- Session management — per-upload isolated workspaces with configurable TTL and automatic cleanup
- Per-IP rate limiting with sliding window
- Health check endpoint (`GET /health`) returning active session count and uptime
- `config.example.toml` with inline documentation for all settings
- Deployment guide (`docs/WEB_DEPLOYMENT.md`)

### Fixed
- Multipart upload truncation — file now saved before returning HTTP response, preventing connection-close truncation
- Axum default 2MB body limit — now properly overridden to match configured `max_upload_size`
- Config TOML deserialization — flattened server fields to top level for correct parsing

### Changed
- Bumped version to 0.3.0
- Rewrote AI system prompt in Chinese with structured dual-section output format
- Markdown reports rendered as styled HTML with GitHub-like CSS and view switching tabs

## [0.2.0] - 2026-03-31

### Added
- `launch_debug` tool — launch a program directly under CDB for interactive debugging
- `close_debug` tool — close a launch debug session and terminate the target program
- `program_path` parameter for `run_windbg_cmd` (three-way mutual exclusion with `dump_path` and `connection_string`)
- `symbols_path` and `source_path` parameters for `launch_debug`
- `_NT_SOURCE_PATH` environment variable support in server config
- Dynamic WinDbg Preview discovery (auto-detect any installed version from Microsoft Store)
- `winget install Microsoft.WinDbg` hint when CDB is not found

### Fixed
- CDB output with non-UTF-8 encoding (GBK/CP936 on Chinese Windows) no longer crashes the server — uses lossy UTF-8 conversion

### Changed
- Rewrote README (EN/CN) — removed roadmap, restructured as a proper open-source project
- Removed unused `config.example.toml`

## [0.1.2] - 2026-03-30

### Added
- Separate initialization timeout (default: 120s) for dump file loading and symbol downloads
- `MCP_WINDBG_INIT_TIMEOUT` environment variable and `--init-timeout` CLI flag
- Unique timestamp-based command markers to prevent output conflicts
- Output size limit (100k lines) to prevent memory overflow

### Fixed
- Timeout issues when opening large dump files (>300MB)
- Command execution failures after opening dump files
- Marker detection conflicts in command output

## [0.1.0] - 2026-03-29

### Added
- Initial release
- Crash dump analysis with `open_windbg_dump`
- Remote debugging with `open_windbg_remote`
- Custom command execution with `run_windbg_cmd`
- Session management with connection pooling
- MCP server with stdio transport
- CDB auto-discovery from Windows SDK paths
