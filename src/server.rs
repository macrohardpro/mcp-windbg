//! MCP 服务器模块
//!
//! 实现 MCP 协议服务器，处理工具调用和消息路由。

use crate::error::ServerError;
use crate::session::SessionManager;
use crate::tools;
use crate::types::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// 服务器配置
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// 自定义 CDB 路径
    pub cdb_path: Option<PathBuf>,
    /// 符号路径
    pub symbols_path: Option<String>,
    /// 源文件路径
    pub source_path: Option<String>,
    /// 命令执行超时时间
    pub timeout: Duration,
    /// 初始化超时时间
    pub init_timeout: Duration,
    /// 是否启用详细日志
    pub verbose: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            cdb_path: None,
            symbols_path: None,
            source_path: None,
            timeout: Duration::from_secs(120),
            init_timeout: Duration::from_secs(120),
            verbose: false,
        }
    }
}

impl ServerConfig {
    /// 从环境变量读取配置
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // 读取 CDB 路径
        if let Ok(path) = std::env::var("CDB_PATH") {
            config.cdb_path = Some(PathBuf::from(path));
        }

        // 读取符号路径
        if let Ok(path) = std::env::var("_NT_SYMBOL_PATH") {
            config.symbols_path = Some(path);
        }

        // 读取源文件路径
        if let Ok(path) = std::env::var("_NT_SOURCE_PATH") {
            config.source_path = Some(path);
        }

        // 读取命令超时时间
        if let Ok(timeout_str) = std::env::var("MCP_WINDBG_TIMEOUT") {
            if let Ok(timeout_secs) = timeout_str.parse::<u64>() {
                config.timeout = Duration::from_secs(timeout_secs);
            }
        }

        // 读取初始化超时时间
        if let Ok(timeout_str) = std::env::var("MCP_WINDBG_INIT_TIMEOUT") {
            if let Ok(timeout_secs) = timeout_str.parse::<u64>() {
                config.init_timeout = Duration::from_secs(timeout_secs);
            }
        }

        // 读取详细日志设置
        if let Ok(verbose_str) = std::env::var("MCP_WINDBG_VERBOSE") {
            config.verbose =
                verbose_str.eq_ignore_ascii_case("true") || verbose_str.eq_ignore_ascii_case("1");
        }

        config
    }
}

/// MCP 服务器
pub struct McpServer {
    /// 会话管理器
    session_manager: Arc<SessionManager>,
    /// 服务器配置
    #[allow(dead_code)]
    config: ServerConfig,
}

impl McpServer {
    /// 创建新的 MCP 服务器
    ///
    /// # 参数
    /// * `config` - 服务器配置
    ///
    /// # 返回
    /// 返回新创建的服务器实例
    pub fn new(config: ServerConfig) -> Self {
        info!("Creating MCP server");
        info!("Configuration: {:?}", config);

        let session_manager = Arc::new(SessionManager::new(
            config.timeout,
            config.init_timeout,
            config.verbose,
        ));

        Self {
            session_manager,
            config,
        }
    }

    /// 获取会话管理器的引用
    pub fn session_manager(&self) -> &Arc<SessionManager> {
        &self.session_manager
    }

    /// 将工具定义转换为 MCP Tool 格式
    fn convert_tools(&self) -> Vec<rmcp::model::Tool> {
        use rmcp::model::Tool;
        use std::borrow::Cow;

        self.list_tools()
            .into_iter()
            .map(|t| {
                let input_schema = if let serde_json::Value::Object(map) = t.input_schema {
                    Arc::new(map)
                } else {
                    Arc::new(serde_json::Map::new())
                };

                Tool {
                    name: Cow::Owned(t.name),
                    description: Some(Cow::Owned(t.description)),
                    input_schema,
                    annotations: None,
                    icons: None,
                    output_schema: None,
                    title: None,
                }
            })
            .collect()
    }

