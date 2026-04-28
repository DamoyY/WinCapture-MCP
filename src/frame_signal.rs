use crate::error::AppResult;
use anyhow::anyhow;
use core::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use std::sync::Mutex;
use tokio::{runtime::Runtime, sync::Notify, time::timeout};
use windows::Win32::Foundation::E_FAIL;
pub(crate) struct FirstFrameSignal {
    notify: Notify,
    result: Mutex<Option<Result<(), String>>>,
    sent: AtomicBool,
}
impl FirstFrameSignal {
    pub(crate) fn new() -> Self {
        Self {
            notify: Notify::new(),
            result: Mutex::new(None),
            sent: AtomicBool::new(false),
        }
    }
    #[cfg(test)]
    fn has_sent(&self) -> bool {
        self.sent.load(Ordering::Acquire)
    }
    pub(crate) fn try_signal(&self, value: Result<(), String>) -> windows::core::Result<()> {
        if self.sent.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let mut result = self
            .result
            .lock()
            .map_err(|error| windows::core::Error::new(E_FAIL, format!("{error}")))?;
        *result = Some(value);
        drop(result);
        self.notify.notify_one();
        Ok(())
    }
    async fn wait(&self) -> AppResult<()> {
        loop {
            if let Some(result) = self.take_result()? {
                return result.map_err(anyhow::Error::msg);
            }
            self.notify.notified().await;
        }
    }
    fn take_result(&self) -> AppResult<Option<Result<(), String>>> {
        let mut result = self
            .result
            .lock()
            .map_err(|error| anyhow!("首帧通知状态互斥锁已中毒: {error}"))?;
        Ok(result.take())
    }
}
pub(crate) fn wait_for_first_frame(
    runtime: &Runtime,
    first_frame_signal: &FirstFrameSignal,
) -> AppResult<()> {
    runtime.block_on(async {
        timeout(Duration::from_secs(1), first_frame_signal.wait())
            .await
            .map_err(|error| anyhow!("等待首帧超时: {error}"))?
    })
}
#[cfg(test)]
mod tests {
    use super::FirstFrameSignal;
    use core::fmt::Display;
    use tokio::time::{Duration, timeout};
    fn must<T, E: Display>(result: Result<T, E>, message: &str) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{message}: {error}"),
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn first_frame_signal_only_delivers_the_first_payload() {
        let first_frame_signal = FirstFrameSignal::new();
        must(first_frame_signal.try_signal(Ok(())), "首次通知应成功");
        must(
            first_frame_signal.try_signal(Err(String::from("ignored"))),
            "重复通知应被忽略",
        );
        let wait_result = must(
            timeout(Duration::from_secs(1), first_frame_signal.wait()).await,
            "等待首帧不应超时",
        );
        must(wait_result, "首次通知应返回成功");
        assert!(first_frame_signal.has_sent());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn first_frame_signal_preserves_the_first_error() {
        let first_frame_signal = FirstFrameSignal::new();
        must(
            first_frame_signal.try_signal(Err(String::from("receiver dropped"))),
            "首次通知应成功",
        );
        must(first_frame_signal.try_signal(Ok(())), "重复通知应被忽略");
        let Err(error) = first_frame_signal.wait().await else {
            panic!("首个错误应被保留");
        };
        assert!(format!("{error}").contains("receiver dropped"));
        assert!(first_frame_signal.has_sent());
    }
}
