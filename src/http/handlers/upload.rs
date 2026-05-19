use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info};

use crate::http::{
    archive::{scan_extracted_files, Extractor},
    config::ServerConfig,
    error::{HttpError, HttpResult},
    mcp_wrapper::{AnalysisRequest, McpClientWrapper},
    session::{SessionManager, SessionStatus},
};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub config: Arc<ServerConfig>,
    pub mcp_wrapper: Arc<McpClientWrapper>,
}

/// Upload response
#[derive(Debug, Serialize, Deserialize)]
pub struct UploadResponse {
    pub session_id: String,
    pub progress_url: String,
    pub report_url: String,
}

/// Handle file upload
pub async fn handle_upload(
    State(state): State<AppState>,
    multipart: Multipart,
) -> HttpResult<impl IntoResponse> {
    info!("Received upload request");
    
    // Process multipart upload
    let (session_id, _workspace) = process_upload(state.clone(), multipart).await?;
    
    // Return response with URLs
    let response = UploadResponse {
        session_id: session_id.clone(),
        progress_url: format!("/progress/{}", session_id),
        report_url: format!("/report/{}", session_id),
    };
    
    Ok((StatusCode::OK, Json(response)))
}

/// Process multipart upload — save file first, then spawn extract+analyze
async fn process_upload(
    state: AppState,
    mut multipart: Multipart,
) -> HttpResult<(String, PathBuf)> {
    let client_ip: Option<IpAddr> = None; // TODO: Extract from ConnectInfo

    // Create session
    let session_id = state.session_manager.create_session(client_ip).await?;
    let workspace = state
        .session_manager
        .get_workspace(&session_id)
        .await
        .ok_or_else(|| HttpError::SessionNotFound(session_id.clone()))?;

    info!("Created session {} with workspace {:?}", session_id, workspace);

    // Save uploaded file BEFORE returning response — multipart is tied to HTTP connection
    state
        .session_manager
        .update_status(&session_id, SessionStatus::Uploading)
        .await;

    let archive_path = match save_uploaded_file(&mut multipart, &workspace, state.config.max_upload_size).await {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to save uploaded file: {:?}", e);
            return Err(e);
        }
    };

    info!("Saved uploaded file to {:?} ({} bytes)", archive_path,
          archive_path.metadata().map(|m| m.len()).unwrap_or(0));

    // Spawn extraction + analysis as background task
    let session_id_clone = session_id.clone();
    let state_clone = state.clone();
    let workspace_for_bg = workspace.clone();

    tokio::spawn(async move {
        if let Err(e) = process_extract_and_analyze(
            state_clone.clone(),
            session_id_clone.clone(),
            workspace_for_bg,
            archive_path,
        )
        .await
        {
            error!("Upload processing failed for session {}: {}", session_id_clone, e);
            state_clone
                .session_manager
                .update_status(&session_id_clone, SessionStatus::Failed(e.to_string()))
                .await;
        }
    });

    Ok((session_id, workspace))
}

/// Extract archive and run AI analysis (runs in background task)
async fn process_extract_and_analyze(
    state: AppState,
    session_id: String,
    workspace: PathBuf,
    archive_path: PathBuf,
) -> HttpResult<()> {
    // Update status to extracting
    state
        .session_manager
        .update_status(&session_id, SessionStatus::Extracting)
        .await;

    // Extract archive
    let extract_dir = workspace.join("extracted");
    tokio::fs::create_dir_all(&extract_dir).await?;

    Extractor::extract(&archive_path, &extract_dir).await?;

    info!("Extracted archive to {:?}", extract_dir);

    // Scan extracted files
    let extracted_files = scan_extracted_files(&extract_dir).await?;

    // Validate at least one dump file exists
    if extracted_files.dump_files.is_empty() {
        return Err(HttpError::NoDumpFile);
    }

    info!(
        "Found {} dump file(s), {} symbol dir(s), {} source dir(s)",
        extracted_files.dump_files.len(),
        extracted_files.symbol_dirs.len(),
        extracted_files.source_dirs.len()
    );

    // Update status to analyzing
    state
        .session_manager
        .update_status(&session_id, SessionStatus::Analyzing)
        .await;

    // Prepare analysis request
    let dump_path = extracted_files.dump_files[0].clone();
    let symbols_path = extracted_files.symbol_dirs.first().cloned();
    let source_path = extracted_files.source_dirs.first().cloned();

    let request = AnalysisRequest {
        dump_path,
        symbols_path,
        source_path,
        workspace: workspace.clone(),
        api_key: state.config.llm.api_key.clone(),
        api_base: state.config.llm.api_base.clone(),
        model: state.config.llm.model.clone(),
        max_turns: state.config.llm.max_turns,
        timeout_secs: state.config.llm.timeout_secs,
        cdb_path: state.config.paths.cdb.clone(),
    };

    // Start MCP client process
    let child = state.mcp_wrapper.run_analysis(request).await?;

    info!("Started analysis process for session {}", session_id);

    // Store process in session
    state.session_manager.set_process(&session_id, child).await;

    Ok(())
}

