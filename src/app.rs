use crate::{
    capture_service::capture_window_png,
    error::to_tool_error,
    hwnd::{hwnd_to_value, parse_hwnd},
    sonic_json::SonicToolResult,
    tool_types::{SearchHwndRequest, SearchHwndResponse, WindowScreenshotRequest},
    window_query::find_windows_by_process,
};
use anyhow::Error as AnyhowError;
use base64_turbo::STANDARD;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Content, Implementation, ServerCapabilities, ServerInfo},
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
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| to_tool_error(&AnyhowError::new(error)))?
        .map_err(|error| to_tool_error(&error))
}
#[tool_router]
impl ScreenServer {
    pub(crate) fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
    #[tool(description = "输入进程名，返回匹配该窗口的 HWND 列表及窗口元信息")]
    async fn search_hwnd(
        &self,
        Parameters(SearchHwndRequest { process_name }): Parameters<SearchHwndRequest>,
    ) -> SonicToolResult<SearchHwndResponse, String> {
        let _: &ToolRouter<Self> = &self.tool_router;
        SonicToolResult(
            run_blocking_tool(move || {
                let windows = find_windows_by_process(&process_name)?;
                Ok(SearchHwndResponse {
                    process_name,
                    windows,
                })
            })
            .await,
        )
    }
    #[tool(description = "输入 HWND，返回该窗口的画面截图")]
    async fn window_screenshot(
        &self,
        Parameters(WindowScreenshotRequest { hwnd }): Parameters<WindowScreenshotRequest>,
    ) -> Result<Content, String> {
        let _: &ToolRouter<Self> = &self.tool_router;
        let hwnd_value = parse_hwnd(&hwnd)
            .map(hwnd_to_value)
            .map_err(|error| to_tool_error(&error))?;
        let png = capture_window_png(hwnd_value)
            .await
            .map_err(|error| to_tool_error(&error))?;
        let base64_png = STANDARD.encode(png);
        Ok(Content::image(base64_png, "image/png"))
    }
}
# [tool_handler (router = self . tool_router)]
#[expect(
    clippy::missing_trait_methods,
    reason = "rmcp::ServerHandler 提供了大量默认实现，这里仅覆盖当前服务需要的入口"
)]
impl ServerHandler for ScreenServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("提供按进程名搜索窗口句柄，以及按 HWND 返回 PNG 截图的工具。")
    }
}
