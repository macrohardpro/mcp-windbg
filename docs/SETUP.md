# Setup Guide

## Prerequisites

1. **Build the project**
   ```bash
   cargo build --release
   ```
   The executable will be at `target/release/mcp-windbg-rs.exe`

2. **Install Debugging Tools for Windows**
   - Download from [Windows SDK](https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/)
   - Or install via `winget install Microsoft.WindowsSDK`

3. **Add to PATH** (Optional but recommended)
   ```powershell
   # Add the release directory to your PATH
   $env:PATH += ";D:\workspace\mcp-windbg\target\release"
   ```

## Configuration for Kiro IDE

Kiro uses MCP configuration files located at `.kiro/settings/mcp.json` (workspace) or `~/.kiro/settings/mcp.json` (global).

### Workspace Configuration

Create or edit `.kiro/settings/mcp.json`:

```json
{
  "mcpServers": {
    "mcp-windbg-rs": {
      "command": "D:\\workspace\\mcp-windbg\\target\\release\\mcp-windbg-rs.exe",
      "args": ["--verbose"],
      "env": {
        "_NT_SYMBOL_PATH": "SRV*C:\\Symbols*https://msdl.microsoft.com/download/symbols",
        "MCP_WINDBG_TIMEOUT": "60"
      },
      "disabled": false,
      "autoApprove": []
    }
  }
}
```

### Global Configuration

Create or edit `~/.kiro/settings/mcp.json` for system-wide access:

```json
{
  "mcpServers": {
    "mcp-windbg-rs": {
      "command": "mcp-windbg-rs",
      "args": [],
      "env": {
        "_NT_SYMBOL_PATH": "SRV*C:\\Symbols*https://msdl.microsoft.com/download/symbols"
      },
      "disabled": false
    }
  }
}
```

**Note**: If you added the executable to PATH, you can use just `mcp-windbg-rs` as the command.

## Configuration for VSCode (with Cline/Claude Dev)

### For Cline Extension

Edit your Cline settings (`.vscode/settings.json` or user settings):

```json
{
  "cline.mcpServers": {
    "mcp-windbg-rs": {
      "command": "D:\\workspace\\mcp-windbg\\target\\release\\mcp-windbg-rs.exe",
      "args": [],
      "env": {
        "_NT_SYMBOL_PATH": "SRV*C:\\Symbols*https://msdl.microsoft.com/download/symbols"
      }
    }
  }
}
```

### For Claude Dev Extension

Create or edit `~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` (macOS/Linux) or `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json` (Windows):

```json
{
  "mcpServers": {
    "mcp-windbg-rs": {
      "command": "D:\\workspace\\mcp-windbg\\target\\release\\mcp-windbg-rs.exe",
      "args": [],
      "env": {
        "_NT_SYMBOL_PATH": "SRV*C:\\Symbols*https://msdl.microsoft.com/download/symbols"
      }
    }
  }
}
```

## Verification

### Test in Kiro

1. Open Kiro IDE
2. Open the MCP Server view in the Kiro feature panel
3. You should see `mcp-windbg-rs` listed
4. Click to connect/reconnect if needed
5. In chat, try: "List available crash dumps in C:\Windows\Minidump"

### Test in VSCode

1. Open VSCode with Cline/Claude Dev extension
2. Open the extension panel
3. Start a conversation
4. Try: "Help me analyze the crash dump at C:\dumps\app.dmp"

## Configuration Options

### Environment Variables

- `CDB_PATH` - Custom path to cdb.exe (if not in default location)
- `_NT_SYMBOL_PATH` - Symbol server path (highly recommended)
- `MCP_WINDBG_TIMEOUT` - Command timeout in seconds (default: 30)
- `MCP_WINDBG_VERBOSE` - Enable verbose logging (true/false)

### Command Line Arguments

- `--timeout <SECONDS>` - Override timeout setting
- `--verbose` - Enable verbose logging
- `--help` - Show help information

## Troubleshooting

### Server Not Starting

1. **Check the executable path**
   ```powershell
   Test-Path "D:\workspace\mcp-windbg\target\release\mcp-windbg-rs.exe"
   ```

2. **Test manually**
   ```powershell
   D:\workspace\mcp-windbg\target\release\mcp-windbg-rs.exe --help
   ```

3. **Check logs**
   - Kiro: Check the MCP Server view for error messages
   - VSCode: Check the extension output panel

### CDB Not Found

If you see "CDB executable not found":

1. Verify Debugging Tools installation
2. Set `CDB_PATH` environment variable:
   ```json
   "env": {
     "CDB_PATH": "C:\\Program Files (x86)\\Windows Kits\\10\\Debuggers\\x64\\cdb.exe"
   }
   ```

### Symbol Loading Issues

If symbols fail to load:

1. Ensure `_NT_SYMBOL_PATH` is set correctly
2. Check network connectivity
3. Try downloading symbols manually first:
   ```powershell
   symchk /r C:\Windows\System32\*.dll /s SRV*C:\Symbols*https://msdl.microsoft.com/download/symbols
   ```

### Permission Issues

If you encounter permission errors:

1. Run your IDE as Administrator (for system dumps)
2. Check file permissions on dump files
3. Ensure the symbol cache directory is writable

## Example Usage

Once configured, you can ask the AI:

- "Analyze the crash dump at C:\dumps\app.dmp"
- "Connect to remote debugger at tcp:Port=5005,Server=192.168.1.100"
- "Show me the stack trace from the current dump"
- "List all crash dumps in C:\Windows\Minidump"
- "Run the !analyze -v command on the opened dump"

The AI will automatically use the MCP server to execute these debugging tasks!