    /// 列出所有可用工具
    ///
    /// # 返回
    /// 返回工具定义列表
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "open_windbg_dump".to_string(),
                description: "Open and analyze Windows crash dump files".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "dump_path": {
                            "type": "string",
                            "description": "Path to the dump file"
                        },
                        "include_stack_trace": {
                            "type": "boolean",
                            "description": "Whether to include stack trace",
                            "default": false
                        },
                        "include_modules": {
                            "type": "boolean",
                            "description": "Whether to include module list",
                            "default": false
                        },
                        "include_threads": {
                            "type": "boolean",
                            "description": "Whether to include thread list",
                            "default": false
                        }
                    },
                    "required": ["dump_path"]
                }),
            },
            ToolDefinition {
                name: "open_windbg_remote".to_string(),
                description: "Connect to a remote debugging session".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "connection_string": {
                            "type": "string",
                            "description": "Remote connection string (e.g., tcp:Port=5005,Server=192.168.0.100)"
                        },
                        "include_stack_trace": {
                            "type": "boolean",
                            "description": "Whether to include stack trace",
                            "default": false
                        },
                        "include_modules": {
                            "type": "boolean",
                            "description": "Whether to include module list",
                            "default": false
                        },
                        "include_threads": {
                            "type": "boolean",
                            "description": "Whether to include thread list",
                            "default": false
                        }
                    },
                    "required": ["connection_string"]
                }),
            },
            ToolDefinition {
                name: "run_windbg_cmd".to_string(),
                description: "Execute WinDbg commands in an existing session".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "dump_path": {
                            "type": "string",
                            "description": "Dump file path (mutually exclusive with connection_string and program_path)"
                        },
                        "connection_string": {
                            "type": "string",
                            "description": "Remote connection string (mutually exclusive with dump_path and program_path)"
                        },
                        "program_path": {
                            "type": "string",
                            "description": "Target program path for launch debug session (mutually exclusive with dump_path and connection_string)"
                        },
                        "command": {
                            "type": "string",
                            "description": "WinDbg command to execute"
                        }
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "close_windbg_dump".to_string(),
                description: "Close a dump file session".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "dump_path": {
                            "type": "string",
                            "description": "Path to the dump file to close"
                        }
                    },
                    "required": ["dump_path"]
                }),
            },
            ToolDefinition {
                name: "close_windbg_remote".to_string(),
                description: "Close a remote debugging session".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "connection_string": {
                            "type": "string",
                            "description": "Remote connection string to close"
                        }
                    },
                    "required": ["connection_string"]
                }),
            },
            ToolDefinition {
                name: "launch_debug".to_string(),
                description: "Launch a program and start debugging it with CDB".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "program_path": {
                            "type": "string",
                            "description": "Path to the target program to debug"
                        },
                        "arguments": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Command line arguments for the target program"
                        },
                        "working_directory": {
                            "type": "string",
                            "description": "Working directory for the target program"
                        },
                        "symbols_path": {
                            "type": "string",
                            "description": "Debug symbols path (PDB files search path)"
                        },
                        "source_path": {
                            "type": "string",
                            "description": "Source file path for source-level debugging"
                        },
                        "include_stack_trace": {
                            "type": "boolean",
                            "description": "Whether to include stack trace in initial output",
                            "default": false
                        },
                        "include_modules": {
                            "type": "boolean",
                            "description": "Whether to include loaded modules in initial output",
                            "default": false
                        }
                    },
                    "required": ["program_path"]
                }),
            },
            ToolDefinition {
                name: "close_debug".to_string(),
                description: "Close a launch debug session and terminate the target program".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "program_path": {
                            "type": "string",
                            "description": "Path to the target program to close"
                        }
                    },
                    "required": ["program_path"]
                }),
            },
            ToolDefinition {
                name: "list_windbg_dumps".to_string(),
                description: "List dump files in a directory".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "directory_path": {
                            "type": "string",
                            "description": "Directory path to search (optional, defaults to system dump directory)"
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Whether to recursively search subdirectories",
                            "default": false
                        }
                    }
                }),
            },
        ]
    }

    /// 处理工具调用
    ///
    /// # 参数
    /// * `tool_name` - 工具名称
    /// * `arguments` - 工具参数（JSON 格式）
    ///
    /// # 返回
    /// 返回工具响应
    ///
    /// # 错误
    /// 如果工具不存在或执行失败，返回错误
    pub async fn handle_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolResponse, ServerError> {
        debug!("Handling tool call: {}", tool_name);

        match tool_name {
            "open_windbg_dump" => {
                let params: OpenWindbgDumpParams = serde_json::from_value(arguments)?;
                Ok(
                    tools::handle_open_windbg_dump(Arc::clone(&self.session_manager), params)
                        .await?,
                )
            }
            "open_windbg_remote" => {
                let params: OpenWindbgRemoteParams = serde_json::from_value(arguments)?;
                Ok(
                    tools::handle_open_windbg_remote(Arc::clone(&self.session_manager), params)
                        .await?,
                )
            }
            "run_windbg_cmd" => {
                let params: RunWindbgCmdParams = serde_json::from_value(arguments)?;
                Ok(tools::handle_run_windbg_cmd(Arc::clone(&self.session_manager), params).await?)
            }
            "close_windbg_dump" => {
                let params: CloseWindbgDumpParams = serde_json::from_value(arguments)?;
                Ok(
                    tools::handle_close_windbg_dump(Arc::clone(&self.session_manager), params)
                        .await?,
                )
            }
            "close_windbg_remote" => {
                let params: CloseWindbgRemoteParams = serde_json::from_value(arguments)?;
                Ok(
                    tools::handle_close_windbg_remote(Arc::clone(&self.session_manager), params)
                        .await?,
                )
            }
            "list_windbg_dumps" => {
                let params: ListWindbgDumpsParams = serde_json::from_value(arguments)?;
                Ok(tools::handle_list_windbg_dumps(params).await?)
            }
            "launch_debug" => {
                let params: LaunchDebugParams = serde_json::from_value(arguments)?;
                Ok(tools::handle_launch_debug(
                    Arc::clone(&self.session_manager),
                    params,
                    self.config.cdb_path.as_deref(),
                    self.config.symbols_path.as_deref(),
                    self.config.source_path.as_deref(),
                )
                .await?)
            }
            "close_debug" => {
                let params: CloseDebugParams = serde_json::from_value(arguments)?;
                Ok(tools::handle_close_debug(Arc::clone(&self.session_manager), params).await?)
            }
            _ => Err(ServerError::ProtocolError(format!(
                "Unknown tool: {}",
                tool_name
            ))),
        }
    }

    /// 运行服务器（stdio 传输）
    ///
    /// 启动服务器并监听 stdin 上的 MCP 请求。
    ///
    /// # 返回
    /// 如果服务器正常关闭，返回 Ok；否则返回错误
    ///
    /// # 错误
    /// 如果发生 I/O 错误或协议错误，返回错误
    pub async fn run(self) -> Result<(), ServerError> {
        use rmcp::*;

        info!("Starting MCP server (stdio transport)");
        info!("Available tools: {}", self.list_tools().len());

        // 使用 serve_server 启动服务器
        let transport = transport::stdio();

        // serve_server 返回 RunningService，我们需要等待它运行
        match serve_server(self, transport).await {
            Ok(running_service) => {
                info!("MCP server initialized successfully, waiting for requests...");

                // 等待服务运行直到关闭
                match running_service.waiting().await {
                    Ok(quit_reason) => {
                        info!("MCP server stopped: {:?}", quit_reason);
                        Ok(())
                    }
                    Err(e) => {
                        tracing::error!("MCP server task error: {}", e);
                        Err(ServerError::ProtocolError(e.to_string()))
                    }
                }
            }
            Err(e) => {
                tracing::error!("MCP server initialization error: {}", e);
                Err(ServerError::ProtocolError(e.to_string()))
            }
        }
    }
}

