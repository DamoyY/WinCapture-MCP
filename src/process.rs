use crate::error::AppResult;
use anyhow::Context as _;
use windows::Win32::{
    Foundation::{CloseHandle, ERROR_NO_MORE_FILES, HANDLE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    },
};
use windows::core::HRESULT;
pub(crate) fn find_process_ids_by_name(process_name: &str) -> AppResult<Vec<u32>> {
    let snapshot = anyhow::Context::context(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) },
        "创建进程快照失败",
    )?;
    let result = collect_matching_process_ids(snapshot, process_name);
    anyhow::Context::context(unsafe { CloseHandle(snapshot) }, "关闭进程快照句柄失败")?;
    result
}
fn collect_matching_process_ids(snapshot: HANDLE, process_name: &str) -> AppResult<Vec<u32>> {
    let matcher = ProcessNameMatcher::new(process_name);
    let mut entry = PROCESSENTRY32W {
        dwSize: anyhow::Context::context(
            u32::try_from(core::mem::size_of::<PROCESSENTRY32W>()),
            "进程快照项结构体大小无效",
        )?,
        ..PROCESSENTRY32W::default()
    };
    let mut process_ids = Vec::new();
    match unsafe { Process32FirstW(snapshot, core::ptr::from_mut(&mut entry)) } {
        Ok(()) => {}
        Err(error) if is_no_more_files(&error) => return Ok(process_ids),
        Err(error) => return Err(error).context("读取首个进程快照项失败"),
    }
    loop {
        match process_entry_name(&entry) {
            Ok(name) if matcher.matches(&name) => process_ids.push(entry.th32ProcessID),
            Ok(_) => {}
            Err(error) => tracing::error!(
                "读取进程快照项名称失败，pid={}: {error:#}",
                entry.th32ProcessID
            ),
        }
        match unsafe { Process32NextW(snapshot, core::ptr::from_mut(&mut entry)) } {
            Ok(()) => {}
            Err(error) if is_no_more_files(&error) => break,
            Err(error) => return Err(error).context("读取下一个进程快照项失败"),
        }
    }
    Ok(process_ids)
}
fn process_entry_name(entry: &PROCESSENTRY32W) -> AppResult<String> {
    let name_len = entry
        .szExeFile
        .iter()
        .position(|code_unit| *code_unit == 0)
        .ok_or_else(|| anyhow::anyhow!("进程名缺少字符串终止符: {}", entry.th32ProcessID))?;
    let name_slice =
        anyhow::Context::context(entry.szExeFile.get(..name_len), "进程名长度超出缓冲区")?;
    if name_slice.is_empty() {
        anyhow::bail!("进程名为空: {}", entry.th32ProcessID);
    }
    anyhow::Context::with_context(String::from_utf16(name_slice), || {
        format!("进程名不是有效 UTF-16: {}", entry.th32ProcessID)
    })
}
fn is_no_more_files(error: &windows::core::Error) -> bool {
    error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0)
}
fn normalize_process_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}
fn process_name_without_exe(name: &str) -> &str {
    name.trim_end_matches(".exe")
}
struct ProcessNameMatcher {
    normalized_target: String,
    target_without_extension: String,
}
impl ProcessNameMatcher {
    fn new(target: &str) -> Self {
        let normalized_target = normalize_process_name(target);
        let target_without_extension = process_name_without_exe(&normalized_target).to_owned();
        Self {
            normalized_target,
            target_without_extension,
        }
    }
    fn matches(&self, actual: &str) -> bool {
        let normalized_actual = normalize_process_name(actual);
        normalized_actual == self.normalized_target
            || process_name_without_exe(&normalized_actual) == self.target_without_extension
    }
}
#[cfg(test)]
mod tests {
    use super::ProcessNameMatcher;
    #[test]
    fn accepts_exact_name() {
        assert!(ProcessNameMatcher::new("notepad.exe").matches("notepad.exe"));
    }
    #[test]
    fn accepts_name_without_extension() {
        assert!(ProcessNameMatcher::new("notepad").matches("notepad.exe"));
    }
}
