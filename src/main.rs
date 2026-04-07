mod app;
mod capture;
mod capture_item;
mod capture_service;
mod d3d;
mod error;
mod frame;
mod hwnd;
mod process;
mod sonic_json;
mod tool_types;
mod window_details;
mod window_query;
use anyhow::Result;
use app::ScreenServer;
use capture_service::warmup_capture_service;
use mimalloc::MiMalloc;
use rmcp::transport::stdio;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let service = rmcp::ServiceExt::serve(ScreenServer::new(), stdio()).await?;
    core::mem::drop(tokio::task::spawn_blocking(
        || match warmup_capture_service() {
            Ok(()) => tracing::info!("截图服务预热完成"),
            Err(error) => tracing::error!("截图服务预热失败: {error:#}"),
        },
    ));
    service.waiting().await?;
    Ok(())
}
