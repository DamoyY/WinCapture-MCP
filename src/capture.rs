use crate::{
    capture_flow::capture_png,
    capture_item::create_capture_item,
    d3d::{CaptureDevice, create_capture_device},
    error::AppResult,
};
use anyhow::anyhow;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use windows::{
    Graphics::Capture::GraphicsCaptureSession,
    Win32::{Foundation::HWND, System::Com::CoIncrementMTAUsage},
};
static MTA_USAGE_STATE: OnceLock<Result<(), String>> = OnceLock::new();
pub(crate) struct CaptureContext {
    device: CaptureDevice,
}
impl CaptureContext {
    pub(crate) fn new() -> AppResult<Self> {
        ensure_mta_usage()?;
        if !GraphicsCaptureSession::IsSupported()? {
            return Err(anyhow!("当前系统不支持 Windows.Graphics.Capture"));
        }
        Ok(Self {
            device: create_capture_device()?,
        })
    }
    pub(crate) fn capture_window_png(&self, runtime: &Runtime, hwnd: HWND) -> AppResult<Vec<u8>> {
        let item = create_capture_item(hwnd)?;
        capture_png(&self.device, &item, runtime)
    }
}
pub(crate) fn ensure_mta_usage() -> AppResult<()> {
    let state = MTA_USAGE_STATE.get_or_init(|| match unsafe { CoIncrementMTAUsage() } {
        Ok(cookie) => {
            let leaked_cookie = Box::leak(Box::new(cookie));
            tracing::trace!(?leaked_cookie, "全局 MTA 使用计数已初始化");
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    });
    if let Err(error) = state.as_ref() {
        return Err(anyhow!("{error}"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{CaptureContext, ensure_mta_usage};
    use crate::{hwnd::parse_hwnd, window_query::find_windows_by_process};
    use core::{fmt::Display, time::Duration};
    use std::sync::{Mutex, OnceLock};
    use tokio::runtime::{Builder, Runtime};
    static CAPTURE_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    fn capture_test_mutex() -> &'static Mutex<()> {
        CAPTURE_TEST_MUTEX.get_or_init(|| Mutex::new(()))
    }
    fn must<T, E: Display>(result: Result<T, E>, message: &str) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{message}: {error}"),
        }
    }
    fn find_test_hwnd() -> windows::Win32::Foundation::HWND {
        let windows = must(
            find_windows_by_process("explorer"),
            "应能枚举 explorer 窗口",
        );
        let window = windows
            .into_iter()
            .find(|window| window.visible && !window.minimized)
            .unwrap_or_else(|| panic!("应至少存在一个可见且未最小化的 explorer 窗口"));
        must(parse_hwnd(&window.hwnd), "窗口句柄应能成功解析")
    }
    #[test]
    fn ensure_mta_usage_is_idempotent() {
        must(ensure_mta_usage(), "第一次初始化 MTA 应成功");
        must(ensure_mta_usage(), "重复初始化 MTA 应成功");
    }
    #[test]
    fn capture_window_png_keeps_process_usable_after_capture() {
        let _guard = must(capture_test_mutex().lock(), "应能独占执行截图回归测试");
        let runtime = build_test_runtime();
        let capture_context = must(CaptureContext::new(), "应能初始化截图上下文");
        let hwnd = find_test_hwnd();
        let png = must(
            capture_context.capture_window_png(&runtime, hwnd),
            "截图应成功",
        );
        assert!(png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
        std::thread::sleep(Duration::from_millis(500));
        let windows_after_capture = must(
            find_windows_by_process("explorer"),
            "截图后应仍能继续查询窗口",
        );
        assert!(
            windows_after_capture
                .iter()
                .any(|window| window.visible && !window.minimized),
            "截图后应仍能查到可见窗口"
        );
        let second_png = must(
            capture_context.capture_window_png(&runtime, hwnd),
            "同一进程内再次截图应成功",
        );
        assert!(second_png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    }
    fn build_test_runtime() -> Runtime {
        must(
            Builder::new_current_thread().enable_time().build(),
            "应能创建测试运行时",
        )
    }
}
