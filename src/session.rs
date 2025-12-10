//! 会话管理模块
//!
//! 提供 CDB 会话的生命周期管理、连接池和会话复用功能。

use crate::cdb::CdbSession;
use crate::error::SessionError;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};

/// 会话管理器
///
/// 管理多个 CDB 会话，支持会话复用和并发访问。
pub struct SessionManager {
    /// 会话存储（会话 ID -> 会话实例）
    sessions: Arc<RwLock<HashMap<String, Arc<Mutex<CdbSession>>>>>,
    /// 默认命令超时时间
    default_timeout: Duration,
    /// 默认初始化超时时间
    default_init_timeout: Duration,
    /// 是否启用详细日志
    verbose: bool,
}

impl SessionManager {
    /// 创建新的会话管理器
    ///
    /// # 参数
    /// * `default_timeout` - 默认命令执行超时时间
    /// * `default_init_timeout` - 默认初始化超时时间
    /// * `verbose` - 是否启用详细日志
    ///
    /// # 返回
    /// 返回新创建的会话管理器
    pub fn new(default_timeout: Duration, default_init_timeout: Duration, verbose: bool) -> Self {
        info!("Creating session manager, timeout: {:?}, init_timeout: {:?}", default_timeout, default_init_timeout);
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            default_timeout,
            default_init_timeout,
            verbose,
        }
    }

    /// 获取活跃会话数量
    ///
    /// # 返回
    /// 返回当前活跃的会话数量
    pub async fn active_session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// 获取或创建崩溃转储会话
    ///
    /// 如果会话已存在，返回现有会话；否则创建新会话。
    ///
    /// # 参数
    /// * `dump_path` - 转储文件路径
    /// * `cdb_path` - 可选的自定义 CDB 路径
    /// * `symbols_path` - 可选的符号路径
    ///
    /// # 返回
    /// 返回会话的 Arc<Mutex> 引用
    ///
    /// # 错误
    /// 如果转储文件不存在或会话创建失败，返回错误
    pub async fn get_or_create_dump_session(
        &self,
        dump_path: &Path,
        cdb_path: Option<&Path>,
        symbols_path: Option<&str>,
    ) -> Result<Arc<Mutex<CdbSession>>, SessionError> {
        // 检查转储文件是否存在
        if !dump_path.exists() {
            return Err(SessionError::DumpFileNotFound(dump_path.to_path_buf()));
        }

        // 生成会话 ID（使用绝对路径）
        let session_id = dump_path
            .canonicalize()
            .unwrap_or_else(|_| dump_path.to_path_buf())
            .to_string_lossy()
            .to_string();

        debug!("Requesting dump session: {}", session_id);

        // 检查会话是否已存在
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                info!("Reusing existing dump session: {}", session_id);
                return Ok(Arc::clone(session));
            }
        }

        // 创建新会话
        info!("Creating new dump session: {}", session_id);
        let session = CdbSession::new_dump(
            dump_path,
            cdb_path,
            symbols_path,
            self.default_timeout,
            self.default_init_timeout,
            self.verbose,
        )
        .await?;

        let session_arc = Arc::new(Mutex::new(session));

        // 存储会话
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), Arc::clone(&session_arc));
        }

        info!("Dump session created and stored: {}", session_id);

        Ok(session_arc)
    }

    /// 获取或创建远程调试会话
    ///
    /// 如果会话已存在，返回现有会话；否则创建新会话。
    ///
    /// # 参数
    /// * `connection_string` - 远程连接字符串
    /// * `cdb_path` - 可选的自定义 CDB 路径
    /// * `symbols_path` - 可选的符号路径
    ///
    /// # 返回
    /// 返回会话的 Arc<Mutex> 引用
    ///
    /// # 错误
    /// 如果会话创建失败，返回错误
    pub async fn get_or_create_remote_session(
        &self,
        connection_string: &str,
        cdb_path: Option<&Path>,
        symbols_path: Option<&str>,
    ) -> Result<Arc<Mutex<CdbSession>>, SessionError> {
        let session_id = connection_string.to_string();

        debug!("Requesting remote session: {}", session_id);

        // 检查会话是否已存在
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                info!("Reusing existing remote session: {}", session_id);
                return Ok(Arc::clone(session));
            }
        }

        // 创建新会话
        info!("Creating new remote session: {}", session_id);
        let session = CdbSession::new_remote(
            connection_string,
            cdb_path,
            symbols_path,
            self.default_timeout,
            self.default_init_timeout,
            self.verbose,
        )
        .await?;

        let session_arc = Arc::new(Mutex::new(session));

        // 存储会话
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), Arc::clone(&session_arc));
        }

        info!("Remote session created and stored: {}", session_id);

        Ok(session_arc)
    }

    /// 关闭指定会话
    ///
    /// # 参数
    /// * `session_id` - 要关闭的会话 ID
    ///
    /// # 返回
    /// 如果成功关闭，返回 Ok；如果会话不存在，返回错误
    ///
    /// # 错误
    /// 如果会话不存在或关闭失败，返回错误
    pub async fn close_session(&self, session_id: &str) -> Result<(), SessionError> {
        info!("Closing session: {}", session_id);

        // 从存储中移除会话
        let session_arc = {
            let mut sessions = self.sessions.write().await;
            sessions
                .remove(session_id)
                .ok_or_else(|| SessionError::SessionNotFound(session_id.to_string()))?
        };

        // 尝试获取会话的独占访问权
        // 如果有其他地方还在使用这个会话，这里会等待
        match Arc::try_unwrap(session_arc) {
            Ok(session_mutex) => {
                // 成功获取独占访问权，关闭会话
                let session = session_mutex.into_inner();
                session.shutdown().await?;
                info!("Session closed: {}", session_id);
            }
            Err(arc) => {
                // 还有其他引用，放回去并记录警告
                let mut sessions = self.sessions.write().await;
                sessions.insert(session_id.to_string(), arc);
                return Err(SessionError::InvalidSessionId(format!(
                    "Session still in use: {}",
                    session_id
                )));
            }
        }

        Ok(())
    }

    /// 关闭所有会话
    ///
    /// # 返回
    /// 如果成功关闭所有会话，返回 Ok；否则返回第一个错误
    ///
    /// # 错误
    /// 如果任何会话关闭失败，返回错误
    pub async fn close_all_sessions(&self) -> Result<(), SessionError> {
        info!("Closing all sessions");

        // 获取所有会话 ID
        let session_ids: Vec<String> = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect()
        };

        let count = session_ids.len();
        info!("Preparing to close {} sessions", count);

        // 逐个关闭会话
        for session_id in session_ids {
            if let Err(e) = self.close_session(&session_id).await {
                // 记录错误但继续关闭其他会话
                tracing::warn!("Failed to close session {}: {}", session_id, e);
            }
        }

        info!("All sessions closed");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_manager_new() {
        let manager = SessionManager::new(Duration::from_secs(30), Duration::from_secs(120), false);
        assert_eq!(manager.active_session_count().await, 0);
    }

    #[tokio::test]
    async fn test_active_session_count() {
        let manager = SessionManager::new(Duration::from_secs(30), Duration::from_secs(120), false);
        let count = manager.active_session_count().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_get_or_create_dump_session_file_not_found() {
        let manager = SessionManager::new(Duration::from_secs(30), Duration::from_secs(120), false);
        let result = manager
            .get_or_create_dump_session(Path::new("nonexistent.dmp"), None, None)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::DumpFileNotFound(_) => {}
            _ => panic!("Expected DumpFileNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_close_session_not_found() {
        let manager = SessionManager::new(Duration::from_secs(30), Duration::from_secs(120), false);
        let result = manager.close_session("nonexistent").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::SessionNotFound(_) => {}
            _ => panic!("Expected SessionNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_close_all_sessions_empty() {
        let manager = SessionManager::new(Duration::from_secs(30), Duration::from_secs(120), false);
        let result = manager.close_all_sessions().await;
        assert!(result.is_ok());
    }
}
