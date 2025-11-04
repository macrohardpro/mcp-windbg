use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化 tracing 日志订阅器
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("MCP WinDbg Server starting...");

    // TODO: 初始化服务器并开始监听

    Ok(())
}
