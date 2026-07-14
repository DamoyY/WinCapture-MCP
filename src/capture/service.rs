use crate::capture::{AppResult, CaptureContext};
use anyhow::anyhow;
use std::{
    sync::{OnceLock, mpsc},
    thread,
};
use tokio::sync::oneshot;
static CAPTURE_SERVICE: OnceLock<Result<CaptureService, String>> = OnceLock::new();
pub(crate) async fn capture_window_png(hwnd: String) -> AppResult<Vec<u8>> {
    capture_service()?.capture_window_png(hwnd).await
}
fn capture_service() -> AppResult<&'static CaptureService> {
    let service_result =
        CAPTURE_SERVICE.get_or_init(|| CaptureService::start().map_err(|error| error.to_string()));
    service_result
        .as_ref()
        .map_err(|error| anyhow!(error.clone()))
}
struct CaptureService {
    sender: mpsc::Sender<CaptureRequest>,
}
impl CaptureService {
    fn start() -> AppResult<Self> {
        let (sender, receiver) = mpsc::channel();
        let (init_sender, init_receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name(String::from("capture-service"))
            .spawn(move || run_worker(&receiver, &init_sender))
            .context("创建截图服务线程失败")?;
        init_receiver
            .recv()
            .context("等待截图服务初始化结果失败")?
            .map_err(anyhow::Error::msg)?;
        Ok(Self { sender })
    }
    async fn capture_window_png(&self, hwnd: String) -> AppResult<Vec<u8>> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.sender
            .send(CaptureRequest {
                hwnd,
                response_sender,
            })
            .map_err(|error| anyhow!("向截图服务发送任务失败: {error}"))?;
        response_receiver
            .await
            .map_err(|error| anyhow!("截图服务在线程返回结果前已停止: {error}"))?
    }
}
struct CaptureRequest {
    hwnd: String,
    response_sender: oneshot::Sender<AppResult<Vec<u8>>>,
}
fn run_worker(
    receiver: &mpsc::Receiver<CaptureRequest>,
    init_sender: &mpsc::SyncSender<Result<(), String>>,
) {
    let capture_context = match CaptureContext::new() {
        Ok(context) => context,
        Err(error) => {
            let error_message = format!("{error:#}");
            tracing::error!("{error_message}");
            if init_sender.send(Err(error_message)).is_err() {
                tracing::error!("截图服务初始化失败结果无人接收");
            }
            return;
        }
    };
    if init_sender.send(Ok(())).is_err() {
        tracing::error!("截图服务初始化结果无人接收");
        return;
    }
    for request in receiver {
        let result = capture_context.capture_window_png(&request.hwnd);
        if request.response_sender.send(result).is_err() {
            tracing::error!("截图服务结果接收端已提前关闭");
        }
    }
}
use anyhow::Context as _;
