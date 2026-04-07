use crate::{
    error::AppResult,
    hwnd::format_hwnd,
    tool_types::{WindowEntry, WindowRect},
};
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::{
        GetClassNameW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsIconic,
        IsWindowVisible,
    },
};
pub(crate) fn build_window_entry(hwnd: HWND, pid: u32) -> AppResult<WindowEntry> {
    Ok(WindowEntry {
        hwnd: format_hwnd(hwnd),
        pid,
        title: read_window_text(hwnd)?,
        class_name: read_class_name(hwnd)?,
        visible: unsafe { IsWindowVisible(hwnd).as_bool() },
        minimized: unsafe { IsIconic(hwnd).as_bool() },
        rect: read_window_rect(hwnd)?,
    })
}
fn read_window_text(hwnd: HWND) -> AppResult<String> {
    let length = anyhow::Context::context(
        usize::try_from(unsafe { GetWindowTextLengthW(hwnd) }),
        "读取窗口标题长度失败",
    )?;
    let buffer_len = anyhow::Context::context(length.checked_add(1), "窗口标题长度溢出")?;
    let mut buffer = vec![0_u16; buffer_len];
    let copied = anyhow::Context::context(
        usize::try_from(unsafe { GetWindowTextW(hwnd, &mut buffer) }),
        "读取窗口标题失败",
    )?;
    let text = anyhow::Context::context(buffer.get(..copied), "窗口标题长度超出缓冲区")?;
    Ok(String::from_utf16_lossy(text))
}
fn read_class_name(hwnd: HWND) -> AppResult<String> {
    let mut buffer = [0_u16; 256];
    let copied = anyhow::Context::context(
        usize::try_from(unsafe { GetClassNameW(hwnd, &mut buffer) }),
        "读取窗口类名失败",
    )?;
    let class_name = anyhow::Context::context(buffer.get(..copied), "窗口类名长度超出缓冲区")?;
    Ok(String::from_utf16_lossy(class_name))
}
fn read_window_rect(hwnd: HWND) -> AppResult<WindowRect> {
    let mut rect = RECT::default();
    anyhow::Context::with_context(
        unsafe { GetWindowRect(hwnd, core::ptr::from_mut(&mut rect)) },
        || "读取窗口矩形失败",
    )?;
    Ok(WindowRect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}
