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
- [ ] Server configuration and initialization
- [ ] Tool registration and JSON-RPC dispatch
- [ ] Stdio transport implementation
- [ ] CLI with argument parsing

### Phase 6: Documentation
- [ ] Usage guide and examples
- [ ] Configuration reference
- [ ] Troubleshooting guide

See [tasks.md](.kiro/specs/mcp-windbg-rs/tasks.md) for detailed implementation tasks.

## License

MIT

