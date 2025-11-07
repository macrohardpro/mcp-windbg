# MCP WinDbg (Rust 实现版本)

中文 | [English](./README.md)

一个高性能的 Model Context Protocol (MCP) 服务器，用于 Windows 崩溃转储分析和远程调试，使用 Rust 实现。

## 快速开始

### 前置要求

- Windows 10 或更高版本
- [Debugging Tools for Windows](https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/)
- Rust 1.70 或更高版本

### 安装

```bash
cargo build --release
```

可执行文件位于 `target/release/mcp-windbg-rs.exe`

### VSCode 配置

在 VSCode MCP 设置中添加（`.vscode/mcp.json` 或用户设置）：

```json
{
  "servers": {
    "mcp-windbg": {
      "type": "stdio",
      "command": "D:\\workspace\\mcp-windbg\\target\\release\\mcp-windbg-rs.exe",
      "args": ["--verbose"],
      "env": {
        "_NT_SYMBOL_PATH": "SRV*D:\\Symbols*https://msdl.microsoft.com/download/symbols",
        "MCP_WINDBG_TIMEOUT": "120"
      }
    }
  },
  "inputs": []
}
```

## 开发路线图

### 阶段 1：基础设施
- [x] 项目结构和依赖配置
- [x] 错误类型系统（CdbError、SessionError、ToolError）
- [x] 共享类型定义（ToolResponse、参数类型）

### 阶段 2：核心工具
- [x] CDB 可执行文件发现和路径解析
- [x] Windows 注册表访问获取转储文件路径
- [x] 递归转储文件搜索功能

### 阶段 3：CDB 会话管理
- [x] CdbSession 实现（支持转储文件和远程调试）
- [x] 异步命令执行和超时处理
- [x] SessionManager 连接池和生命周期管理

### 阶段 4：MCP 工具
- [x] `open_windbg_dump` - 使用 !analyze -v 进行崩溃分析
- [x] `open_windbg_remote` - 远程调试连接
- [x] `run_windbg_cmd` - 自定义命令执行
- [x] `close_windbg_dump` / `close_windbg_remote` - 会话清理
- [x] `list_windbg_dumps` - 转储文件发现

### 阶段 5：MCP 服务器
- [x] 服务器配置和初始化
- [x] 工具注册和 JSON-RPC 分发
- [x] Stdio 传输实现
- [x] CLI 参数解析

### 阶段 6：文档
- [x] 使用指南和示例
- [x] 配置参考
- [x] 故障排除指南

详细实现任务见 [tasks.md](.kiro/specs/mcp-windbg-rs/tasks.md)。

## 使用指南

### 可用工具

- `open_windbg_dump` - 分析崩溃转储文件
- `open_windbg_remote` - 连接到远程调试会话
- `run_windbg_cmd` - 执行 WinDbg 命令
- `close_windbg_dump` - 关闭转储文件会话
- `close_windbg_remote` - 关闭远程调试会话
- `list_windbg_dumps` - 列出可用的崩溃转储文件

### 配置

#### 环境变量

- `CDB_PATH` - 自定义 cdb.exe 路径
- `_NT_SYMBOL_PATH` - Windows 符号路径
- `MCP_WINDBG_TIMEOUT` - 命令超时时间（秒），默认：30
- `MCP_WINDBG_VERBOSE` - 启用详细日志（true/false）

#### 命令行选项

```bash
mcp-windbg-rs [选项]

选项:
    --timeout <秒数>      命令超时时间（秒），默认：30
    --verbose             启用详细日志
    --help                显示帮助信息
```

### 使用示例

#### 分析崩溃转储

```
分析位于 C:\dumps\app.dmp 的崩溃转储文件
```

#### 远程调试

```
连接到 tcp:Port=5005,Server=192.168.0.100 并显示当前线程状态
```

#### 执行自定义命令

```
在已打开的转储文件上执行 !analyze -v 命令
```

### 故障排除

#### CDB 未找到

如果遇到 "CDB executable not found" 错误：

1. 确保已安装 Debugging Tools for Windows
2. 设置 `CDB_PATH` 环境变量指向 cdb.exe
3. 或使用 `--cdb-path` 参数指定路径

#### 符号加载问题

如果符号加载失败：

1. 设置 `_NT_SYMBOL_PATH` 环境变量
2. 推荐值：`SRV*C:\Symbols*https://msdl.microsoft.com/download/symbols`
3. 确保有网络连接以下载符号

#### 命令超时

如果命令执行超时：

1. 增加超时时间：`--timeout 60`
2. 或设置环境变量：`MCP_WINDBG_TIMEOUT=60`
3. 检查转储文件大小和符号加载状态

## 与 Python 版本对比

| 特性 | Python | Rust |
|------|--------|------|
| 性能 | 良好 | 优秀 |
| 内存安全 | 运行时 | 编译时 |
| 并发处理 | asyncio | Tokio (原生异步) |
| 类型安全 | 动态类型 | 静态类型 |
| 二进制大小 | 需要 Python 环境 | 单一可执行文件 |
| 启动时间 | ~100ms | ~10ms |

## 相关链接

- [Python 版本](https://github.com/svnscha/mcp-windbg)
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [WinDbg 文档](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/)

## 许可证

AGPL-3.0-or-later

本项目采用 GNU Affero 通用公共许可证 v3.0 或更高版本。这意味着：

- ✅ 你可以使用、修改和分发本软件
- ✅ 你必须公开任何修改的源代码
- ⚠️ **如果你使用本软件提供网络服务，必须向该服务的用户提供完整的源代码**
- ⚠️ 任何衍生作品也必须使用 AGPL-3.0 许可证

此许可证防止公司在不回馈社区的情况下将此代码用于专有服务。
