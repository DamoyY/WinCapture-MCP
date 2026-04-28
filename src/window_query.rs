use crate::{
    error::AppResult, process::find_process_ids_by_name, tool_types::WindowEntry,
    window_details::build_window_entry,
};
use std::collections::HashSet;
use windows::Win32::{
    Foundation::{HWND, LPARAM, TRUE},
    UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId},
};
struct WindowSearchState {
    target_process_ids: HashSet<u32>,
    windows: Vec<WindowEntry>,
}
pub(crate) fn find_windows_by_process(process_name: &str) -> AppResult<Vec<WindowEntry>> {
    let target_process_ids = find_process_ids_by_name(process_name)?
        .into_iter()
        .collect::<HashSet<_>>();
    if target_process_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut state = WindowSearchState {
        target_process_ids,
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
    if !state.target_process_ids.contains(&pid) {
        return TRUE;
    }
    match build_window_entry(hwnd, pid) {
        Ok(entry) => state.windows.push(entry),
        Err(error) => tracing::error!("读取窗口信息失败，pid={pid}: {error:#}"),
    }
    TRUE
}
