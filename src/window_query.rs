use crate::{
    error::AppResult,
    process::{matches_process_name, query_process_name},
    tool_types::WindowEntry,
    window_details::build_window_entry,
};
use std::collections::HashMap;
use windows::Win32::{
    Foundation::{HWND, LPARAM, TRUE},
    UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId},
};
struct WindowSearchState {
    process_name: String,
    names_by_pid: HashMap<u32, Option<String>>,
    windows: Vec<WindowEntry>,
}
pub(crate) fn find_windows_by_process(process_name: &str) -> AppResult<Vec<WindowEntry>> {
    let mut state = WindowSearchState {
        process_name: process_name.to_owned(),
        names_by_pid: HashMap::new(),
        windows: Vec::new(),
    };
    let state_ptr = core::ptr::from_mut(&mut state);
    let state_addr =
        anyhow::Context::context(isize::try_from(state_ptr.addr()), "窗口搜索状态指针无效")?;
    unsafe {
        EnumWindows(Some(enum_windows_callback), LPARAM(state_addr))?;
    }
    state
        .windows
        .sort_by(|left, right| left.hwnd.cmp(&right.hwnd));
    Ok(state.windows)
}
unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let Ok(state_addr) = usize::try_from(lparam.0) else {
        tracing::error!("窗口搜索状态指针无效: {}", lparam.0);
        return TRUE;
    };
    let state_ptr = core::ptr::with_exposed_provenance_mut::<WindowSearchState>(state_addr);
    let state = unsafe { &mut *state_ptr };
    let mut pid = 0_u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(core::ptr::from_mut(&mut pid)));
    }
    if pid == 0 {
        return TRUE;
    }
    let process_name_entry =
        state
            .names_by_pid
            .entry(pid)
            .or_insert_with(|| match query_process_name(pid) {
                Ok(name) => Some(name),
                Err(error) => {
                    tracing::error!("读取进程名失败，pid={pid}: {error:#}");
                    None
                }
            });
    let Some(process_name) = process_name_entry.as_deref() else {
        return TRUE;
    };
    if !matches_process_name(process_name, &state.process_name) {
        return TRUE;
    }
    match build_window_entry(hwnd, pid) {
        Ok(entry) => state.windows.push(entry),
        Err(error) => tracing::error!("读取窗口信息失败，pid={pid}: {error:#}"),
    }
    TRUE
}