// 实现 ServerHandler trait
impl rmcp::ServerHandler for McpServer {
    fn get_info(&self) -> rmcp::model::InitializeResult {
        use rmcp::model::*;

        InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: None }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "mcp-windbg-rs".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                title: None,
                website_url: None,
            },
            instructions: Some(
                "WinDbg MCP Server - Provides Windows debugging tools for crash dump analysis"
                    .into(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _params: Option<rmcp::model::PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        Ok(rmcp::model::ListToolsResult {
            tools: self.convert_tools(),
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        params: rmcp::model::CallToolRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        use rmcp::model::Content;

        let tool_name = params.name.to_string();
        let arguments = if let Some(map) = params.arguments {
            serde_json::Value::Object(map)
        } else {
            serde_json::json!({})
        };

        // 调用工具处理器
        let response = match tool_name.as_ref() {
            "open_windbg_dump" => {
                let params: OpenWindbgDumpParams =
                    serde_json::from_value(arguments).map_err(|e| {
                        rmcp::ErrorData::invalid_params(
                            format!("Failed to parse parameters: {}", e),
                            None,
                        )
                    })?;
                tools::handle_open_windbg_dump(Arc::clone(&self.session_manager), params)
                    .await
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            }
            "open_windbg_remote" => {
                let params: OpenWindbgRemoteParams =
                    serde_json::from_value(arguments).map_err(|e| {
                        rmcp::ErrorData::invalid_params(
                            format!("Failed to parse parameters: {}", e),
                            None,
                        )
                    })?;
                tools::handle_open_windbg_remote(Arc::clone(&self.session_manager), params)
                    .await
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            }
            "run_windbg_cmd" => {
                let params: RunWindbgCmdParams =
                    serde_json::from_value(arguments).map_err(|e| {
                        rmcp::ErrorData::invalid_params(
                            format!("Failed to parse parameters: {}", e),
                            None,
                        )
                    })?;
                tools::handle_run_windbg_cmd(Arc::clone(&self.session_manager), params)
                    .await
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            }
            "close_windbg_dump" => {
                let params: CloseWindbgDumpParams =
                    serde_json::from_value(arguments).map_err(|e| {
                        rmcp::ErrorData::invalid_params(
                            format!("Failed to parse parameters: {}", e),
                            None,
                        )
                    })?;
                tools::handle_close_windbg_dump(Arc::clone(&self.session_manager), params)
                    .await
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            }
            "close_windbg_remote" => {
                let params: CloseWindbgRemoteParams =
                    serde_json::from_value(arguments).map_err(|e| {
                        rmcp::ErrorData::invalid_params(
                            format!("Failed to parse parameters: {}", e),
                            None,
                        )
                    })?;
                tools::handle_close_windbg_remote(Arc::clone(&self.session_manager), params)
                    .await
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            }
            "list_windbg_dumps" => {
                let params: ListWindbgDumpsParams =
                    serde_json::from_value(arguments).map_err(|e| {
                        rmcp::ErrorData::invalid_params(
                            format!("Failed to parse parameters: {}", e),
                            None,
                        )
                    })?;
                tools::handle_list_windbg_dumps(params)
                    .await
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            }
            "launch_debug" => {
                let params: LaunchDebugParams =
                    serde_json::from_value(arguments).map_err(|e| {
                        rmcp::ErrorData::invalid_params(
                            format!("Failed to parse parameters: {}", e),
                            None,
                        )
                    })?;
                tools::handle_launch_debug(
                    Arc::clone(&self.session_manager),
                    params,
                    self.config.cdb_path.as_deref(),
                    self.config.symbols_path.as_deref(),
                    self.config.source_path.as_deref(),
                )
                .await
                .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            }
            "close_debug" => {
                let params: CloseDebugParams =
                    serde_json::from_value(arguments).map_err(|e| {
                        rmcp::ErrorData::invalid_params(
                            format!("Failed to parse parameters: {}", e),
                            None,
                        )
                    })?;
                tools::handle_close_debug(Arc::clone(&self.session_manager), params)
                    .await
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            }
            _ => {
                return Err(rmcp::ErrorData::invalid_request(
                    format!("Unknown tool: {}", tool_name),
                    None,
                ));
            }
        };

        // 转换响应格式
        let content: Vec<Content> = response
            .content
            .into_iter()
            .map(|item| match item {
                crate::types::ContentItem::Text { text } => Content::text(text),
            })
            .collect();

        Ok(rmcp::model::CallToolResult {
            content,
            is_error: None,
            meta: None,
            structured_content: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(120));
        assert!(!config.verbose);
        assert!(config.cdb_path.is_none());
        assert!(config.symbols_path.is_none());
        assert!(config.source_path.is_none());
    }

    #[test]
    fn test_server_new() {
        let config = ServerConfig::default();
        let _server = McpServer::new(config);
    }

    #[test]
    fn test_list_tools_count() {
        let server = McpServer::new(ServerConfig::default());
        let tools = server.list_tools();
        assert_eq!(tools.len(), 8);
    }

    #[test]
    fn test_list_tools_names() {
        let server = McpServer::new(ServerConfig::default());
        let tools = server.list_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"open_windbg_dump"));
        assert!(names.contains(&"open_windbg_remote"));
        assert!(names.contains(&"run_windbg_cmd"));
        assert!(names.contains(&"close_windbg_dump"));
        assert!(names.contains(&"close_windbg_remote"));
        assert!(names.contains(&"launch_debug"));
        assert!(names.contains(&"close_debug"));
        assert!(names.contains(&"list_windbg_dumps"));
    }

    #[test]
    fn test_all_tools_have_descriptions() {
        let server = McpServer::new(ServerConfig::default());
        for tool in server.list_tools() {
            assert!(!tool.description.is_empty(), "Tool {} has empty description", tool.name);
        }
    }

    #[test]
    fn test_all_tool_schemas_have_property_descriptions() {
        let server = McpServer::new(ServerConfig::default());
        for tool in server.list_tools() {
            if let Some(props) = tool.input_schema.get("properties") {
                if let Some(obj) = props.as_object() {
                    for (param_name, param_schema) in obj {
                        assert!(
                            param_schema.get("description").is_some(),
                            "Tool '{}' param '{}' missing description",
                            tool.name, param_name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_launch_debug_schema_required_fields() {
        let server = McpServer::new(ServerConfig::default());
        let tools = server.list_tools();
        let launch = tools.iter().find(|t| t.name == "launch_debug").unwrap();
        let required = launch.input_schema.get("required").unwrap().as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].as_str().unwrap(), "program_path");
    }

    #[test]
    fn test_run_windbg_cmd_schema_has_program_path() {
        let server = McpServer::new(ServerConfig::default());
        let tools = server.list_tools();
        let cmd = tools.iter().find(|t| t.name == "run_windbg_cmd").unwrap();
        let props = cmd.input_schema.get("properties").unwrap();
        assert!(props.get("program_path").is_some());
        assert!(props.get("dump_path").is_some());
        assert!(props.get("connection_string").is_some());
    }

    #[tokio::test]
    async fn test_handle_tool_call_unknown_tool() {
        let server = McpServer::new(ServerConfig::default());
        let result = server.handle_tool_call("nonexistent_tool", serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_tool_call_invalid_params() {
        let server = McpServer::new(ServerConfig::default());
        // Missing required dump_path
        let result = server.handle_tool_call("open_windbg_dump", serde_json::json!({})).await;
        assert!(result.is_err());
    }
}
