use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FindWindowsRequest {
    pub(crate) process_name: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CaptureWindowRequest {
    pub(crate) hwnd: String,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FindWindowsResponse {
    pub(crate) process_name: String,
    pub(crate) windows: Vec<WindowEntry>,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub(crate) struct WindowRect {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub(crate) struct WindowEntry {
    pub(crate) hwnd: String,
    pub(crate) pid: u32,
    pub(crate) title: String,
    pub(crate) class_name: String,
    pub(crate) visible: bool,
    pub(crate) minimized: bool,
    pub(crate) rect: WindowRect,
}
