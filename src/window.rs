use crate::{
    capture::AppResult,
    model::{WindowEntry, WindowRect},
};
use anyhow::{Context as _, anyhow};
use std::collections::HashSet;
use sysinfo::System;
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{GetClassNameW, IsIconic, IsWindowVisible},
};
use windows_capture::window::Window;
pub(crate) fn find_windows_by_process(process_name: &str) -> AppResult<Vec<WindowEntry>> {
    let process_ids = matching_process_ids(process_name)?;
    if process_ids.is_empty() {
        return Ok(Vec::new());
    }
    let windows = Window::enumerate().context("枚举可捕获窗口失败")?;
    let mut entries = Vec::new();
    for window in windows {
        let pid = window.process_id().context("读取窗口进程 ID 失败")?;
        if process_ids.contains(&pid) {
            entries.push(build_window_entry(&window, pid)?);
        }
    }
    entries.sort_by(|left, right| left.hwnd.cmp(&right.hwnd));
    Ok(entries)
}
pub(crate) fn parse_hwnd(raw: &str) -> AppResult<HWND> {
    let trimmed = raw.trim();
    let value = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .map_or_else(
            || trimmed.parse::<usize>(),
            |hex| usize::from_str_radix(hex, 16),
        )
        .with_context(|| format!("无效的 HWND: {raw}"))?;
    if value == 0 {
        return Err(anyhow!("HWND 不能为 0"));
    }
    Ok(HWND(core::ptr::with_exposed_provenance_mut(value)))
}
fn matching_process_ids(process_name: &str) -> AppResult<HashSet<u32>> {
    let matcher = ProcessNameMatcher::new(process_name)?;
    let system = System::new_all();
    Ok(system
        .processes()
        .iter()
        .filter(|&(_, process)| matcher.matches(&process.name().to_string_lossy()))
        .map(|(pid, _)| pid.as_u32())
        .collect())
}
fn build_window_entry(window: &Window, pid: u32) -> AppResult<WindowEntry> {
    let hwnd = HWND(window.as_raw_hwnd());
    let rect = window.rect().context("读取窗口矩形失败")?;
    Ok(WindowEntry {
        hwnd: format_hwnd(hwnd),
        pid,
        title: window.title().context("读取窗口标题失败")?,
        class_name: read_class_name(hwnd)?,
        visible: unsafe { IsWindowVisible(hwnd).as_bool() },
        minimized: unsafe { IsIconic(hwnd).as_bool() },
        rect: WindowRect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        },
    })
}
fn read_class_name(hwnd: HWND) -> AppResult<String> {
    let mut buffer = [0_u16; 256];
    let copied =
        usize::try_from(unsafe { GetClassNameW(hwnd, &mut buffer) }).context("读取窗口类名失败")?;
    if copied == 0 {
        return Err(anyhow!("窗口类名为空"));
    }
    let class_name = buffer.get(..copied).context("窗口类名长度超出缓冲区")?;
    String::from_utf16(class_name).context("窗口类名不是有效 UTF-16")
}
fn format_hwnd(hwnd: HWND) -> String {
    format!("0x{:016X}", hwnd.0.addr())
}
struct ProcessNameMatcher {
    normalized_target: String,
    target_without_extension: String,
}
impl ProcessNameMatcher {
    fn new(target: &str) -> AppResult<Self> {
        let normalized_target = normalize_process_name(target);
        let target_without_extension = remove_exe_suffix(&normalized_target).to_owned();
        if target_without_extension.is_empty() {
            return Err(anyhow!("进程名不能为空"));
        }
        Ok(Self {
            normalized_target,
            target_without_extension,
        })
    }
    fn matches(&self, actual: &str) -> bool {
        let normalized_actual = normalize_process_name(actual);
        normalized_actual == self.normalized_target
            || remove_exe_suffix(&normalized_actual) == self.target_without_extension
    }
}
fn normalize_process_name(name: &str) -> String {
    name.trim().to_lowercase()
}
fn remove_exe_suffix(name: &str) -> &str {
    name.strip_suffix(".exe").unwrap_or(name)
}
#[cfg(test)]
mod tests {
    use super::{ProcessNameMatcher, format_hwnd, parse_hwnd};
    #[test]
    fn process_name_matching_is_case_insensitive_and_extension_optional() {
        let Ok(matcher) = ProcessNameMatcher::new(" Explorer ") else {
            panic!("有效进程名应能创建匹配器");
        };
        assert!(matcher.matches("EXPLORER.EXE"));
        assert!(!matcher.matches("explorer-helper.exe"));
    }
    #[test]
    fn hwnd_accepts_hex_and_decimal_but_rejects_zero() {
        let Ok(hex_hwnd) = parse_hwnd("0x2A") else {
            panic!("十六进制 HWND 应能解析");
        };
        let Ok(decimal_hwnd) = parse_hwnd("42") else {
            panic!("十进制 HWND 应能解析");
        };
        assert_eq!(format_hwnd(hex_hwnd), "0x000000000000002A");
        assert_eq!(format_hwnd(decimal_hwnd), "0x000000000000002A");
        let Err(_) = parse_hwnd("0") else {
            panic!("零 HWND 必须被拒绝");
        };
    }
}
