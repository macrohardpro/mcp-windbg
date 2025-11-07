# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure with Rust/Tokio
- Error type system with hierarchical error handling
- Shared type definitions for MCP protocol
- CDB executable discovery and Windows registry access
- Dump file search functionality
- CDB session management with async I/O
- Session manager with connection pooling
- Six MCP tools:
  - `open_windbg_dump` - Analyze crash dump files
  - `open_windbg_remote` - Connect to remote debugging sessions
  - `run_windbg_cmd` - Execute custom WinDbg commands
  - `close_windbg_dump` - Close dump file sessions
  - `close_windbg_remote` - Close remote debugging sessions
  - `list_windbg_dumps` - List available crash dumps
- MCP server with stdio transport
- CLI with argument parsing
- Comprehensive documentation (README, USAGE guide)
- Unit tests for all modules

## [0.1.0] - TBD

### Added
- Initial release
