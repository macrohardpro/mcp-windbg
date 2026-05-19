use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// HTTP server errors
#[derive(Debug, Error)]
pub enum HttpError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    
    #[error("Upload too large: {0} bytes (max: {1} bytes)")]
    UploadTooLarge(usize, usize),
    
    #[error("Unsupported archive format: {0}")]
    UnsupportedFormat(String),
    
    #[error("No dump file found in archive")]
    NoDumpFile,
    
    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),
    
    #[error("Too many concurrent sessions (max: {0})")]
    TooManySessions(usize),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),
    
    #[error("Invalid file name: {0}")]
    InvalidFileName(String),
    
    #[error("Path traversal attempt detected")]
    PathTraversal,
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Configuration error: {0}")]
    Config(#[from] crate::http::config::ConfigError),
    
    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            HttpError::SessionNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            HttpError::UploadTooLarge(_, _) => (StatusCode::PAYLOAD_TOO_LARGE, self.to_string()),
            HttpError::UnsupportedFormat(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            HttpError::NoDumpFile => (StatusCode::BAD_REQUEST, self.to_string()),
            HttpError::ExtractionFailed(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            HttpError::TooManySessions(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            HttpError::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            HttpError::InvalidFileName(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            HttpError::PathTraversal => (StatusCode::BAD_REQUEST, self.to_string()),
            HttpError::AnalysisFailed(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            HttpError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            HttpError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            HttpError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        
        let body = Json(json!({
            "error": error_message,
        }));
        
        (status, body).into_response()
    }
}

/// Result type for HTTP operations
pub type HttpResult<T> = Result<T, HttpError>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_status_codes() {
        let err = HttpError::SessionNotFound("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        
        let err = HttpError::UploadTooLarge(1000, 500);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        
        let err = HttpError::RateLimitExceeded;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        
        let err = HttpError::TooManySessions(5);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        
        let err = HttpError::UnsupportedFormat("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