/// Save uploaded file from multipart form
async fn save_uploaded_file(
    multipart: &mut Multipart,
    workspace: &PathBuf,
    max_size: usize,
) -> HttpResult<PathBuf> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| HttpError::Internal(format!("Failed to read multipart field: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        
        if name != "file" {
            continue;
        }
        
        // Get filename
        let filename = field
            .file_name()
            .ok_or_else(|| HttpError::InvalidFileName("No filename provided".to_string()))?
            .to_string();
        
        debug!("Receiving file: {}", filename);
        
        // Validate filename
        validate_filename(&filename)?;
        
        // Validate file extension
        validate_extension(&filename)?;
        
        // Save file
        let file_path = workspace.join(&filename);
        let mut file = tokio::fs::File::create(&file_path).await?;
        
        let mut total_size = 0;
        let mut stream = field;
        
        while let Some(chunk) = stream
            .chunk()
            .await
            .map_err(|e| HttpError::Internal(format!("Failed to read chunk: {}", e)))?
        {
            total_size += chunk.len();
            if total_size % (10 * 1024 * 1024) < chunk.len() || total_size < 1024 * 1024 {
                debug!("Receiving chunk: {} bytes, total: {} bytes", chunk.len(), total_size);
            }
            
            // Check size limit
            if total_size > max_size {
                // Clean up partial file
                let _ = tokio::fs::remove_file(&file_path).await;
                return Err(HttpError::UploadTooLarge(total_size, max_size));
            }
            
            file.write_all(&chunk).await?;
        }
        
        file.flush().await?;
        
        info!("Saved file {} ({} bytes)", filename, total_size);
        
        return Ok(file_path);
    }
    
    Err(HttpError::InvalidFileName("No file field found".to_string()))
}

/// Validate filename for security
fn validate_filename(filename: &str) -> HttpResult<()> {
    // Check for path traversal attempts
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(HttpError::InvalidFileName(format!(
            "Invalid filename: {}",
            filename
        )));
    }
    
    // Check for empty filename
    if filename.is_empty() {
        return Err(HttpError::InvalidFileName("Empty filename".to_string()));
    }
    
    Ok(())
}

/// Validate file extension
fn validate_extension(filename: &str) -> HttpResult<()> {
    let filename_lower = filename.to_lowercase();
    
    if filename_lower.ends_with(".zip")
        || filename_lower.ends_with(".7z")
        || filename_lower.ends_with(".tar.gz")
        || filename_lower.ends_with(".tgz")
    {
        Ok(())
    } else {
        Err(HttpError::UnsupportedFormat(format!(
            "Unsupported file extension. Supported formats: .zip, .7z, .tar.gz, .tgz"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_filename() {
        // Valid filenames
        assert!(validate_filename("test.zip").is_ok());
        assert!(validate_filename("my-dump.7z").is_ok());
        assert!(validate_filename("archive_123.tar.gz").is_ok());
        
        // Invalid filenames
        assert!(validate_filename("../etc/passwd").is_err());
        assert!(validate_filename("dir/file.zip").is_err());
        assert!(validate_filename("C:\\Windows\\file.zip").is_err());
        assert!(validate_filename("").is_err());
    }
    
    #[test]
    fn test_validate_extension() {
        // Valid extensions
        assert!(validate_extension("test.zip").is_ok());
        assert!(validate_extension("test.7z").is_ok());
        assert!(validate_extension("test.tar.gz").is_ok());
        assert!(validate_extension("test.tgz").is_ok());
        assert!(validate_extension("TEST.ZIP").is_ok()); // Case insensitive
        
        // Invalid extensions
        assert!(validate_extension("test.txt").is_err());
        assert!(validate_extension("test.rar").is_err());
        assert!(validate_extension("test").is_err());
    }
}
