use crate::{
    capture::capture_window_png,
    model::{SearchHwndRequest, SearchHwndResponse, WindowScreenshotRequest},
    window::find_windows_by_process,
};
use anyhow::Error as AnyhowError;
use base64_turbo::STANDARD;
use rmcp::{
    ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
#[derive(Clone, Debug)]
pub(crate) struct ScreenServer {
    tool_router: ToolRouter<Self>,
}
async fn run_blocking_tool<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    let task_result = tokio::task::spawn_blocking(task)
        .await
        .map_err(AnyhowError::new)
        .map_err(|error| report_tool_error(&error))?;
    task_result.map_err(|error| report_tool_error(&error))
}
fn report_tool_error(error: &AnyhowError) -> String {
    tracing::error!("{error:#}");
    error.to_string()
}
#[tool_router]
impl ScreenServer {
    pub(crate) fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
    #[tool(description = "输入进程名，返回匹配的可捕获窗口 HWND 及窗口元信息")]
    async fn search_hwnd(
        &self,
        Parameters(SearchHwndRequest { process_name }): Parameters<SearchHwndRequest>,
    ) -> Result<Json<SearchHwndResponse>, String> {
        run_blocking_tool(move || {
            let windows = find_windows_by_process(&process_name)?;
            Ok(Json(SearchHwndResponse {
                process_name,
                windows,
            }))
        })
        .await
    }
    #[tool(description = "输入 HWND，使用 Windows.Graphics.Capture 返回窗口画面")]
    async fn window_screenshot(
        &self,
        Parameters(WindowScreenshotRequest { hwnd }): Parameters<WindowScreenshotRequest>,
    ) -> Result<ContentBlock, String> {
        let png = capture_window_png(hwnd)
            .await
            .map_err(|error| report_tool_error(&error))?;
        Ok(ContentBlock::image(STANDARD.encode(png), "image/png"))
    }
}
# [tool_handler (router = self . tool_router)]
#[expect(
    clippy::missing_trait_methods,
    reason = "rmcp::ServerHandler 为未启用的 MCP 能力提供默认实现"
)]
#[expect(
    clippy::unused_async_trait_impl,
    reason = "异步 trait 由 rmcp 的 tool_handler 宏生成"
)]
impl ServerHandler for ScreenServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("提供按进程名搜索可捕获窗口，以及按 HWND 返回 PNG 截图的工具。")
    }
}
#[cfg(test)]
mod tests {
    use super::ScreenServer;
    #[test]
    fn search_tool_declares_structured_output_schema() {
        let server = ScreenServer::new();
        let matching_tool = server
            .tool_router
            .list_all()
            .into_iter()
            .find(|tool| tool.name == "search_hwnd");
        let Some(search_tool) = matching_tool else {
            panic!("search_hwnd 工具应存在");
        };
        assert!(search_tool.output_schema.is_some());
    }
}
