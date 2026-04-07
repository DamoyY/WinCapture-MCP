use crate::{
    capture_item::create_capture_item, d3d::create_capture_device, error::AppResult,
    frame::capture_png,
};
use anyhow::anyhow;
use std::sync::OnceLock;
use windows::{
    Graphics::Capture::GraphicsCaptureSession,
    Win32::{Foundation::HWND, System::Com::CoIncrementMTAUsage},
};
static MTA_USAGE_STATE: OnceLock<Result<(), String>> = OnceLock::new();
pub(crate) fn capture_window_png(hwnd: HWND) -> AppResult<Vec<u8>> {
    ensure_mta_usage()?;
    if !GraphicsCaptureSession::IsSupported()? {
        return Err(anyhow!("当前系统不支持 Windows.Graphics.Capture"));
    }
    let item = create_capture_item(hwnd)?;
    let device = create_capture_device()?;
    capture_png(&device, &item)
}
fn ensure_mta_usage() -> AppResult<()> {
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
    use super::{capture_window_png, ensure_mta_usage};
    use crate::{hwnd::parse_hwnd, window_query::find_windows_by_process};
    use std::sync::{Mutex, OnceLock};
    static CAPTURE_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    fn capture_test_mutex() -> &'static Mutex<()> {
        CAPTURE_TEST_MUTEX.get_or_init(|| Mutex::new(()))
    }
    fn find_test_hwnd() -> windows::Win32::Foundation::HWND {
        let windows = find_windows_by_process("explorer").expect("应能枚举 explorer 窗口");
        let window = windows
            .into_iter()
            .find(|window| window.visible && !window.minimized)
            .expect("应至少存在一个可见且未最小化的 explorer 窗口");
        parse_hwnd(&window.hwnd).expect("窗口句柄应能成功解析")
    }
    #[test]
    fn ensure_mta_usage_is_idempotent() {
        ensure_mta_usage().expect("第一次初始化 MTA 应成功");
        ensure_mta_usage().expect("重复初始化 MTA 应成功");
    }
    #[test]
    fn capture_window_png_keeps_process_usable_after_capture() {
        let _guard = capture_test_mutex()
            .lock()
            .expect("应能独占执行截图回归测试");
        let hwnd = find_test_hwnd();
        let png = capture_window_png(hwnd).expect("截图应成功");
        assert!(png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
        std::thread::sleep(std::time::Duration::from_millis(500));
        let windows_after_capture =
            find_windows_by_process("explorer").expect("截图后应仍能继续查询窗口");
        assert!(
            windows_after_capture
                .iter()
                .any(|window| window.visible && !window.minimized),
            "截图后应仍能查到可见窗口"
        );
        let second_png = capture_window_png(hwnd).expect("同一进程内再次截图应成功");
        assert!(second_png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    }
}
