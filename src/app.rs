use crate::{
    capture::capture_window_png,
    error::to_mcp_error,
    hwnd::parse_hwnd,
    tool_types::{CaptureWindowRequest, FindWindowsRequest},
    window_query::find_windows_by_process,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use serde_json::json;
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
    ) -> Result<CallToolResult, ErrorData> {
        let _: &ToolRouter<Self> = &self.tool_router;
        let windows =
            find_windows_by_process(&process_name).map_err(|error| to_mcp_error(&error))?;
        Ok(CallToolResult::structured(
            json ! ({ "process_name" : process_name , "windows" : windows }),
        ))
    }
    #[tool(description = "输入 HWND，返回该窗口的 PNG 截图")]
    fn capture_hwnd(
        &self,
        Parameters(CaptureWindowRequest { hwnd }): Parameters<CaptureWindowRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let _: &ToolRouter<Self> = &self.tool_router;
        let parsed_hwnd = parse_hwnd(&hwnd).map_err(|error| to_mcp_error(&error))?;
        let png = capture_window_png(parsed_hwnd).map_err(|error| to_mcp_error(&error))?;
        let image = Content::image(STANDARD.encode(png), "image/png");
        Ok(CallToolResult::success(vec![image]))
    }
}
# [tool_handler (router = self . tool_router)]
#[expect(
    clippy::missing_trait_methods,
    reason = "rmcp::ServerHandler 提供了大量默认实现，这里仅覆盖当前服务需要的入口"
)]
impl ServerHandler for ScreenServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "提供按进程名列出窗口句柄，以及按 HWND 返回 PNG 截图的工具。".to_owned(),
        )
    }
}
