# Web Dump Debugger - Deployment Guide

## Overview

The Web Dump Debugger (`web-dump-debugger`) is an HTTP-based service for Windows crash dump analysis. Users upload compressed archives containing `.dmp` files, symbols, and source code through a web browser; the server extracts, analyzes using LLM orchestration, and renders an HTML report.

## System Requirements

**Minimum:**
- OS: Windows Server 2019+ or Windows 10/11
- CPU: 4 cores
- RAM: 8 GB
- Disk: 100 GB free (for temporary session workspaces)
- Python: 3.8+
- Windows Debugging Tools (`cdb.exe`)

**Recommended:**
- CPU: 8+ cores
- RAM: 16 GB+
- Disk: 500 GB SSD

## Dependencies

### 1. Rust Toolchain

Install via [rustup](https://rustup.rs/):

```bash
rustup default stable
```

### 2. Python 3.8+

Download from [python.org](https://www.python.org/downloads/) or:

```bash
winget install Python.Python.3.11
```

Install required Python packages:

```bash
pip install httpx mcp
```

### 3. Windows Debugging Tools

```bash
winget install Microsoft.WinDbg
```

Or install the full [Windows SDK](https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/).

## Build

```bash
cd mcp-windbg
cargo build --release
```

Produces:
- `target/release/mcp-windbg-rs.exe` — MCP server (used internally)
- `target/release/web-dump-debugger.exe` — Web Dump Debugger HTTP server

## Configuration

### Config File (Recommended)

Copy and edit the example:

```bash
cp config.example.toml config.toml
```

Edit `config.toml` — the minimal required fields under `[llm]`:
- `api_key` — Your LLM API key
- `api_base` — API endpoint URL
- `model` — Model name (e.g., `gpt-4`, `deepseek-chat`)

See [config.example.toml](../config.example.toml) for all options and their defaults.

### Environment Variables

As an alternative to the config file, set these environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `API_KEY` | LLM API key (required) | — |
| `API_BASE` | LLM API base URL (required) | — |
| `MODEL` | LLM model name (required) | — |
| `PORT` | HTTP server port | `8080` |
| `MAX_UPLOAD_SIZE` | Maximum upload size (bytes) | `524288000` (500MB) |
| `MAX_CONCURRENT_SESSIONS` | Maximum concurrent analyses | `5` |
| `CLEANUP_INTERVAL` | Cleanup interval (seconds) | `3600` |
| `SESSION_TTL` | Session lifetime (seconds) | `86400` |
| `MAX_TURNS` | Max LLM tool-calling turns | `30` |
| `TIMEOUT` | Analysis timeout (seconds) | `300` |
| `MCP_SERVER_PATH` | Path to `mcp-windbg-rs.exe` | `mcp-windbg-rs.exe` |
| `PYTHON_PATH` | Path to Python interpreter | `python` |
| `CDB_PATH` | Path to `cdb.exe` | Auto-discovered |
| `WORKSPACE_ROOT` | Temp workspace directory | `%TEMP%\web-dump-debugger` |
| `RATE_LIMIT_ENABLED` | Enable rate limiting (`true`/`false`) | `true` |
| `MAX_UPLOADS_PER_MINUTE` | Rate limit threshold | `3` |

## Running the Server

```bash
# With config file
./target/release/web-dump-debugger --config config.toml

# With explicit port override
./target/release/web-dump-debugger --config config.toml --port 9090
```

The server starts on `http://localhost:8080` by default.

### CLI Options

```
web-dump-debugger [OPTIONS]

  -c, --config <PATH>  Path to TOML config file
  -p, --port <PORT>    HTTP server port (overrides config)
  -h, --help           Show help
```

## Monitoring

### Health Check

```
GET /health
```

Response:

```json
{
  "status": "ok",
  "active_sessions": 2,
  "uptime_seconds": 3600
}
```

### Key Metrics to Watch

- **Active session count** — should not consistently hit `max_concurrent_sessions`
- **Upload rate** — track spikes that may indicate abuse
- **Average analysis duration** — increases may indicate LLM API slowdowns
- **Disk space** — monitor `WORKSPACE_ROOT` usage; cleanup runs every hour
- **Memory usage** — baseline ~200MB, can grow with concurrent sessions

### Logging

The server uses `tracing-subscriber` writing to stderr. For production, redirect to a file:

```bash
./web-dump-debugger --config config.toml 2> logs/server.log
```

Log levels (set via `RUST_LOG`):

```
RUST_LOG=web_dump_debugger=info ./target/release/web-dump-debugger --config config.toml
```

Available levels: `error`, `warn`, `info`, `debug`, `trace`.

## Security Hardening

### Network

1. **Bind to localhost only** — the server binds `127.0.0.1` by default. For external access, use a reverse proxy.
2. **HTTPS via reverse proxy** — place nginx or IIS in front for TLS termination.

Example nginx config:

```nginx
server {
    listen 443 ssl;
    server_name debug.example.com;

    ssl_certificate     /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_buffering off;                   # Required for SSE streaming
        proxy_read_timeout 600s;               # Long timeout for analysis
    }

    # Increase body size for large uploads
    client_max_body_size 500M;
}
```

3. **CORS** — by default, CORS is set to the server's own origin. Override only if needed.

### Filesystem

1. **Run as dedicated user** — create a `dump-debugger` service account with minimal privileges
2. **Workspace permissions** — restrict `WORKSPACE_ROOT` to the service account only
3. **Path traversal protection** — the server rejects archive entries with `..` or absolute paths

### Rate Limiting

- Enabled by default: 3 uploads/minute per IP
- Tune `max_uploads_per_minute` based on expected usage
- Monitor for abuse patterns and adjust

### LLM API Key

- Store the API key in environment variable, never hardcode in `config.toml`
- Use Windows DPAPI or a secrets manager for production
- The server never logs the API key

## Windows Service Setup

Create a Windows service using `nssm` (Non-Sucking Service Manager):

```powershell
# Install nssm
winget install nssm

# Create service
nssm install WebDumpDebugger "C:\path\to\web-dump-debugger.exe"
nssm set WebDumpDebugger AppParameters "--config C:\path\to\config.toml"
nssm set WebDumpDebugger AppDirectory "C:\path\to\mcp-windbg"
nssm set WebDumpDebugger AppStderr "C:\path\to\logs\error.log"
nssm set WebDumpDebugger AppStdout "C:\path\to\logs\server.log"

# Set environment variables
nssm set WebDumpDebugger AppEnvironmentExtra RUST_LOG=info

# Start the service
nssm start WebDumpDebugger
```

## Troubleshooting

### Server won't start

1. Verify the config file syntax:
   ```bash
   cat config.toml
   ```
2. Check Python is accessible:
   ```bash
   python --version
   ```
3. Check CDB is installed:
   ```bash
   cdb.exe -version
   ```
4. Check port is not in use:
   ```powershell
   netstat -ano | findstr :8080
   ```

### Analysis fails

1. Check the LLM API key and endpoint are correct
2. Verify the uploaded archive contains at least one `.dmp` file
3. Check disk space on the workspace drive
4. Look for errors in the server log

### Upload rejected (413)

The uploaded file exceeds `max_upload_size`. Increase it in `config.toml` or adjust the reverse proxy's `client_max_body_size`.

### Upload rejected (429)

Rate limit hit. The default is 3 uploads/minute per IP. Increase `max_uploads_per_minute` if needed.

### Progress stream disconnects

SSE connections may time out through some proxies. Ensure `proxy_read_timeout` is set high enough (5+ minutes) to cover the full analysis duration.

## Performance Tuning

- **SSD storage** — Use SSD for `WORKSPACE_ROOT`; extraction and scanning are I/O heavy
- **Increase concurrent limit** — If CPU/RAM are underutilized, raise `max_concurrent_sessions`
- **Adjust analysis timeout** — Complex dumps may need `timeout` higher than 300s
- **Symbol caching** — Set `_NT_SYMBOL_PATH` with a local cache (`SRV*C:\Symbols*...`) to avoid re-downloading symbols
- **Log level** — Use `warn` or `error` in production to reduce log volume
