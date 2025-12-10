//! MCP 工具处理器模块
//!
//! 实现所有 MCP 工具的处理逻辑。

use crate::error::ToolError;
use crate::session::SessionManager;
use crate::types::*;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

/// 处理 open_windbg_dump 工具调用
///
/// 打开并分析崩溃转储文件。
///
/// # 参数
/// * `manager` - 会话管理器
/// * `params` - 工具参数
///
/// # 返回
/// 返回包含分析结果的工具响应
///
/// # 错误
/// 如果文件不存在或分析失败，返回错误
pub async fn handle_open_windbg_dump(
    manager: Arc<SessionManager>,
    params: OpenWindbgDumpParams,
) -> Result<ToolResponse, ToolError> {
    info!("Opening dump file: {}", params.dump_path);

    // 验证文件路径
    let dump_path = Path::new(&params.dump_path);
    if !dump_path.exists() {
        return Err(ToolError::InvalidParams(format!(
            "Dump file does not exist: {}",
            params.dump_path
        )));
    }

    // 获取或创建会话
    let session = manager
        .get_or_create_dump_session(dump_path, None, None)
        .await?;

    let mut session_guard = session.lock().await;

    // 构建输出
    let mut output_lines = Vec::new();
    output_lines.push(format!("# Crash Dump Analysis: {}", params.dump_path));
    output_lines.push(String::new());

    // 执行 .lastevent 命令获取崩溃信息
    debug!("Executing .lastevent command");
    output_lines.push("## Last Event".to_string());
    output_lines.push("```".to_string());
    match session_guard.send_command(".lastevent").await {
        Ok(lines) => {
            output_lines.extend(lines);
        }
        Err(e) => {
            output_lines.push(format!("Error: {}", e));
        }
    }
    output_lines.push("```".to_string());
    output_lines.push(String::new());

    // 执行 !analyze -v 命令进行详细分析
    debug!("Executing !analyze -v command");
    output_lines.push("## Detailed Analysis".to_string());
    output_lines.push("```".to_string());
    match session_guard.send_command("!analyze -v").await {
        Ok(lines) => {
            output_lines.extend(lines);
        }
        Err(e) => {
            output_lines.push(format!("Error: {}", e));
        }
    }
    output_lines.push("```".to_string());
    output_lines.push(String::new());

    // 根据参数执行可选命令
    if params.include_stack_trace {
        debug!("Executing kb command (stack trace)");
        output_lines.push("## Stack Trace".to_string());
        output_lines.push("```".to_string());
        match session_guard.send_command("kb").await {
            Ok(lines) => {
                output_lines.extend(lines);
            }
            Err(e) => {
                output_lines.push(format!("Error: {}", e));
            }
        }
        output_lines.push("```".to_string());
        output_lines.push(String::new());
    }

    if params.include_modules {
        debug!("Executing lm command (module list)");
        output_lines.push("## Loaded Modules".to_string());
        output_lines.push("```".to_string());
        match session_guard.send_command("lm").await {
            Ok(lines) => {
                output_lines.extend(lines);
            }
            Err(e) => {
                output_lines.push(format!("Error: {}", e));
            }
        }
        output_lines.push("```".to_string());
        output_lines.push(String::new());
    }

    if params.include_threads {
        debug!("Executing ~ command (thread list)");
        output_lines.push("## Thread List".to_string());
        output_lines.push("```".to_string());
        match session_guard.send_command("~").await {
            Ok(lines) => {
                output_lines.extend(lines);
            }
            Err(e) => {
                output_lines.push(format!("Error: {}", e));
            }
        }
        output_lines.push("```".to_string());
        output_lines.push(String::new());
    }

    // 格式化输出为 Markdown
    let output = output_lines.join("\n");

    info!("Dump file analysis completed");

    Ok(ToolResponse::text(output))
}

