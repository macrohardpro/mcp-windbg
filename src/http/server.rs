use axum::{
    extract::State,
    http::{header, Method},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde_json::json;
use std::sync::Arc;
use std::time::SystemTime;
use axum::extract::DefaultBodyLimit;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;

use crate::http::{
    config::ServerConfig,
    handlers::{handle_download, handle_progress, handle_report, handle_upload, AppState},
    mcp_wrapper::McpClientWrapper,
    session::{SessionConfig, SessionManager},
};

/// HTTP server
pub struct HttpServer {
    config: Arc<ServerConfig>,
    session_manager: Arc<SessionManager>,
}

impl HttpServer {
    /// Create a new HTTP server
    pub fn new(config: ServerConfig) -> Self {
        let config = Arc::new(config);
        
        // Create session manager
        let session_config = SessionConfig {
            max_concurrent: config.max_concurrent_sessions,
            ttl: config.session_ttl(),
            workspace_root: config.paths.workspace_root.clone(),
            max_stored_sessions: config.max_stored_sessions,
        };
        
        let session_manager = Arc::new(SessionManager::new(session_config));
        
        Self {
            config,
            session_manager,
        }
    }
    
    /// Get session manager (for cleanup task)
    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }
    
    /// Run the HTTP server
    pub async fn run(self) -> anyhow::Result<()> {
        let addr = format!("0.0.0.0:{}", self.config.port);
        
        info!("Starting HTTP server on {}", addr);
        
        // Create MCP wrapper (resolve relative paths)
        let mcp_server_path = if self.config.paths.mcp_server.is_absolute() {
            self.config.paths.mcp_server.clone()
        } else {
            std::env::current_dir()?.join(&self.config.paths.mcp_server)
        };
        let mcp_wrapper = Arc::new(McpClientWrapper::new(
            self.config.paths.python.clone(),
            std::env::current_dir()?.join("action").join("mcp_client.py"),
            mcp_server_path,
        ));
        
        // Create app state
        let state = AppState {
            session_manager: self.session_manager.clone(),
            config: self.config.clone(),
            mcp_wrapper,
        };
        
        // Build router
        let app = Router::new()
            .route("/", get(handle_index))
            .route("/upload", post(handle_upload))
            .route("/progress/:session_id", get(handle_progress))
            .route("/report/:session_id", get(handle_report))
            .route("/download/:session_id", get(handle_download))
            .route("/health", get(handle_health))
            .layer(DefaultBodyLimit::max(self.config.max_upload_size))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods([Method::GET, Method::POST])
                    .allow_headers([header::CONTENT_TYPE]),
            )
            .layer(TraceLayer::new_for_http())
            .with_state(state);
        
        // Start server
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        
        info!("HTTP server listening on {}", addr);
        
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

/// Handle index page (upload form)
async fn handle_index(State(state): State<AppState>) -> impl IntoResponse {
    let html = INDEX_HTML.replace("__MAX_UPLOAD_SIZE__", &format_max_upload_size(state.config.max_upload_size));
    Html(html)
}

fn format_max_upload_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Handle health check
async fn handle_health(State(state): State<AppState>) -> impl IntoResponse {
    let active_sessions = state.session_manager.active_count().await;
    
    let uptime = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    Json(json!({
        "status": "ok",
        "active_sessions": active_sessions,
        "uptime_seconds": uptime,
    }))
}

/// Index HTML page with upload form
const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>转储文件分析平台</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 20px;
        }
        
        .container {
            background: white;
            border-radius: 12px;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
            max-width: 600px;
            width: 100%;
            padding: 40px;
        }
        
        h1 {
            color: #333;
            margin-bottom: 10px;
            font-size: 2em;
        }
        
        .subtitle {
            color: #666;
            margin-bottom: 30px;
            font-size: 1.1em;
        }
        
        .upload-area {
            border: 2px dashed #667eea;
            border-radius: 8px;
            padding: 40px;
            text-align: center;
            background: #f8f9ff;
            cursor: pointer;
            transition: all 0.3s ease;
            margin-bottom: 20px;
        }
        
        .upload-area:hover {
            border-color: #764ba2;
            background: #f0f2ff;
        }
        
        .upload-area.dragover {
            border-color: #764ba2;
            background: #e8ebff;
        }
        
        .upload-icon {
            font-size: 48px;
            margin-bottom: 10px;
        }
        
        input[type="file"] {
            display: none;
        }
        
        .file-info {
            margin: 20px 0;
            padding: 15px;
            background: #f0f2ff;
            border-radius: 6px;
            display: none;
        }
        
        .file-info.show {
            display: block;
        }
        
        .file-name {
            font-weight: 600;
            color: #333;
            margin-bottom: 5px;
        }
        
        .file-size {
            color: #666;
            font-size: 0.9em;
        }
        
        button {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            padding: 15px 40px;
            border-radius: 6px;
            font-size: 1.1em;
            font-weight: 600;
            cursor: pointer;
            width: 100%;
            transition: transform 0.2s ease, box-shadow 0.2s ease;
        }
        
        button:hover:not(:disabled) {
            transform: translateY(-2px);
            box-shadow: 0 10px 20px rgba(102, 126, 234, 0.3);
        }
        
        button:disabled {
            opacity: 0.6;
            cursor: not-allowed;
        }
        
        .info-box {
            background: #fff9e6;
            border-left: 4px solid #ffc107;
            padding: 15px;
            margin-top: 20px;
            border-radius: 4px;
        }
        
        .info-box h3 {
            color: #f57c00;
            margin-bottom: 10px;
            font-size: 1em;
        }
        
        .info-box ul {
            list-style: none;
            color: #666;
            font-size: 0.9em;
        }
        
        .info-box li {
            margin: 5px 0;
            padding-left: 20px;
            position: relative;
        }
        
        .info-box li:before {
            content: "✓";
            position: absolute;
            left: 0;
            color: #4caf50;
            font-weight: bold;
        }
        
        .progress {
            display: none;
            margin-top: 20px;
        }
        
        .progress.show {
            display: block;
        }
        
        .progress-bar {
            width: 100%;
            height: 8px;
            background: #e0e0e0;
            border-radius: 4px;
            overflow: hidden;
            margin-bottom: 10px;
        }
        
        .progress-bar-fill {
            height: 100%;
            background: linear-gradient(90deg, #667eea 0%, #764ba2 100%);
            width: 0%;
            transition: width 0.3s ease;
            animation: progress-animation 1.5s ease-in-out infinite;
        }
        
        @keyframes progress-animation {
            0% { opacity: 1; }
            50% { opacity: 0.6; }
            100% { opacity: 1; }
        }
        
        .progress-text {
            text-align: center;
            color: #666;
            font-size: 0.9em;
        }
        
        .log-container {
            max-height: 200px;
            overflow-y: auto;
            background: #f5f5f5;
            border-radius: 4px;
            padding: 10px;
            margin-top: 10px;
            font-family: monospace;
            font-size: 0.85em;
            display: none;
        }
        
        .log-container.show {
            display: block;
        }
        
        .log-entry {
            margin: 2px 0;
            color: #333;
        }
        
        .result {
            display: none;
            margin-top: 20px;
            padding: 20px;
            border-radius: 8px;
            text-align: center;
        }

        .result.show {
            display: block;
        }

        .result.success {
            background: #e8f5e9;
            border: 2px solid #4caf50;
        }
        
        .result.error {
            background: #ffebee;
            border: 2px solid #f44336;
        }
        
        .result h3 {
            margin-bottom: 15px;
        }
        
        .result.success h3 {
            color: #2e7d32;
        }
        
        .result.error h3 {
            color: #c62828;
        }
        
        .result a {
            display: inline-block;
            margin: 10px;
            padding: 10px 20px;
            background: #667eea;
            color: white;
            text-decoration: none;
            border-radius: 4px;
            transition: background 0.3s ease;
        }
        
        .result a:hover {
            background: #764ba2;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>转储文件分析平台</h1>
        <p class="subtitle">上传崩溃转储压缩包，AI 自动分析并生成报告</p>
        
        <div class="upload-area" id="uploadArea">
            <div class="upload-icon">📦</div>
            <p><strong>点击选择</strong>或将压缩包拖放到此处</p>
            <p style="color: #999; font-size: 0.9em; margin-top: 10px;">最大文件大小：__MAX_UPLOAD_SIZE__</p>
        </div>
        
        <input type="file" id="fileInput" accept=".zip,.7z,.tar.gz,.tgz">
        
        <div class="file-info" id="fileInfo">
            <div class="file-name" id="fileName"></div>
            <div class="file-size" id="fileSize"></div>
        </div>
        
        <button id="uploadBtn" disabled>上传并分析</button>
        
        <div class="progress" id="progress">
            <div class="progress-bar">
                <div class="progress-bar-fill" id="progressFill"></div>
            </div>
            <div class="progress-text" id="progressText">Uploading...</div>
            <div class="log-container" id="logContainer"></div>
        </div>
        
        <div class="result" id="result">
            <h3 id="resultTitle"></h3>
            <p id="resultMessage"></p>
            <div id="resultLinks"></div>
        </div>
        
        <div class="info-box">
            <h3>压缩包要求</h3>
            <ul>
                <li>支持格式: ZIP, 7z, tar.gz</li>
                <li>必须包含至少一个 .dmp 文件</li>
                <li>符号文件 (.pdb) 可选，但建议包含</li>
                <li>源代码文件可选</li>
            </ul>
            <h3 style="margin-top: 16px;">压缩包结构示例</h3>
            <pre style="background:#2d2d2d;color:#f8f8f2;padding:16px;border-radius:6px;font-size:0.85em;line-height:1.5;overflow-x:auto;">your_crash.zip
├── crash.dmp          ← 必需
├── symbols/
│   └── *.pdb          ← 可选（建议）
└── src/
    └── *.cpp / *.h / *.rs  ← 可选</pre>
        </div>
    </div>
    
    <script>
        const uploadArea = document.getElementById('uploadArea');
        const fileInput = document.getElementById('fileInput');
        const fileInfo = document.getElementById('fileInfo');
        const fileName = document.getElementById('fileName');
        const fileSize = document.getElementById('fileSize');
        const uploadBtn = document.getElementById('uploadBtn');
        const progress = document.getElementById('progress');
        const progressFill = document.getElementById('progressFill');
        const progressText = document.getElementById('progressText');
        const logContainer = document.getElementById('logContainer');
        const result = document.getElementById('result');
        const resultTitle = document.getElementById('resultTitle');
        const resultMessage = document.getElementById('resultMessage');
        const resultLinks = document.getElementById('resultLinks');
        
        let selectedFile = null;
        
        // Click to select file
        uploadArea.addEventListener('click', () => fileInput.click());
        
        // Drag and drop
        uploadArea.addEventListener('dragover', (e) => {
            e.preventDefault();
            uploadArea.classList.add('dragover');
        });
        
        uploadArea.addEventListener('dragleave', () => {
            uploadArea.classList.remove('dragover');
        });
        
        uploadArea.addEventListener('drop', (e) => {
            e.preventDefault();
            uploadArea.classList.remove('dragover');
            
            if (e.dataTransfer.files.length > 0) {
                handleFileSelect(e.dataTransfer.files[0]);
            }
        });
        
        // File input change
        fileInput.addEventListener('change', (e) => {
            if (e.target.files.length > 0) {
                handleFileSelect(e.target.files[0]);
            }
        });
        
        // Handle file selection
        function handleFileSelect(file) {
            selectedFile = file;
            
            fileName.textContent = file.name;
            fileSize.textContent = formatFileSize(file.size);
            
            fileInfo.classList.add('show');
            uploadBtn.disabled = false;
        }
        
        // Format file size
        function formatFileSize(bytes) {
            if (bytes < 1024) return bytes + ' B';
            if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + ' KB';
            if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
            return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
        }
        
        // Upload button click
        uploadBtn.addEventListener('click', async () => {
            if (!selectedFile) return;
            
            uploadBtn.disabled = true;
            progress.classList.add('show');
            result.classList.remove('show');
            
            try {
                // Upload file
                const formData = new FormData();
                formData.append('file', selectedFile);
                
                progressText.textContent = '上传中...';
                progressFill.style.width = '10%';

                const response = await fetch('/upload', {
                    method: 'POST',
                    body: formData
                });

                if (!response.ok) {
                    const error = await response.json();
                    throw new Error(error.error || '上传失败');
                }

                const data = await response.json();

                progressText.textContent = '处理中...';
                progressFill.style.width = '20%';
                logContainer.classList.add('show');

                // Connect to progress stream
                const eventSource = new EventSource(data.progress_url);

                eventSource.onmessage = (event) => {
                    const progressEvent = JSON.parse(event.data);

                    if (progressEvent.type === 'status') {
                        const currentText = progressText.textContent;
                        // Don't overwrite progress percentage with plain status
                        if (!currentText.includes('进度')) {
                            progressText.textContent = '状态：' + progressEvent.status;
                        }
                    } else if (progressEvent.type === 'progress') {
                        progressFill.style.width = progressEvent.percent + '%';
                        progressText.textContent = '分析进度：' + progressEvent.percent + '%';
                    } else if (progressEvent.type === 'log') {
                        const logEntry = document.createElement('div');
                        logEntry.className = 'log-entry';
                        logEntry.textContent = progressEvent.message;
                        logContainer.appendChild(logEntry);
                        logContainer.scrollTop = logContainer.scrollHeight;
                    } else if (progressEvent.type === 'complete') {
                        eventSource.close();
                        showSuccess(data.session_id, progressEvent.report_url);
                    } else if (progressEvent.type === 'error') {
                        eventSource.close();
                        showError(progressEvent.message);
                    }
                };
                
                eventSource.onerror = () => {
                    eventSource.close();
                    showError('与服务器连接断开');
                };
                
            } catch (error) {
                showError(error.message);
            }
        });
        
        // Show success result
        function showSuccess(sessionId, reportUrl) {
            progress.classList.remove('show');
            result.classList.add('show', 'success');
            result.classList.remove('error');

            resultTitle.textContent = '分析完成';
            resultMessage.textContent = '崩溃转储已成功分析。';

            resultLinks.innerHTML = `
                <a href="${reportUrl}" target="_blank">查看报告</a>
                <a href="/download/${sessionId}" download>下载 Markdown</a>
            `;

            uploadBtn.disabled = false;
        }

        // Show error result
        function showError(message) {
            progress.classList.remove('show');
            result.classList.add('show', 'error');
            result.classList.remove('success');

            resultTitle.textContent = '分析失败';
            resultMessage.textContent = message;
            resultLinks.innerHTML = '';

            uploadBtn.disabled = false;
        }
    </script>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_server_creation() {
        let config = ServerConfig {
            port: 8080,
            max_upload_size: 500 * 1024 * 1024,
            max_concurrent_sessions: 5,
            cleanup_interval_secs: 3600,
            session_ttl_secs: 86400,
            max_stored_sessions: 20,
            cdb_command_timeout_secs: 120,
            cdb_init_timeout_secs: 120,
            paths: crate::http::config::PathConfig::default(),
            llm: crate::http::config::LlmConfig {
                api_key: "test-key".to_string(),
                api_base: "https://api.example.com".to_string(),
                model: "test-model".to_string(),
                max_turns: 30,
                timeout_secs: 600,
            },
            rate_limit: crate::http::config::RateLimitConfig::default(),
        };
        
        let server = HttpServer::new(config);
        assert_eq!(server.config.port, 8080);
    }
}
