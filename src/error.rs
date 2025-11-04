//! MCP WinDbg 服务器的错误类型
//!
//! 本模块定义了应用程序中使用的错误层次结构。
//! 每一层都有自己的错误类型，可以通过 `From` trait 转换为更高层的错误。

use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// CDB 进程交互时可能发生的错误
#[derive(Debug, Error)]
pub enum CdbError {
    /// CDB 可执行文件在默认位置中未找到
    #[error("CDB executable not found")]
    ExecutableNotFound,

    /// 启动 CDB 进程失败
    #[error("Failed to start CDB process: {0}")]
    ProcessStartFailed(String),

    /// 命令执行超时
    #[error("Command timeout after {0:?}")]
    CommandTimeout(Duration),

    /// 向 CDB 进程发送命令失败
    #[error("Failed to send command: {0}")]
    CommandSendFailed(String),

    /// CDB 进程意外终止
    #[error("CDB process terminated unexpectedly")]
    ProcessTerminated,

    /// 与 CDB 通信时发生 I/O 错误
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// 会话管理期间可能发生的错误
#[derive(Debug, Error)]
pub enum SessionError {
    /// 请求的会话未找到
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// 创建新会话失败
    #[error("Failed to create session: {0}")]
    CreationFailed(#[from] CdbError),

    /// 指定的转储文件未找到
    #[error("Dump file not found: {0}")]
    DumpFileNotFound(PathBuf),

    /// 会话 ID 格式无效
    #[error("Invalid session ID: {0}")]
    InvalidSessionId(String),
}

/// 处理 MCP 工具调用时可能发生的错误
#[derive(Debug, Error)]
pub enum ToolError {
    /// 提供给工具的参数无效
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    /// 发生会话相关错误
    #[error("Session error: {0}")]
    SessionError(#[from] SessionError),

    /// CDB 命令执行错误
    #[error("CDB error: {0}")]
    CdbError(#[from] CdbError),

    /// 工具执行期间发生内部错误
    #[error("Internal error: {0}")]
    InternalError(String),

    /// 文件系统错误
    #[error("File system error: {0}")]
    FileSystemError(#[from] std::io::Error),
}

/// MCP 服务器中可能发生的错误
#[derive(Debug, Error)]
pub enum ServerError {
    /// MCP 协议错误
    #[error("MCP protocol error: {0}")]
    ProtocolError(String),

    /// 发生 I/O 错误
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON 序列化/反序列化错误
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// 工具执行错误
    #[error("Tool error: {0}")]
    ToolError(#[from] ToolError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdb_error_display() {
        let err = CdbError::ExecutableNotFound;
        assert_eq!(err.to_string(), "CDB executable not found");

        let err = CdbError::CommandTimeout(Duration::from_secs(30));
        assert_eq!(err.to_string(), "Command timeout after 30s");
    }

    #[test]
    fn test_session_error_from_cdb_error() {
        let cdb_err = CdbError::ProcessStartFailed("test error".to_string());
        let session_err: SessionError = cdb_err.into();
        assert!(matches!(session_err, SessionError::CreationFailed(_)));
    }

    #[test]
    fn test_tool_error_from_session_error() {
        let session_err = SessionError::SessionNotFound("test-session".to_string());
        let tool_err: ToolError = session_err.into();
        assert!(matches!(tool_err, ToolError::SessionError(_)));
    }

    #[test]
    fn test_server_error_from_tool_error() {
        let tool_err = ToolError::InvalidParams("test param".to_string());
        let server_err: ServerError = tool_err.into();
        assert!(matches!(server_err, ServerError::ToolError(_)));
    }
}