/// 处理 open_windbg_remote 工具调用
///
/// 连接到远程调试会话。
///
/// # 参数
/// * `manager` - 会话管理器
/// * `params` - 工具参数
///
/// # 返回
/// 返回包含连接信息的工具响应
///
/// # 错误
/// 如果连接失败，返回错误
pub async fn handle_open_windbg_remote(
    manager: Arc<SessionManager>,
    params: OpenWindbgRemoteParams,
) -> Result<ToolResponse, ToolError> {
    info!("Connecting to remote target: {}", params.connection_string);

    // 获取或创建会话
    let session = manager
        .get_or_create_remote_session(&params.connection_string, None, None)
        .await?;

    let mut session_guard = session.lock().await;

    // 构建输出
    let mut output_lines = Vec::new();
    output_lines.push(format!("# Remote Debugging Session: {}", params.connection_string));
    output_lines.push(String::new());

    // 执行 !peb 命令获取进程信息
    debug!("Executing !peb command");
    output_lines.push("## Process Environment Block (PEB)".to_string());
    output_lines.push("```".to_string());
    match session_guard.send_command("!peb").await {
        Ok(lines) => {
            output_lines.extend(lines);
        }
        Err(e) => {
            output_lines.push(format!("Error: {}", e));
        }
    }
    output_lines.push("```".to_string());
    output_lines.push(String::new());

    // 执行 r 命令获取寄存器信息
    debug!("Executing r command");
    output_lines.push("## Registers".to_string());
    output_lines.push("```".to_string());
    match session_guard.send_command("r").await {
        Ok(lines) => {
            output_lines.extend(lines);
        }
        Err(e) => {
            output_lines.push(format!("Error: {}", e));
        }
    }
    output_lines.push("```".to_string());
    output_lines.push(String::new());

    // 根据参数执行可选命令
    if params.include_stack_trace {
        debug!("Executing kb command (stack trace)");
        output_lines.push("## Stack Trace".to_string());
        output_lines.push("```".to_string());
        match session_guard.send_command("kb").await {
            Ok(lines) => {
                output_lines.extend(lines);
            }
            Err(e) => {
                output_lines.push(format!("Error: {}", e));
            }
        }
        output_lines.push("```".to_string());
        output_lines.push(String::new());
    }

    if params.include_modules {
        debug!("Executing lm command (module list)");
        output_lines.push("## Loaded Modules".to_string());
        output_lines.push("```".to_string());
        match session_guard.send_command("lm").await {
            Ok(lines) => {
                output_lines.extend(lines);
            }
            Err(e) => {
                output_lines.push(format!("Error: {}", e));
            }
        }
        output_lines.push("```".to_string());
        output_lines.push(String::new());
    }

    if params.include_threads {
        debug!("Executing ~ command (thread list)");
        output_lines.push("## Thread List".to_string());
        output_lines.push("```".to_string());
        match session_guard.send_command("~").await {
            Ok(lines) => {
                output_lines.extend(lines);
            }
            Err(e) => {
                output_lines.push(format!("Error: {}", e));
            }
        }
        output_lines.push("```".to_string());
        output_lines.push(String::new());
    }

    // 格式化输出为 Markdown
    let output = output_lines.join("\n");

    info!("Remote session connection completed");

    Ok(ToolResponse::text(output))
}

/// 处理 run_windbg_cmd 工具调用
///
/// 在现有会话中执行自定义 WinDbg 命令。
///
/// # 参数
/// * `manager` - 会话管理器
/// * `params` - 工具参数
///
/// # 返回
/// 返回命令输出
///
/// # 错误
/// 如果参数无效或命令执行失败，返回错误
pub async fn handle_run_windbg_cmd(
    manager: Arc<SessionManager>,
    params: RunWindbgCmdParams,
) -> Result<ToolResponse, ToolError> {
    // 验证参数
    params.validate().map_err(ToolError::InvalidParams)?;

    info!("Executing custom command: {}", params.command);

    // 根据参数类型获取会话
    let session = if let Some(dump_path) = &params.dump_path {
        let path = Path::new(dump_path);
        manager.get_or_create_dump_session(path, None, None).await?
    } else if let Some(connection_string) = &params.connection_string {
        manager
            .get_or_create_remote_session(connection_string, None, None)
            .await?
    } else {
        return Err(ToolError::InvalidParams(
            "Either dump_path or connection_string must be provided".to_string(),
        ));
    };

    let mut session_guard = session.lock().await;

    // 执行命令
    debug!("Executing command: {}", params.command);
    let output_lines = session_guard.send_command(&params.command).await?;

    // 格式化输出
    let output = format!("```\n{}\n```", output_lines.join("\n"));

    info!("Command execution completed");

    Ok(ToolResponse::text(output))
}

/// 处理 close_windbg_dump 工具调用
///
/// 关闭转储文件会话。
///
/// # 参数
/// * `manager` - 会话管理器
/// * `params` - 工具参数
///
/// # 返回
/// 返回成功消息
///
/// # 错误
/// 如果会话不存在或关闭失败，返回错误
pub async fn handle_close_windbg_dump(
    manager: Arc<SessionManager>,
    params: CloseWindbgDumpParams,
) -> Result<ToolResponse, ToolError> {
    info!("Closing dump session: {}", params.dump_path);

    // 生成会话 ID（与创建时相同的逻辑）
    let dump_path = Path::new(&params.dump_path);
    let session_id = dump_path
        .canonicalize()
        .unwrap_or_else(|_| dump_path.to_path_buf())
        .to_string_lossy()
        .to_string();

    // 关闭会话
    manager.close_session(&session_id).await?;

    info!("Dump session closed");

    Ok(ToolResponse::text(format!(
        "Dump file session closed: {}",
        params.dump_path
    )))
}

