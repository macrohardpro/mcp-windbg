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

## VSCode 配置

添加到 VSCode MCP 设置（`.vscode/mcp.json`）：

```json
{
  "servers": {
    "mcp-windbg": {
      "type": "stdio",
      "command": "D:\\workspace\\mcp-windbg\\target\\release\\mcp-windbg-rs.exe",
      "args": ["--verbose"],
      "env": {
        "_NT_SYMBOL_PATH": "SRV*D:\\Symbols*https://msdl.microsoft.com/download/symbols",
        "MCP_WINDBG_TIMEOUT": "60",
        "MCP_WINDBG_INIT_TIMEOUT": "180"
      }
    }
  },
  "inputs": []
}
```

**注意**：
- 如果已将可执行文件添加到 PATH，可以直接使用 `mcp-windbg-rs` 作为命令。
- 对于大型 dump 文件（>100MB）或符号下载较慢的情况，可以增加 `MCP_WINDBG_INIT_TIMEOUT`。

## 验证

1. 打开 VSCode
2. 打开 MCP 扩展面板
3. 应该能看到 `mcp-windbg` 列出并已连接
4. 开始对话并尝试："列出 C:\Windows\Minidump 中的崩溃转储文件"

## 配置选项

### 环境变量

- `CDB_PATH` - 自定义 cdb.exe 路径（如果不在默认位置）
- `_NT_SYMBOL_PATH` - 符号服务器路径（强烈推荐）
- `MCP_WINDBG_TIMEOUT` - 命令执行超时时间（秒），默认：30
- `MCP_WINDBG_INIT_TIMEOUT` - 初始化超时时间（秒），默认：120
- `MCP_WINDBG_VERBOSE` - 启用详细日志（true/false）

### 命令行参数

- `--timeout <秒数>` - 覆盖命令执行超时设置
- `--init-timeout <秒数>` - 覆盖初始化超时设置
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
   - VSCode：查看 MCP 扩展输出面板

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
