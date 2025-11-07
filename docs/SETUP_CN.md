# 配置指南

## 前置准备

1. **构建项目**
   ```bash
   cargo build --release
   ```
   可执行文件位于 `target/release/mcp-windbg-rs.exe`

2. **安装 Debugging Tools for Windows**
   - 从 [Windows SDK](https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/) 下载
   - 或通过 `winget install Microsoft.WindowsSDK` 安装

3. **添加到 PATH**（可选但推荐）
   ```powershell
   # 将 release 目录添加到 PATH
   $env:PATH += ";D:\workspace\mcp-windbg\target\release"
   ```

## Kiro IDE 配置

Kiro 使用位于 `.kiro/settings/mcp.json`（工作区）或 `~/.kiro/settings/mcp.json`（全局）的 MCP 配置文件。

### 工作区配置

创建或编辑 `.kiro/settings/mcp.json`：

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

### 全局配置

创建或编辑 `~/.kiro/settings/mcp.json` 以实现系统级访问：

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

**注意**：如果已将可执行文件添加到 PATH，可以直接使用 `mcp-windbg-rs` 作为命令。

## VSCode 配置（使用 Cline/Claude Dev）

### Cline 扩展

编辑 Cline 设置（`.vscode/settings.json` 或用户设置）：

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

### Claude Dev 扩展

创建或编辑 `~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json`（macOS/Linux）或 `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json`（Windows）：

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

## 验证

### 在 Kiro 中测试

1. 打开 Kiro IDE
2. 在 Kiro 功能面板中打开 MCP Server 视图
3. 应该能看到 `mcp-windbg-rs` 列出
4. 如需要，点击连接/重新连接
5. 在聊天中尝试："列出 C:\Windows\Minidump 中的崩溃转储文件"

### 在 VSCode 中测试

1. 打开安装了 Cline/Claude Dev 扩展的 VSCode
2. 打开扩展面板
3. 开始对话
4. 尝试："帮我分析 C:\dumps\app.dmp 的崩溃转储"

## 配置选项

### 环境变量

- `CDB_PATH` - 自定义 cdb.exe 路径（如果不在默认位置）
- `_NT_SYMBOL_PATH` - 符号服务器路径（强烈推荐）
- `MCP_WINDBG_TIMEOUT` - 命令超时时间（秒），默认：30
- `MCP_WINDBG_VERBOSE` - 启用详细日志（true/false）

### 命令行参数

- `--timeout <秒数>` - 覆盖超时设置
- `--verbose` - 启用详细日志
- `--help` - 显示帮助信息

## 故障排除

### 服务器无法启动

1. **检查可执行文件路径**
   ```powershell
   Test-Path "D:\workspace\mcp-windbg\target\release\mcp-windbg-rs.exe"
   ```

2. **手动测试**
   ```powershell
   D:\workspace\mcp-windbg\target\release\mcp-windbg-rs.exe --help
   ```

3. **检查日志**
   - Kiro：查看 MCP Server 视图中的错误消息
   - VSCode：查看扩展输出面板

### CDB 未找到

如果看到 "CDB executable not found"：

1. 验证 Debugging Tools 安装
2. 设置 `CDB_PATH` 环境变量：
   ```json
   "env": {
     "CDB_PATH": "C:\\Program Files (x86)\\Windows Kits\\10\\Debuggers\\x64\\cdb.exe"
   }
   ```

### 符号加载问题

如果符号加载失败：

1. 确保 `_NT_SYMBOL_PATH` 设置正确
2. 检查网络连接
3. 尝试先手动下载符号：
   ```powershell
   symchk /r C:\Windows\System32\*.dll /s SRV*C:\Symbols*https://msdl.microsoft.com/download/symbols
   ```

### 权限问题

如果遇到权限错误：

1. 以管理员身份运行 IDE（用于系统转储）
2. 检查转储文件的文件权限
3. 确保符号缓存目录可写

## 使用示例

配置完成后，你可以向 AI 提问：

- "分析 C:\dumps\app.dmp 的崩溃转储"
- "连接到 tcp:Port=5005,Server=192.168.1.100 的远程调试器"
- "显示当前转储的堆栈跟踪"
- "列出 C:\Windows\Minidump 中的所有崩溃转储"
- "在已打开的转储上运行 !analyze -v 命令"

AI 将自动使用 MCP 服务器执行这些调试任务！
