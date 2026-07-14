use crate::capture::AppResult;
use anyhow::anyhow;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureSession};
pub(super) struct CaptureResources {
    frame_pool: Direct3D11CaptureFramePool,
    handler_token: Option<i64>,
    session: Option<GraphicsCaptureSession>,
    closed: bool,
}
impl CaptureResources {
    pub(super) const fn new(frame_pool: Direct3D11CaptureFramePool) -> Self {
        Self {
            frame_pool,
            handler_token: None,
            session: None,
            closed: false,
        }
    }
    pub(super) const fn frame_pool(&self) -> &Direct3D11CaptureFramePool {
        &self.frame_pool
    }
    pub(super) fn session(&self) -> AppResult<&GraphicsCaptureSession> {
        self.session
            .as_ref()
            .ok_or_else(|| anyhow!("捕获会话尚未初始化"))
    }
    pub(super) const fn set_handler_token(&mut self, token: i64) {
        self.handler_token = Some(token);
    }
    pub(super) fn set_session(&mut self, session: GraphicsCaptureSession) {
        self.session = Some(session);
    }
    pub(super) fn finish(mut self, capture_result: AppResult<Vec<u8>>) -> AppResult<Vec<u8>> {
        let cleanup_errors = self.close();
        merge_capture_result(capture_result, &cleanup_errors)
    }
    fn close(&mut self) -> Vec<String> {
        if self.closed {
            return Vec::new();
        }
        let mut errors = Vec::new();
        if let Some(session) = self.session.take()
            && let Err(error) = session.Close()
        {
            errors.push(format!("关闭捕获会话失败: {error}"));
        }
        if let Some(token) = self.handler_token.take()
            && let Err(error) = self.frame_pool.RemoveFrameArrived(token)
        {
            errors.push(format!("移除帧到达回调失败: {error}"));
        }
        if let Err(error) = self.frame_pool.Close() {
            errors.push(format!("关闭捕获帧池失败: {error}"));
        }
        self.closed = true;
        errors
    }
}
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop 是编译器内部提供的默认方法，普通 Drop 类型只实现 drop"
)]
impl Drop for CaptureResources {
    fn drop(&mut self) {
        for error in self.close() {
            tracing::error!("{error}");
        }
    }
}
fn merge_capture_result(
    capture_result: AppResult<Vec<u8>>,
    cleanup_errors: &[String],
) -> AppResult<Vec<u8>> {
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
    use super::merge_capture_result;
    use anyhow::anyhow;
    #[test]
    fn cleanup_errors_are_not_lost_after_capture_failure() {
        let result = merge_capture_result(
            Err(anyhow!("编码失败")),
            &[String::from("关闭会话失败"), String::from("关闭帧池失败")],
        );
        let Err(error) = result else {
            panic!("清理失败时应返回错误");
        };
        let message = error.to_string();
        assert!(message.contains("编码失败"));
        assert!(message.contains("关闭会话失败"));
        assert!(message.contains("关闭帧池失败"));
    }
}
