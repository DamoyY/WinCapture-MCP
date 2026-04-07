use crate::{
    capture::capture_window_png,
    error::to_tool_error,
    hwnd::parse_hwnd,
    tool_types::{CaptureWindowRequest, FindWindowsRequest, FindWindowsResponse},
    window_query::find_windows_by_process,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rmcp::{
    Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
#[derive(Clone, Debug)]
pub(crate) struct ScreenServer {
    tool_router: ToolRouter<Self>,
}
#[tool_router]
impl ScreenServer {
    pub(crate) fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
    #[tool(description = "输入进程名，返回匹配窗口的 HWND 列表及窗口元信息")]
    fn list_hwnds(
        &self,
        Parameters(FindWindowsRequest { process_name }): Parameters<FindWindowsRequest>,
    ) -> Result<Json<FindWindowsResponse>, String> {
        let _: &ToolRouter<Self> = &self.tool_router;
        match find_windows_by_process(&process_name) {
            Ok(windows) => Ok(Json(FindWindowsResponse {
                process_name,
                windows,
            })),
            Err(error) => Err(to_tool_error(&error)),
        }
    }
    #[tool(description = "输入 HWND，返回该窗口的 PNG 截图")]
    fn capture_hwnd(
        &self,
        Parameters(CaptureWindowRequest { hwnd }): Parameters<CaptureWindowRequest>,
    ) -> Result<Content, String> {
        let _: &ToolRouter<Self> = &self.tool_router;
        let parsed_hwnd = match parse_hwnd(&hwnd) {
            Ok(parsed_hwnd) => parsed_hwnd,
            Err(error) => return Err(to_tool_error(&error)),
        };
        let png = match capture_window_png(parsed_hwnd) {
            Ok(png) => png,
            Err(error) => return Err(to_tool_error(&error)),
        };
        Ok(Content::image(STANDARD.encode(png), "image/png"))
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
            .with_instructions("提供按进程名列出窗口句柄，以及按 HWND 返回 PNG 截图的工具。")
    }
}
