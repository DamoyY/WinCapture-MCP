use crate::error::AppResult;
use std::{ffi::OsString, path::Path};
use windows::{
    Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    },
    core::PWSTR,
};
pub(crate) fn query_process_name(pid: u32) -> AppResult<String> {
    let handle = anyhow::Context::with_context(
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) },
        || format!("打开进程失败: {pid}"),
    )?;
    let mut buffer = [0_u16; 1024];
    let mut size = anyhow::Context::context(u32::try_from(buffer.len()), "进程路径缓冲区长度无效")?;
    let result = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            core::ptr::from_mut(&mut size),
        )
    };
    let close_result = unsafe { CloseHandle(handle) };
    anyhow::Context::with_context(close_result, || format!("关闭进程句柄失败: {pid}"))?;
    anyhow::Context::with_context(result, || format!("读取进程名失败: {pid}"))?;
    let size_usize = anyhow::Context::context(usize::try_from(size), "进程路径长度无效")?;
    let path_slice = anyhow::Context::context(buffer.get(..size_usize), "进程路径长度超出缓冲区")?;
    let path = <OsString as std::os::windows::ffi::OsStringExt>::from_wide(path_slice);
    let name = Path::new(&path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("提取进程名失败: {pid}"))?;
    Ok(name)
}
pub(crate) fn normalize_process_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}
pub(crate) fn matches_process_name(actual: &str, target: &str) -> bool {
    let normalized_actual = normalize_process_name(actual);
    let normalized_target = normalize_process_name(target);
    normalized_actual == normalized_target
        || normalized_actual.trim_end_matches(".exe") == normalized_target.trim_end_matches(".exe")
}
#[cfg(test)]
mod tests {
    use super::matches_process_name;
    #[test]
    fn accepts_exact_name() {
        assert!(matches_process_name("notepad.exe", "notepad.exe"));
    }
    #[test]
    fn accepts_name_without_extension() {
        assert!(matches_process_name("notepad.exe", "notepad"));
    }
}