/// 处理 close_windbg_remote 工具调用
///
/// 关闭远程调试会话。
///
/// # 参数
/// * `manager` - 会话管理器
/// * `params` - 工具参数
///
/// # 返回
/// 返回成功消息
///
/// # 错误
/// 如果会话不存在或关闭失败，返回错误
pub async fn handle_close_windbg_remote(
    manager: Arc<SessionManager>,
    params: CloseWindbgRemoteParams,
) -> Result<ToolResponse, ToolError> {
    info!("Closing remote session: {}", params.connection_string);

    // 关闭会话
    manager.close_session(&params.connection_string).await?;

    info!("Remote session closed");

    Ok(ToolResponse::text(format!(
        "Remote debugging session closed: {}",
        params.connection_string
    )))
}

/// 处理 list_windbg_dumps 工具调用
///
/// 列出目录中的转储文件。
///
/// # 参数
/// * `params` - 工具参数
///
/// # 返回
/// 返回转储文件列表
///
/// # 错误
/// 如果目录不存在或搜索失败，返回错误
pub async fn handle_list_windbg_dumps(
    params: ListWindbgDumpsParams,
) -> Result<ToolResponse, ToolError> {
    info!("Listing dump files");

    // 确定搜索目录
    let search_dir = if let Some(dir_path) = &params.directory_path {
        Path::new(dir_path).to_path_buf()
    } else {
        // 使用默认转储路径
        crate::utils::get_local_dumps_path()
            .ok_or_else(|| ToolError::InternalError("Unable to determine default dump directory".to_string()))?
    };

    debug!("Searching directory: {}", search_dir.display());

    // 检查目录是否存在
    if !search_dir.exists() {
        return Err(ToolError::InvalidParams(format!(
            "Directory does not exist: {}",
            search_dir.display()
        )));
    }

    // 搜索转储文件
    let dump_files = crate::utils::find_dump_files(&search_dir, params.recursive)?;

    // 格式化输出
    let mut output_lines = Vec::new();
    output_lines.push(format!("# Dump File List: {}", search_dir.display()));
    output_lines.push(String::new());

    if dump_files.is_empty() {
        output_lines.push("No dump files found.".to_string());
    } else {
        output_lines.push(format!("Found {} dump files:", dump_files.len()));
        output_lines.push(String::new());

        for (i, file_info) in dump_files.iter().enumerate() {
            let size_mb = file_info.size_bytes as f64 / 1024.0 / 1024.0;
            output_lines.push(format!(
                "{}. {} ({:.2} MB)",
                i + 1,
                file_info.path.display(),
                size_mb
            ));
        }
    }

    let output = output_lines.join("\n");

    info!("Found {} dump files", dump_files.len());

    Ok(ToolResponse::text(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_handle_open_windbg_dump_file_not_found() {
        let manager = Arc::new(SessionManager::new(Duration::from_secs(30), Duration::from_secs(120), false));
        let params = OpenWindbgDumpParams {
            dump_path: "nonexistent.dmp".to_string(),
            include_stack_trace: false,
            include_modules: false,
            include_threads: false,
        };

        let result = handle_open_windbg_dump(manager, params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_run_windbg_cmd_invalid_params() {
        let manager = Arc::new(SessionManager::new(Duration::from_secs(30), Duration::from_secs(120), false));
        let params = RunWindbgCmdParams {
            dump_path: None,
            connection_string: None,
            command: "test".to_string(),
        };

        let result = handle_run_windbg_cmd(manager, params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_close_windbg_dump_not_found() {
        let manager = Arc::new(SessionManager::new(Duration::from_secs(30), Duration::from_secs(120), false));
        let params = CloseWindbgDumpParams {
            dump_path: "nonexistent.dmp".to_string(),
        };

        let result = handle_close_windbg_dump(manager, params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_list_windbg_dumps_invalid_dir() {
        let params = ListWindbgDumpsParams {
            directory_path: Some("nonexistent_dir".to_string()),
            recursive: false,
        };

        let result = handle_list_windbg_dumps(params).await;
        assert!(result.is_err());
    }
}
