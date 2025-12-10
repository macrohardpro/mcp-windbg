use clap::Parser;
use mcp_windbg_rs::server::{McpServer, ServerConfig};
use tracing::info;

/// MCP WinDbg 服务器 - Windows 崩溃转储分析工具
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 命令执行超时时间（秒）
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// 初始化超时时间（秒）
    #[arg(long, default_value = "120")]
    init_timeout: u64,

    /// 启用详细日志
    #[arg(long, default_value = "false")]
    verbose: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 初始化 tracing 日志订阅器
    // 重要：日志必须输出到 stderr，因为 stdout 用于 MCP JSON-RPC 通信
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // 强制输出到 stderr
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();

    info!("MCP WinDbg Server starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // 从环境变量读取配置
    let mut config = ServerConfig::from_env();

    // 命令行参数覆盖配置
    config.timeout = std::time::Duration::from_secs(args.timeout);
    config.init_timeout = std::time::Duration::from_secs(args.init_timeout);
    config.verbose = args.verbose;

    // 创建并启动服务器
    let server = McpServer::new(config);

    // 设置 Ctrl+C 处理
    let session_manager = server.session_manager().clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to set Ctrl+C handler");
        info!("Received shutdown signal, cleaning up...");
        if let Err(e) = session_manager.close_all_sessions().await {
            tracing::error!("Failed to close sessions: {}", e);
        }
        std::process::exit(0);
    });

    // 运行服务器
    // serve_server 应该持续运行，如果它返回了说明连接关闭
    match server.run().await {
        Ok(_) => {
            info!("MCP server shut down normally");
        }
        Err(e) => {
            tracing::error!("MCP server error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
