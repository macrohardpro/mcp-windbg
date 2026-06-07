use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tracing::{debug, info};

use crate::http::error::{HttpError, HttpResult};

/// MCP Client wrapper
pub struct McpClientWrapper {
    python_path: PathBuf,
    mcp_client_path: PathBuf,
    mcp_server_path: PathBuf,
}

/// Analysis request
pub struct AnalysisRequest {
    pub dump_path: PathBuf,
    pub symbols_path: Option<PathBuf>,
    pub source_path: Option<PathBuf>,
    pub workspace: PathBuf,
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub max_turns: u32,
    pub timeout_secs: u64,
    pub cdb_path: PathBuf,
    pub cdb_command_timeout_secs: u64,
    pub cdb_init_timeout_secs: u64,
}

impl McpClientWrapper {
    /// Create a new MCP client wrapper
    pub fn new(
        python_path: PathBuf,
        mcp_client_path: PathBuf,
        mcp_server_path: PathBuf,
    ) -> Self {
        Self {
            python_path,
            mcp_client_path,
            mcp_server_path,
        }
    }
    
    /// Run analysis by spawning mcp_client.py process
    pub async fn run_analysis(&self, request: AnalysisRequest) -> HttpResult<Child> {
        info!(
            "Starting analysis: dump={:?}, symbols={:?}, source={:?}",
            request.dump_path, request.symbols_path, request.source_path
        );
        
        // Build command
        let mut cmd = Command::new(&self.python_path);
        cmd.arg(&self.mcp_client_path);
        
        // Add optional arguments
        if let Some(symbols_path) = &request.symbols_path {
            cmd.arg("--symbols-path").arg(symbols_path);
        }
        
        if let Some(source_path) = &request.source_path {
            cmd.arg("--source-path").arg(source_path);
        }
        
        // Set environment variables
        cmd.env("API_KEY", &request.api_key);
        cmd.env("API_BASE", &request.api_base);
        cmd.env("MODEL", &request.model);
        cmd.env("MAX_TURNS", request.max_turns.to_string());
        cmd.env("TIMEOUT", request.timeout_secs.to_string());
        cmd.env("MCP_SERVER_PATH", &self.mcp_server_path);
        cmd.env("CDB_PATH", &request.cdb_path);
        cmd.env("MCP_WINDBG_TIMEOUT", request.cdb_command_timeout_secs.to_string());
        cmd.env("MCP_WINDBG_INIT_TIMEOUT", request.cdb_init_timeout_secs.to_string());
        // DOWNLOAD_DIR should point to where the .dmp files actually are
        let download_dir = request.dump_path.parent().unwrap_or(&request.workspace);
        cmd.env("DOWNLOAD_DIR", download_dir);
        cmd.env("GITHUB_WORKSPACE", &request.workspace);
        
        // Configure stdio
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null());
        
        // Set working directory
        cmd.current_dir(&request.workspace);
        
        debug!("Spawning mcp_client.py: {:?}", cmd);
        
        // Spawn process
        let child = cmd
            .spawn()
            .map_err(|e| HttpError::AnalysisFailed(format!("Failed to spawn mcp_client.py: {}", e)))?;
        
        info!("Analysis process started (pid={})", child.id().unwrap_or(0));
        
        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_wrapper_creation() {
        let wrapper = McpClientWrapper::new(
            PathBuf::from("python"),
            PathBuf::from("mcp_client.py"),
            PathBuf::from("mcp-windbg-rs.exe"),
        );
        
        assert_eq!(wrapper.python_path, PathBuf::from("python"));
        assert_eq!(wrapper.mcp_client_path, PathBuf::from("mcp_client.py"));
        assert_eq!(wrapper.mcp_server_path, PathBuf::from("mcp-windbg-rs.exe"));
    }
    
    #[tokio::test]
    async fn test_analysis_request_building() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().to_path_buf();
        
        let request = AnalysisRequest {
            dump_path: workspace.join("test.dmp"),
            symbols_path: Some(workspace.join("symbols")),
            source_path: Some(workspace.join("src")),
            workspace: workspace.clone(),
            api_key: "test-key".to_string(),
            api_base: "https://api.example.com".to_string(),
            model: "test-model".to_string(),
            max_turns: 30,
            timeout_secs: 300,
            cdb_path: PathBuf::from("cdb.exe"),
            cdb_command_timeout_secs: 60,
            cdb_init_timeout_secs: 120,
        };

        assert_eq!(request.api_key, "test-key");
        assert_eq!(request.max_turns, 30);
        assert_eq!(request.cdb_command_timeout_secs, 60);
        assert_eq!(request.cdb_init_timeout_secs, 120);
        assert!(request.symbols_path.is_some());
        assert!(request.source_path.is_some());
    }
}
