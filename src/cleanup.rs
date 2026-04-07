use crate::error::AppResult;
use anyhow::anyhow;
pub(crate) fn build_first_frame_result(
    png_result: AppResult<Vec<u8>>,
    close_result: windows::core::Result<()>,
) -> Result<Vec<u8>, String> {
    match (png_result, close_result) {
        (Ok(png), Ok(())) => Ok(png),
        (Err(error), Ok(())) => Err(error.to_string()),
        (Ok(_), Err(error)) => Err(format!("关闭捕获帧失败: {error}")),
        (Err(error), Err(close_error)) => Err(format!("{error:#}; 关闭捕获帧失败: {close_error}")),
    }
}
pub(crate) fn finalize_capture_result(
    capture_result: AppResult<Vec<u8>>,
    session_close_result: windows::core::Result<()>,
    remove_handler_result: windows::core::Result<()>,
    frame_pool_close_result: windows::core::Result<()>,
) -> AppResult<Vec<u8>> {
    let mut cleanup_errors = Vec::new();
    if let Err(error) = session_close_result {
        cleanup_errors.push(format!("关闭捕获会话失败: {error}"));
    }
    if let Err(error) = remove_handler_result {
        cleanup_errors.push(format!("移除帧到达回调失败: {error}"));
    }
    if let Err(error) = frame_pool_close_result {
        cleanup_errors.push(format!("关闭捕获帧池失败: {error}"));
    }
    if cleanup_errors.is_empty() {
        return capture_result;
    }
    let cleanup_summary = cleanup_errors.join("; ");
    match capture_result {
        Ok(_) => Err(anyhow!(cleanup_summary)),
        Err(error) => Err(anyhow!("{error:#}; {cleanup_summary}")),
    }
}
#[cfg(test)]
mod tests {
    use super::{build_first_frame_result, finalize_capture_result};
    use anyhow::anyhow;
    use windows::Win32::Foundation::E_POINTER;
    #[test]
    fn build_first_frame_result_preserves_encode_errors_and_close_errors() {
        let result = build_first_frame_result(
            Err(anyhow!("编码失败")),
            Err(windows::core::Error::from(E_POINTER)),
        );
        let error = result.unwrap_err();
        assert!(error.contains("编码失败"));
        assert!(error.contains("关闭捕获帧失败"));
    }
    #[test]
    fn finalize_capture_result_merges_cleanup_errors() {
        let result = finalize_capture_result(
            Err(anyhow!("编码失败")),
            Err(windows::core::Error::from(E_POINTER)),
            Ok(()),
            Err(windows::core::Error::from(E_POINTER)),
        );
        let error = result.expect_err("应返回合并后的错误");
        let error_text = error.to_string();
        assert!(error_text.contains("编码失败"));
        assert!(error_text.contains("关闭捕获会话失败"));
        assert!(error_text.contains("关闭捕获帧池失败"));
    }
}
