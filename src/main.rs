mod app;
mod capture;
mod capture_item;
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
    service.waiting().await?;
    Ok(())
}
