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

### MCP 客户端配置

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
- [ ] 服务器配置和初始化
- [ ] 工具注册和 JSON-RPC 分发
- [ ] Stdio 传输实现
- [ ] CLI 参数解析

### 阶段 6：文档
- [ ] 使用指南和示例
- [ ] 配置参考
- [ ] 故障排除指南

详细实现任务见 [tasks.md](.kiro/specs/mcp-windbg-rs/tasks.md)。

## 许可证

MIT
