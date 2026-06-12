use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::process::Child;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::http::error::{HttpError, HttpResult};

/// Session ID type (UUID v4 string)
pub type SessionId = String;

/// Session status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Uploading,
    Extracting,
    Analyzing,
    Complete,
    Failed(String),
}

/// Debug session
#[derive(Debug)]
pub struct Session {
    pub id: SessionId,
    pub workspace: PathBuf,
    pub status: SessionStatus,
    pub created_at: SystemTime,
    pub process: Option<Child>,
    pub client_ip: Option<IpAddr>,
}

/// Session manager configuration
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub max_concurrent: usize,
    pub ttl: Duration,
    pub workspace_root: PathBuf,
    pub max_stored_sessions: usize,
}

/// Session manager
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    config: SessionConfig,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(config: SessionConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
    
    /// Create a new session
    pub async fn create_session(&self, client_ip: Option<IpAddr>) -> HttpResult<SessionId> {
        // Clean up oldest sessions if at capacity
        self.cleanup_overflow().await;

        let mut sessions = self.sessions.write().await;

        // Check concurrent session limit (only count active sessions)
        let active_count = sessions
            .values()
            .filter(|s| {
                matches!(
                    s.status,
                    SessionStatus::Uploading | SessionStatus::Extracting | SessionStatus::Analyzing
                )
            })
            .count();
        if active_count >= self.config.max_concurrent {
            return Err(HttpError::TooManySessions(self.config.max_concurrent));
        }
        
        // Generate unique session ID
        let session_id = Uuid::new_v4().to_string();
        
        // Create workspace directory
        let workspace = self.config.workspace_root.join("sessions").join(&session_id);
        tokio::fs::create_dir_all(&workspace).await?;
        
        info!("Created session {} (ip={:?})", session_id, client_ip);
        
        // Create session
        let session = Session {
            id: session_id.clone(),
            workspace,
            status: SessionStatus::Uploading,
            created_at: SystemTime::now(),
            process: None,
            client_ip,
        };
        
        sessions.insert(session_id.clone(), session);
        
        Ok(session_id)
    }
    
    /// Get a session by ID
    pub async fn get_session(&self, id: &SessionId) -> Option<SessionStatus> {
        let sessions = self.sessions.read().await;
        sessions.get(id).map(|s| s.status.clone())
    }
    
    /// Get session workspace path
    pub async fn get_workspace(&self, id: &SessionId) -> Option<PathBuf> {
        let sessions = self.sessions.read().await;
        sessions.get(id).map(|s| s.workspace.clone())
    }
    
    /// Update session status
    pub async fn update_status(&self, id: &SessionId, status: SessionStatus) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(id) {
            debug!("Session {} status: {:?} -> {:?}", id, session.status, status);
            session.status = status;
        }
    }
    
    /// Set session process
    pub async fn set_process(&self, id: &SessionId, process: Child) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(id) {
            session.process = Some(process);
        }
    }
    
    /// Get session process
    pub async fn take_process(&self, id: &SessionId) -> Option<Child> {
        let mut sessions = self.sessions.write().await;
        sessions.get_mut(id).and_then(|s| s.process.take())
    }
    
    /// Get active session count
    pub async fn active_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }
    
    /// Clean up expired sessions
    pub async fn cleanup_expired(&self) -> usize {
        let now = SystemTime::now();
        let mut sessions = self.sessions.write().await;
        
        let mut expired = Vec::new();
        
        for (id, session) in sessions.iter() {
            if let Ok(elapsed) = now.duration_since(session.created_at) {
                if elapsed > self.config.ttl {
                    expired.push(id.clone());
                }
            }
        }
        
        let count = expired.len();
        
        for id in expired {
            if let Some(session) = sessions.remove(&id) {
                info!("Cleaning up expired session {}", id);
                
                // Kill process if still running
                if let Some(mut process) = session.process {
                    let _ = process.kill().await;
                }
                
                // Remove workspace directory (with retries)
                let workspace = session.workspace.clone();
                tokio::spawn(async move {
                    for attempt in 1..=3 {
                        match tokio::fs::remove_dir_all(&workspace).await {
                            Ok(_) => {
                                debug!("Removed workspace: {:?}", workspace);
                                break;
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to remove workspace {:?} (attempt {}): {}",
                                    workspace, attempt, e
                                );
                                if attempt < 3 {
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                }
                            }
                        }
                    }
                });
            }
        }
        
        count
    }

    /// Remove oldest sessions until count is below max_stored_sessions
    pub async fn cleanup_overflow(&self) {
        let mut sessions = self.sessions.write().await;
        let max = self.config.max_stored_sessions;

        while sessions.len() >= max {
            // Find oldest session by created_at
            let oldest_id = sessions
                .iter()
                .min_by_key(|(_, s)| s.created_at)
                .map(|(id, _)| id.clone());

            if let Some(id) = oldest_id {
                if let Some(session) = sessions.remove(&id) {
                    info!(
                        "Cleaning up overflow session {} (created {:?})",
                        id, session.created_at
                    );

                    if let Some(mut process) = session.process {
                        let _ = process.kill().await;
                    }

                    let workspace = session.workspace.clone();
                    tokio::spawn(async move {
                        for attempt in 1..=3 {
                            match tokio::fs::remove_dir_all(&workspace).await {
                                Ok(_) => {
                                    debug!("Removed overflow workspace: {:?}", workspace);
                                    break;
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to remove overflow workspace {:?} (attempt {}): {}",
                                        workspace, attempt, e
                                    );
                                    if attempt < 3 {
                                        tokio::time::sleep(Duration::from_secs(1)).await;
                                    }
                                }
                            }
                        }
                    });
                }
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn test_config() -> (SessionConfig, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_concurrent: 2,
            ttl: Duration::from_secs(60),
            workspace_root: temp_dir.path().to_path_buf(),
            max_stored_sessions: 20,
        };
        (config, temp_dir)
    }
    
    #[tokio::test]
    async fn test_create_session() {
        let (config, _temp) = test_config();
        let manager = SessionManager::new(config);
        
        let session_id = manager.create_session(None).await.unwrap();
        assert!(!session_id.is_empty());
        
        let status = manager.get_session(&session_id).await;
        assert_eq!(status, Some(SessionStatus::Uploading));
    }
    
    #[tokio::test]
    async fn test_concurrent_limit() {
        let (config, _temp) = test_config();
        let manager = SessionManager::new(config);
        
        let _session1 = manager.create_session(None).await.unwrap();
        let _session2 = manager.create_session(None).await.unwrap();
        
        // Third session should fail
        let result = manager.create_session(None).await;
        assert!(matches!(result, Err(HttpError::TooManySessions(2))));
    }
    
    #[tokio::test]
    async fn test_update_status() {
        let (config, _temp) = test_config();
        let manager = SessionManager::new(config);
        
        let session_id = manager.create_session(None).await.unwrap();
        
        manager.update_status(&session_id, SessionStatus::Analyzing).await;
        let status = manager.get_session(&session_id).await;
        assert_eq!(status, Some(SessionStatus::Analyzing));
        
        manager.update_status(&session_id, SessionStatus::Complete).await;
        let status = manager.get_session(&session_id).await;
        assert_eq!(status, Some(SessionStatus::Complete));
    }
    
    #[tokio::test]
    async fn test_workspace_creation() {
        let (config, _temp) = test_config();
        let manager = SessionManager::new(config);

        let session_id = manager.create_session(None).await.unwrap();
        let workspace = manager.get_workspace(&session_id).await.unwrap();

        assert!(workspace.exists());
        assert!(workspace.is_dir());
    }

    #[tokio::test]
    async fn test_overflow_cleanup() {
        let (mut config, _temp) = test_config();
        config.max_stored_sessions = 3;
        config.max_concurrent = 100; // don't limit concurrent
        let manager = SessionManager::new(config);

        // Create 5 sessions
        for _ in 0..5 {
            manager.create_session(None).await.unwrap();
        }

        // Should have cleaned up to stay at or below 3
        let count = manager.active_count().await;
        assert!(count <= 3, "Expected <= 3 sessions, got {}", count);
    }
}
