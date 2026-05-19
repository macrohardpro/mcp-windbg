use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

use crate::http::session::SessionManager;

/// Cleanup task for expired sessions
pub struct CleanupTask {
    session_manager: Arc<SessionManager>,
    interval: Duration,
}

impl CleanupTask {
    /// Create a new cleanup task
    pub fn new(session_manager: Arc<SessionManager>, interval: Duration) -> Self {
        Self {
            session_manager,
            interval,
        }
    }
    
    /// Run cleanup task periodically
    pub async fn run(self) {
        info!(
            "Starting cleanup task (interval: {} seconds)",
            self.interval.as_secs()
        );
        
        let mut interval_timer = tokio::time::interval(self.interval);
        
        loop {
            interval_timer.tick().await;
            
            debug!("Running cleanup task");
            
            let count = self.session_manager.cleanup_expired().await;
            
            if count > 0 {
                info!("Cleaned up {} expired session(s)", count);
            } else {
                debug!("No expired sessions to clean up");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::session::SessionConfig;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_cleanup_task_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_concurrent: 5,
            ttl: Duration::from_secs(60),
            workspace_root: temp_dir.path().to_path_buf(),
        };
        
        let manager = Arc::new(SessionManager::new(config));
        let task = CleanupTask::new(manager, Duration::from_secs(3600));
        
        assert_eq!(task.interval, Duration::from_secs(3600));
    }
}
