//! MCP WinDbg - 用于 Windows 崩溃转储分析的高性能 Model Context Protocol 服务器
//!
//! 本库提供了通过 Model Context Protocol 分析 Windows 崩溃转储和执行远程调试的核心功能。

pub mod cdb;
pub mod error;
pub mod session;
pub mod tools;
pub mod types;
pub mod utils;

// 待实现的模块
// pub mod server;
