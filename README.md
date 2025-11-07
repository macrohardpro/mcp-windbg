# MCP WinDbg (Rust Implementation)

[中文文档](./README_CN.md) | English

A high-performance Model Context Protocol server for Windows crash dump analysis and remote debugging, implemented in Rust.

## Quick Start

### Prerequisites

- Windows 10 or higher
- [Debugging Tools for Windows](https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/)
- Rust 1.70 or higher

### Installation

```bash
cargo build --release
```

### MCP Client Configuration

```json
{
    "servers": {
        "mcp_windbg_rs": {
            "type": "stdio",
            "command": "mcp-windbg-rs",
            "env": {
                "_NT_SYMBOL_PATH": "SRV*C:\\Symbols*https://msdl.microsoft.com/download/symbols"
            }
        }
    }
}
```

## Roadmap

### Phase 1: Foundation
- [x] Project structure and dependencies setup
- [x] Error type system (CdbError, SessionError, ToolError)
- [x] Shared type definitions (ToolResponse, parameters)

### Phase 2: Core Utilities
- [x] CDB executable discovery and path resolution
- [x] Windows registry access for dump file paths
- [x] Recursive dump file search functionality

### Phase 3: CDB Session Management
- [x] CdbSession implementation for dump files and remote debugging
- [x] Async command execution with timeout handling
- [x] SessionManager with connection pooling and lifecycle management

### Phase 4: MCP Tools
- [x] `open_windbg_dump` - Crash analysis with !analyze -v
- [x] `open_windbg_remote` - Remote debugging connection
- [x] `run_windbg_cmd` - Custom command execution
- [x] `close_windbg_dump` / `close_windbg_remote` - Session cleanup
- [x] `list_windbg_dumps` - Dump file discovery

### Phase 5: MCP Server
- [x] Server configuration and initialization
- [x] Tool registration and JSON-RPC dispatch
- [x] Stdio transport implementation
- [x] CLI with argument parsing

### Phase 6: Documentation
- [x] Usage guide and examples
- [x] Configuration reference
- [x] Troubleshooting guide

See [tasks.md](.kiro/specs/mcp-windbg-rs/tasks.md) for detailed implementation tasks.

## Usage

### Available Tools

- `open_windbg_dump` - Analyze crash dump files
- `open_windbg_remote` - Connect to remote debugging sessions
- `run_windbg_cmd` - Execute WinDbg commands
- `close_windbg_dump` - Close dump file sessions
- `close_windbg_remote` - Close remote debugging sessions
- `list_windbg_dumps` - List available crash dumps

### Configuration

#### Environment Variables

- `CDB_PATH` - Custom path to cdb.exe
- `_NT_SYMBOL_PATH` - Windows symbol path
- `MCP_WINDBG_TIMEOUT` - Command timeout in seconds (default: 30)
- `MCP_WINDBG_VERBOSE` - Enable verbose logging (true/false)

#### Command Line Options

```bash
mcp-windbg-rs [OPTIONS]

OPTIONS:
    --timeout <SECONDS>   Command timeout in seconds (default: 30)
    --verbose             Enable verbose logging
    --help                Print help information
```

### Usage Examples

#### Crash Dump Analysis

```
Analyze the crash dump at C:\dumps\app.dmp
```

#### Remote Debugging

```
Connect to tcp:Port=5005,Server=192.168.0.100 and show me the current thread state
```

#### Execute Custom Commands

```
Run !analyze -v on the opened dump file
```

### Troubleshooting

#### CDB Not Found

If you encounter "CDB executable not found" error:

1. Ensure Debugging Tools for Windows is installed
2. Set `CDB_PATH` environment variable to point to cdb.exe
3. Or use `--cdb-path` parameter to specify the path

#### Symbol Loading Issues

If symbols fail to load:

1. Set `_NT_SYMBOL_PATH` environment variable
2. Recommended value: `SRV*C:\Symbols*https://msdl.microsoft.com/download/symbols`
3. Ensure network connectivity for symbol downloads

#### Command Timeout

If commands timeout:

1. Increase timeout: `--timeout 60`
2. Or set environment variable: `MCP_WINDBG_TIMEOUT=60`
3. Check dump file size and symbol loading status

## Comparison with Python Version

| Feature       | Python          | Rust                 |
| ------------- | --------------- | -------------------- |
| Performance   | Good            | Excellent            |
| Memory Safety | Runtime         | Compile-time         |
| Concurrency   | asyncio         | Tokio (native async) |
| Type Safety   | Dynamic         | Static               |
| Binary Size   | Requires Python | Single executable    |
| Startup Time  | ~100ms          | ~10ms                |

## Related Links

- [Python Version](https://github.com/svnscha/mcp-windbg)
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [WinDbg Documentation](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/)

## License

AGPL-3.0-or-later

This project is licensed under the GNU Affero General Public License v3.0 or later. This means:

- ✅ You can use, modify, and distribute this software
- ✅ You must disclose the source code of any modifications
- ⚠️ **If you use this software to provide a network service, you must make the complete source code available to users of that service**
- ⚠️ Any derivative works must also be licensed under AGPL-3.0

This license prevents companies from taking this code and using it in proprietary services without contributing back to the community.

