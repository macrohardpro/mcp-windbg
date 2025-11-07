//! CDB 进程管理模块
//!
//! 提供 CDB 进程的启动、命令执行和输出解析功能。

use crate::error::CdbError;
use crate::utils;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// 会话类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionType {
    /// 转储文件会话
    Dump,
    /// 远程调试会话
    Remote,
}

/// CDB 会话
///
/// 表示一个活跃的 CDB 进程实例，用于调试转储文件或远程目标。
pub struct CdbSession {
    /// 会话唯一标识符
    session_id: String,
    /// CDB 子进程
    process: Child,
    /// 标准输入流
    stdin: ChildStdin,
    /// 标准输出读取器（使用 Arc<Mutex> 以支持并发读取）
    stdout_reader: Arc<Mutex<BufReader<ChildStdout>>>,
    /// 命令执行超时时间
    timeout: Duration,
    /// 是否启用详细日志
    verbose: bool,
    /// 会话类型
    session_type: SessionType,
}

impl CdbSession {
    /// 创建新的 CDB 会话（崩溃转储）
    ///
    /// # 参数
    /// * `dump_path` - 转储文件路径
    /// * `cdb_path` - 可选的自定义 CDB 路径
    /// * `symbols_path` - 可选的符号路径
    /// * `timeout` - 命令执行超时时间
    /// * `verbose` - 是否启用详细日志
    ///
    /// # 返回
    /// 返回新创建的 CDB 会话
    ///
    /// # 错误
    /// 如果 CDB 可执行文件未找到或进程启动失败，返回错误
    pub async fn new_dump(
        dump_path: &Path,
        cdb_path: Option<&Path>,
        symbols_path: Option<&str>,
        timeout: Duration,
        verbose: bool,
    ) -> Result<Self, CdbError> {
        // 查找 CDB 可执行文件
        let cdb_exe = utils::find_cdb_executable(cdb_path).ok_or(CdbError::ExecutableNotFound)?;

        info!("Using CDB: {}", cdb_exe.display());
        info!("Opening dump file: {}", dump_path.display());

        // 构建命令
        let mut cmd = Command::new(&cdb_exe);
        cmd.arg("-z") // 打开转储文件
            .arg(dump_path)
            .arg("-c") // 初始命令
            .arg(".echo CDB_READY") // 启动完成标记
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // 设置符号路径
        if let Some(sym_path) = symbols_path {
            cmd.env("_NT_SYMBOL_PATH", sym_path);
        }

        // 启动进程
        let mut process = cmd
            .spawn()
            .map_err(|e| CdbError::ProcessStartFailed(e.to_string()))?;

        // 获取 stdin 和 stdout
        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| CdbError::ProcessStartFailed("Failed to get stdin".to_string()))?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| CdbError::ProcessStartFailed("Failed to get stdout".to_string()))?;

        let stdout_reader = Arc::new(Mutex::new(BufReader::new(stdout)));

        // 生成会话 ID（使用绝对路径）
        let session_id = dump_path
            .canonicalize()
            .unwrap_or_else(|_| dump_path.to_path_buf())
            .to_string_lossy()
            .to_string();

        let mut session = Self {
            session_id,
            process,
            stdin,
            stdout_reader,
            timeout,
            verbose,
            session_type: SessionType::Dump,
        };

        // 等待 CDB 启动完成
        session.wait_for_ready().await?;

        info!("CDB session started");

        Ok(session)
    }

    /// 创建新的 CDB 会话（远程调试）
    ///
    /// # 参数
    /// * `connection_string` - 远程连接字符串（例如：tcp:Port=5005,Server=192.168.0.100）
    /// * `cdb_path` - 可选的自定义 CDB 路径
    /// * `symbols_path` - 可选的符号路径
    /// * `timeout` - 命令执行超时时间
    /// * `verbose` - 是否启用详细日志
    ///
    /// # 返回
    /// 返回新创建的 CDB 会话
    ///
    /// # 错误
    /// 如果 CDB 可执行文件未找到或进程启动失败，返回错误
    pub async fn new_remote(
        connection_string: &str,
        cdb_path: Option<&Path>,
        symbols_path: Option<&str>,
        timeout: Duration,
        verbose: bool,
    ) -> Result<Self, CdbError> {
        // 查找 CDB 可执行文件
        let cdb_exe = utils::find_cdb_executable(cdb_path).ok_or(CdbError::ExecutableNotFound)?;

        info!("Using CDB: {}", cdb_exe.display());
        info!("Connecting to remote target: {}", connection_string);

        // 构建命令
        let mut cmd = Command::new(&cdb_exe);
        cmd.arg("-remote") // 远程调试
            .arg(connection_string)
            .arg("-c") // 初始命令
            .arg(".echo CDB_READY") // 启动完成标记
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // 设置符号路径
        if let Some(sym_path) = symbols_path {
            cmd.env("_NT_SYMBOL_PATH", sym_path);
        }

        // 启动进程
        let mut process = cmd
            .spawn()
            .map_err(|e| CdbError::ProcessStartFailed(e.to_string()))?;

        // 获取 stdin 和 stdout
        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| CdbError::ProcessStartFailed("Failed to get stdin".to_string()))?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| CdbError::ProcessStartFailed("Failed to get stdout".to_string()))?;

        let stdout_reader = Arc::new(Mutex::new(BufReader::new(stdout)));

        // 使用连接字符串作为会话 ID
        let session_id = connection_string.to_string();

        let mut session = Self {
            session_id,
            process,
            stdin,
            stdout_reader,
            timeout,
            verbose,
            session_type: SessionType::Remote,
        };

        // 等待 CDB 启动完成
        session.wait_for_ready().await?;

        info!("CDB remote session started");

        Ok(session)
    }

    /// 获取会话 ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 等待 CDB 启动完成
    ///
    /// 读取输出直到看到 "CDB_READY" 标记
    async fn wait_for_ready(&mut self) -> Result<(), CdbError> {
        debug!("Waiting for CDB to start...");

        let mut reader = self.stdout_reader.lock().await;
        let mut line = String::new();

        // 使用超时等待启动完成
        let wait_result = tokio::time::timeout(self.timeout, async {
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        // EOF
                        return Err(CdbError::ProcessTerminated);
                    }
                    Ok(_) => {
                        if self.verbose {
                            debug!("CDB output: {}", line.trim());
                        }
                        if line.contains("CDB_READY") {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        return Err(CdbError::IoError(e));
                    }
                }
            }
        })
        .await;

        match wait_result {
            Ok(result) => result,
            Err(_) => Err(CdbError::CommandTimeout(self.timeout)),
        }
    }

    /// 发送命令并等待输出
    ///
    /// # 参数
    /// * `command` - 要执行的 WinDbg 命令
    ///
    /// # 返回
    /// 返回命令输出的行列表
    ///
    /// # 错误
    /// 如果命令发送失败、超时或进程终止，返回错误
    pub async fn send_command(&mut self, command: &str) -> Result<Vec<String>, CdbError> {
        debug!("Executing command: {}", command);

        // 构建完整命令（包含完成标记）
        const MARKER: &str = "COMMAND_COMPLETED_MARKER_12345";
        let full_command = format!("{}\n.echo {}\n", command.trim(), MARKER);

        // 发送命令
        self.stdin
            .write_all(full_command.as_bytes())
            .await
            .map_err(|e| CdbError::CommandSendFailed(e.to_string()))?;

        self.stdin
            .flush()
            .await
            .map_err(|e| CdbError::CommandSendFailed(e.to_string()))?;

        // 读取输出直到看到标记
        let output = self.read_until_marker(MARKER).await?;

        debug!("Command execution completed, {} lines of output", output.len());

        Ok(output)
    }

    /// 读取输出直到看到指定标记
    ///
    /// # 参数
    /// * `marker` - 完成标记字符串
    ///
    /// # 返回
    /// 返回读取到的输出行列表（不包含标记行）
    ///
    /// # 错误
    /// 如果读取超时或进程终止，返回错误
    async fn read_until_marker(&mut self, marker: &str) -> Result<Vec<String>, CdbError> {
        let mut output = Vec::new();
        let mut reader = self.stdout_reader.lock().await;
        let mut line = String::new();

        // 使用超时读取输出
        let read_result = tokio::time::timeout(self.timeout, async {
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        // EOF - 进程终止
                        return Err(CdbError::ProcessTerminated);
                    }
                    Ok(_) => {
                        let trimmed = line.trim();

                        if self.verbose {
                            debug!("CDB: {}", trimmed);
                        }

                        // 检查是否是完成标记
                        if trimmed.contains(marker) {
                            return Ok(output);
                        }

                        // 添加到输出（保留原始行，包括空行）
                        output.push(line.trim_end().to_string());
                    }
                    Err(e) => {
                        return Err(CdbError::IoError(e));
                    }
                }
            }
        })
        .await;

        match read_result {
            Ok(result) => result,
            Err(_) => {
                warn!("Command execution timeout ({:?})", self.timeout);
                Err(CdbError::CommandTimeout(self.timeout))
            }
        }
    }

    /// 关闭会话
    ///
    /// 发送退出命令并等待进程终止。
    ///
    /// # 返回
    /// 如果成功关闭，返回 Ok；否则返回错误
    ///
    /// # 错误
    /// 如果无法发送退出命令或进程终止失败，返回错误
    pub async fn shutdown(mut self) -> Result<(), CdbError> {
        info!("Closing CDB session: {}", self.session_id);

        // 根据会话类型发送不同的退出命令
        let quit_command = match self.session_type {
            SessionType::Dump => {
                // 转储文件会话：使用 'q' 命令退出
                "q\n"
            }
            SessionType::Remote => {
                // 远程会话：先发送 CTRL+B 分离，然后退出
                // 注意：CTRL+B 在 CDB 中是 ASCII 字符 0x02
                "\x02q\n"
            }
        };

        // 发送退出命令
        if let Err(e) = self.stdin.write_all(quit_command.as_bytes()).await {
            warn!("Failed to send quit command: {}", e);
            // 继续尝试终止进程
        }

        if let Err(e) = self.stdin.flush().await {
            warn!("Failed to flush stdin: {}", e);
        }

        // 等待进程终止（带超时）
        let wait_result = tokio::time::timeout(Duration::from_secs(5), self.process.wait()).await;

        match wait_result {
            Ok(Ok(status)) => {
                info!("CDB process exited with status: {:?}", status);
                Ok(())
            }
            Ok(Err(e)) => {
                warn!("Failed to wait for process exit: {}", e);
                // 尝试强制终止
                let _ = self.process.kill().await;
                Err(CdbError::ProcessStartFailed(format!("Failed to terminate process: {}", e)))
            }
            Err(_) => {
                warn!("Timeout waiting for process to exit, forcing termination");
                // 超时，强制终止进程
                let _ = self.process.kill().await;
                Err(CdbError::CommandTimeout(Duration::from_secs(5)))
            }
        }
    }
}

/// 实现 Drop trait 以确保资源正确释放
impl Drop for CdbSession {
    fn drop(&mut self) {
        // 尝试终止进程（如果还在运行）
        // 注意：这是同步的 drop，所以我们只能尝试 kill
        let _ = self.process.start_kill();
        debug!("CDB session Drop: {}", self.session_id);
    }
}

/// 实现 Debug trait（手动实现，因为某些字段不支持 Debug）
impl std::fmt::Debug for CdbSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdbSession")
            .field("session_id", &self.session_id)
            .field("timeout", &self.timeout)
            .field("verbose", &self.verbose)
            .field("session_type", &self.session_type)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        // 实际的 CDB 会话测试需要真实的 CDB 环境和转储文件
        // 这些测试将在集成测试中进行
        assert!(true);
    }
}
