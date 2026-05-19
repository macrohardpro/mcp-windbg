# mcp-windbg-rs

[中文文档](./README_CN.md) | English

A [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) server for Windows debugging — crash dump analysis, remote debugging, and direct program debugging via CDB.

Built with Rust and [Tokio](https://tokio.rs/) for async I/O. Ships as a single executable with no runtime dependencies.

## Features

- **Crash dump analysis** — Open `.dmp` files, auto-run `!analyze -v`, inspect threads, modules, and stack traces
- **Remote debugging** — Connect to remote debug sessions via connection strings
- **Direct program debugging** — Launch executables under CDB, set breakpoints, step through code, inspect variables
- **Session management** — Multiple concurrent debug sessions with automatic reuse
- **Configurable timeouts** — Separate timeouts for initialization (symbol loading) and command execution

## Prerequisites

- Windows 10+
- [Debugging Tools for Windows](https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/) (provides `cdb.exe`)

If CDB is not installed, you can get it via:

```bash
winget install Microsoft.WinDbg
```

The server auto-discovers CDB from Windows SDK default paths, WinDbg Preview (Microsoft Store), and `PATH`.

## Installation

### From source

```bash
cargo build --release
```

Binary: `target/release/mcp-windbg-rs.exe`

## Configuration

### VS Code / Kiro

`.vscode/mcp.json`:

```json
{
  "servers": {
    "mcp-windbg": {
      "type": "stdio",
      "command": "/path/to/mcp-windbg-rs.exe",
      "args": [],
      "env": {
        "_NT_SYMBOL_PATH": "SRV*C:\\Symbols*https://msdl.microsoft.com/download/symbols"
      }
    }
  }
}
```

### Claude Desktop / Cline / Other MCP Clients

```json
{
  "mcpServers": {
    "mcp-windbg-rs": {
      "command": "mcp-windbg-rs",
      "args": [],
      "env": {
        "_NT_SYMBOL_PATH": "SRV*C:\\Symbols*https://msdl.microsoft.com/download/symbols"
      }
    }
  }
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CDB_PATH` | Custom path to `cdb.exe` | Auto-discovered |
| `_NT_SYMBOL_PATH` | Debug symbol search path | — |
| `_NT_SOURCE_PATH` | Source file search path | — |
| `MCP_WINDBG_TIMEOUT` | Command timeout (seconds) | `30` |
| `MCP_WINDBG_INIT_TIMEOUT` | Init timeout for dump/symbol loading (seconds) | `120` |
| `MCP_WINDBG_VERBOSE` | Verbose logging (`true`/`false`) | `false` |

### CLI Options

```
mcp-windbg-rs [OPTIONS]

  --timeout <SECONDS>       Command execution timeout (default: 30)
  --init-timeout <SECONDS>  Initialization timeout (default: 120)
  --verbose                 Enable verbose logging
```

## Tools

| Tool | Description |
|------|-------------|
| `open_windbg_dump` | Open and analyze a crash dump file |
| `open_windbg_remote` | Connect to a remote debugging session |
| `launch_debug` | Launch a program under CDB for debugging |
| `run_windbg_cmd` | Execute any WinDbg/CDB command in a session |
| `close_windbg_dump` | Close a dump file session |
| `close_windbg_remote` | Close a remote debugging session |
| `close_debug` | Close a launch debug session and terminate the program |
| `list_windbg_dumps` | List `.dmp` files in a directory |

## Usage Examples

### Crash Dump Analysis

```
Analyze the crash dump at C:\dumps\app.dmp
```

### Remote Debugging

```
Connect to tcp:Port=5005,Server=192.168.0.100 and show the current state
```

### Direct Program Debugging

Launch a program, set breakpoints, step through code:

```
Launch C:\MyApp\app.exe for debugging
```

Then use `run_windbg_cmd` to control execution:

```
bp main          — Set breakpoint at main
g                — Continue execution
p                — Step over
t                — Step into
k                — Stack trace
dv               — View local variables
lsa .            — Show source at current location
```

The `launch_debug` tool supports optional parameters:

| Parameter | Type | Description |
|-----------|------|-------------|
| `program_path` | string | Path to the target program (required) |
| `arguments` | string[] | Command line arguments |
| `working_directory` | string | Working directory |
| `symbols_path` | string | PDB symbol search path |
| `source_path` | string | Source file path for source-level debugging |
| `include_stack_trace` | boolean | Include initial stack trace |
| `include_modules` | boolean | Include loaded modules list |

### Close a Session

```
Close the debug session for C:\MyApp\app.exe
```

## Troubleshooting

**CDB not found** — Install via `winget install Microsoft.WinDbg` or set `CDB_PATH` to your `cdb.exe` location.

**Symbols not loading** — Set `_NT_SYMBOL_PATH`. Recommended: `SRV*C:\Symbols*https://msdl.microsoft.com/download/symbols`

**Command timeout** — Increase with `--timeout 60` or `MCP_WINDBG_TIMEOUT=60`. Large dumps and symbol downloads may need higher `MCP_WINDBG_INIT_TIMEOUT`.

## Web Dump Debugger

The project also ships a **web-based crash dump analysis service** (`web-dump-debugger`) that provides a browser UI for uploading, analyzing, and reporting on crash dumps.

### Features

- **Web upload** — Upload `.zip`, `.7z`, or `.tar.gz` archives containing dump files through a browser
- **Automatic extraction** — Archives are extracted and scanned for `.dmp`, `.pdb`, and source files
- **LLM-powered analysis** — Uses the same `mcp_client.py` orchestration as the GitHub Actions workflow
- **Real-time progress** — Server-Sent Events (SSE) stream live analysis progress to the browser
- **HTML reports** — Markdown analysis output is rendered to HTML with syntax highlighting
- **Rate limiting** — Per-IP upload rate limiting with sliding window algorithm
- **Session management** — Isolated per-session workspaces with automatic cleanup

### Quick Start

```bash
# Build both binaries
cargo build --release

# Create a config file (edit llm.api_key first)
cp config.example.toml config.toml

# Start the web server
./target/release/web-dump-debugger --config config.toml
```

Then open `http://localhost:8080` in your browser.

### Configuration

See [config.example.toml](config.example.toml) for all options. The server supports both TOML file and environment variable configuration.

### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/` | Upload form HTML page |
| `POST` | `/upload` | Upload crash dump archive (multipart/form-data) |
| `GET` | `/progress/:id` | SSE progress stream |
| `GET` | `/report/:id` | HTML report page |
| `GET` | `/download/:id` | Download raw Markdown report |
| `GET` | `/health` | Health check (JSON) |

### Deployment

See [docs/WEB_DEPLOYMENT.md](docs/WEB_DEPLOYMENT.md) for production deployment guidance.

## Related

- [mcp-windbg (Python)](https://github.com/svnscha/mcp-windbg) — Original Python implementation
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [WinDbg Documentation](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/)

## License

[AGPL-3.0-or-later](./LICENSE)
