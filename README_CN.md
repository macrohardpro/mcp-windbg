# mcp-windbg-rs

中文 | [English](./README.md)

一个 [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) 服务器，用于 Windows 调试 — 崩溃转储分析、远程调试和直接程序调试，基于 CDB 实现。

使用 Rust 和 [Tokio](https://tokio.rs/) 构建，编译为单一可执行文件，无运行时依赖。

## 功能

- **崩溃转储分析** — 打开 `.dmp` 文件，自动执行 `!analyze -v`，查看线程、模块和堆栈
- **远程调试** — 通过连接字符串连接远程调试会话
- **直接程序调试** — 在 CDB 下启动程序，设置断点、单步执行、查看变量
- **会话管理** — 支持多个并发调试会话，自动复用已有会话
- **可配置超时** — 初始化超时（符号加载）和命令执行超时分别配置

## 前置要求

- Windows 10+
- [Debugging Tools for Windows](https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/)（提供 `cdb.exe`）

如果未安装 CDB，可以通过以下命令安装：

```bash
winget install Microsoft.WinDbg
```

服务器会自动从 Windows SDK 默认路径、WinDbg Preview（Microsoft Store）和 `PATH` 中查找 CDB。

## 安装

### 从源码构建

```bash
cargo build --release
```

可执行文件：`target/release/mcp-windbg-rs.exe`

## 配置

### VS Code / Kiro

`.vscode/mcp.json`：

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

### Claude Desktop / Cline / 其他 MCP 客户端

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

### 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `CDB_PATH` | 自定义 `cdb.exe` 路径 | 自动查找 |
| `_NT_SYMBOL_PATH` | 调试符号搜索路径 | — |
| `_NT_SOURCE_PATH` | 源文件搜索路径 | — |
| `MCP_WINDBG_TIMEOUT` | 命令执行超时（秒） | `30` |
| `MCP_WINDBG_INIT_TIMEOUT` | 初始化超时，用于 dump 加载和符号下载（秒） | `120` |
| `MCP_WINDBG_VERBOSE` | 详细日志（`true`/`false`） | `false` |

### 命令行选项

```
mcp-windbg-rs [选项]

  --timeout <秒数>          命令执行超时（默认：30）
  --init-timeout <秒数>     初始化超时（默认：120）
  --verbose                 启用详细日志
```

## 工具列表

| 工具 | 说明 |
|------|------|
| `open_windbg_dump` | 打开并分析崩溃转储文件 |
| `open_windbg_remote` | 连接远程调试会话 |
| `launch_debug` | 在 CDB 下启动程序进行调试 |
| `run_windbg_cmd` | 在会话中执行任意 WinDbg/CDB 命令 |
| `close_windbg_dump` | 关闭转储文件会话 |
| `close_windbg_remote` | 关闭远程调试会话 |
| `close_debug` | 关闭调试会话并终止目标程序 |
| `list_windbg_dumps` | 列出目录中的 `.dmp` 文件 |

## 使用示例

### 崩溃转储分析

```
分析 C:\dumps\app.dmp 这个崩溃转储文件
```

### 远程调试

```
连接到 tcp:Port=5005,Server=192.168.0.100 并显示当前状态
```

### 直接调试程序

启动程序、设置断点、单步执行：

```
启动 C:\MyApp\app.exe 进行调试
```

然后用 `run_windbg_cmd` 控制执行：

```
bp main          — 在 main 设置断点
g                — 继续执行
p                — 单步跳过
t                — 单步进入
k                — 查看堆栈
dv               — 查看局部变量
lsa .            — 显示当前位置的源码
```

`launch_debug` 工具支持以下可选参数：

| 参数 | 类型 | 说明 |
|------|------|------|
| `program_path` | string | 目标程序路径（必填） |
| `arguments` | string[] | 命令行参数 |
| `working_directory` | string | 工作目录 |
| `symbols_path` | string | PDB 符号搜索路径 |
| `source_path` | string | 源文件路径，用于源码级调试 |
| `include_stack_trace` | boolean | 包含初始堆栈跟踪 |
| `include_modules` | boolean | 包含已加载模块列表 |

### 关闭会话

```
关闭 C:\MyApp\app.exe 的调试会话
```

## 故障排除

**找不到 CDB** — 执行 `winget install Microsoft.WinDbg` 安装，或设置 `CDB_PATH` 指向 `cdb.exe`。

**符号加载失败** — 设置 `_NT_SYMBOL_PATH`，推荐值：`SRV*C:\Symbols*https://msdl.microsoft.com/download/symbols`

**命令超时** — 通过 `--timeout 60` 或 `MCP_WINDBG_TIMEOUT=60` 增加超时。大型 dump 和符号下载可能需要更高的 `MCP_WINDBG_INIT_TIMEOUT`。

## Web Dump Debugger

该项目还包含一个**基于 Web 的崩溃转储分析服务** (`web-dump-debugger`)，提供用于上传、分析和报告崩溃转储的浏览器界面。

### 功能

- **Web 上传** — 通过浏览器上传包含转储文件的 `.zip`、`.7z` 或 `.tar.gz` 压缩包
- **自动解压** — 自动解压并扫描 `.dmp`、`.pdb` 和源代码文件
- **LLM 分析** — 复用与 GitHub Actions 工作流相同的 `mcp_client.py` 编排逻辑
- **实时进度** — 通过 Server-Sent Events (SSE) 向浏览器流式传输实时分析进度
- **HTML 报告** — Markdown 分析结果渲染为带语法高亮的 HTML 页面
- **速率限制** — 基于 IP 的滑动窗口上传频率限制
- **会话管理** — 每个上传创建独立的工作目录，支持自动清理

### 快速开始

```bash
# 构建两个可执行文件
cargo build --release

# 创建配置文件（先修改 llm.api_key）
cp config.example.toml config.toml

# 启动 Web 服务器
./target/release/web-dump-debugger --config config.toml
```

然后在浏览器中打开 `http://localhost:8080`。

### 配置

所有配置选项请参考 [config.example.toml](config.example.toml)。服务器支持 TOML 配置文件和环境变量两种配置方式。

### API 端点

| 方法 | 路径 | 说明 |
|--------|------|------|
| `GET` | `/` | 上传表单 HTML 页面 |
| `POST` | `/upload` | 上传崩溃转储压缩包（multipart/form-data） |
| `GET` | `/progress/:id` | SSE 进度流 |
| `GET` | `/report/:id` | HTML 报告页面 |
| `GET` | `/download/:id` | 下载原始 Markdown 报告 |
| `GET` | `/health` | 健康检查（JSON） |

### 部署

生产环境部署指南请参考 [docs/WEB_DEPLOYMENT.md](docs/WEB_DEPLOYMENT.md)。

## 相关链接

- [mcp-windbg (Python)](https://github.com/svnscha/mcp-windbg) — 原始 Python 实现
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [WinDbg 文档](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/)

## 许可证

[AGPL-3.0-or-later](./LICENSE)
