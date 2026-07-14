extern crate alloc;
mod capture;
mod model;
mod server;
mod window;
use anyhow::Result;
use mimalloc::MiMalloc;
use rmcp::transport::stdio;
use server::ScreenServer;
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
