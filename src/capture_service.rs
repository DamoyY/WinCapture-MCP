use crate::{capture::CaptureContext, error::AppResult, hwnd::hwnd_from_value};
use anyhow::anyhow;
use std::{
    sync::{OnceLock, mpsc},
    thread,
};
use tokio::{
    runtime::{Builder, Runtime},
    sync::oneshot,
};
static CAPTURE_SERVICE: OnceLock<Result<CaptureService, String>> = OnceLock::new();
pub(crate) async fn capture_window_png(hwnd_value: usize) -> AppResult<Vec<u8>> {
    capture_service()?.capture_window_png(hwnd_value).await
}
pub(crate) fn warmup_capture_service() -> AppResult<()> {
    capture_service()?;
    Ok(())
}
fn capture_service() -> AppResult<&'static CaptureService> {
    let service_result =
        CAPTURE_SERVICE.get_or_init(|| CaptureService::start().map_err(|error| error.to_string()));
    match service_result.as_ref() {
        Ok(service) => Ok(service),
        Err(error) => Err(anyhow!("{error}")),
    }
}
struct CaptureService {
    sender: mpsc::Sender<CaptureRequest>,
}
impl CaptureService {
    fn start() -> AppResult<Self> {
        let (sender, receiver) = mpsc::channel();
        let (init_sender, init_receiver) = mpsc::sync_channel(1);
        let spawn_result = thread::Builder::new()
            .name("capture-service".into())
            .spawn(move || {
                let init_result = CaptureWorker::new(receiver);
                match init_result {
                    Ok(worker) => {
                        if init_sender.send(Ok(())).is_err() {
                            tracing::error!("截图服务初始化结果无人接收");
                            return;
                        }
                        worker.run();
                    }
                    Err(error) => {
                        tracing::error!("{error:#}");
                        if init_sender.send(Err(error.to_string())).is_err() {
                            tracing::error!("截图服务初始化失败结果无人接收");
                        }
                    }
                }
            });
        anyhow::Context::context(spawn_result, "创建截图服务线程失败")?;
        init_receiver
            .recv()
            .map_err(|error| anyhow!("等待截图服务初始化结果失败: {error}"))?
            .map_err(anyhow::Error::msg)?;
        Ok(Self { sender })
    }
    async fn capture_window_png(&self, hwnd_value: usize) -> AppResult<Vec<u8>> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.sender
            .send(CaptureRequest {
                hwnd_value,
                response_sender,
            })
            .map_err(|error| anyhow!("向截图服务发送任务失败: {error}"))?;
        response_receiver
            .await
            .map_err(|error| anyhow!("截图服务在线程返回结果前已停止: {error}"))?
    }
}
struct CaptureRequest {
    hwnd_value: usize,
    response_sender: oneshot::Sender<AppResult<Vec<u8>>>,
}
struct CaptureWorker {
    capture_context: CaptureContext,
    receiver: mpsc::Receiver<CaptureRequest>,
    runtime: Runtime,
}
impl CaptureWorker {
    fn new(receiver: mpsc::Receiver<CaptureRequest>) -> AppResult<Self> {
        let runtime = Builder::new_current_thread()
            .enable_time()
            .build()
            .map_err(|error| anyhow!("创建截图服务运行时失败: {error}"))?;
        let capture_context = CaptureContext::new()?;
        Ok(Self {
            capture_context,
            receiver,
            runtime,
        })
    }
    fn run(self) {
        let Self {
            capture_context,
            receiver,
            runtime,
        } = self;
        for request in receiver {
            let result =
                capture_context.capture_window_png(&runtime, hwnd_from_value(request.hwnd_value));
            if request.response_sender.send(result).is_err() {
                tracing::error!("截图服务结果接收端已提前关闭");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::warmup_capture_service;
    use crate::error::AppResult;
    #[test]
    fn warmup_capture_service_is_idempotent() -> AppResult<()> {
        warmup_capture_service()?;
        warmup_capture_service()
    }
}
