mod device;
mod frame;
mod resources;
mod service;
use crate::{capture::resources::CaptureResources, window::parse_hwnd};
use anyhow::{Context as _, anyhow};
use core::time::Duration;
use device::CaptureDevice;
use frame::FrameEncoder;
pub(crate) use service::capture_window_png;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::DirectXPixelFormat,
    },
    Win32::{Foundation::E_POINTER, System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop},
    core::{IInspectable, factory},
};
pub(crate) type AppResult<T> = anyhow::Result<T>;
struct CaptureContext {
    device: CaptureDevice,
    encoder: FrameEncoder,
}
impl CaptureContext {
    fn new() -> AppResult<Self> {
        let device = CaptureDevice::new()?;
        if !GraphicsCaptureSession::IsSupported()? {
            return Err(anyhow!("当前系统不支持 Windows.Graphics.Capture"));
        }
        Ok(Self {
            device,
            encoder: FrameEncoder::new()?,
        })
    }
    fn capture_window_png(&self, hwnd_text: &str) -> AppResult<Vec<u8>> {
        let hwnd = parse_hwnd(hwnd_text)?;
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item = unsafe { interop.CreateForWindow(hwnd) }
            .with_context(|| format!("无法为 HWND {hwnd_text} 创建捕获目标"))?;
        capture_png(&self.device, &self.encoder, &item)
    }
}
fn capture_png(
    device: &CaptureDevice,
    encoder: &FrameEncoder,
    item: &GraphicsCaptureItem,
) -> AppResult<Vec<u8>> {
    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        device.direct3d_device(),
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        item.Size()?,
    )?;
    let mut resources = CaptureResources::new(frame_pool);
    let (signal_sender, signal_receiver) = sync_channel(1);
    let token = resources
        .frame_pool()
        .FrameArrived(&frame_handler(signal_sender))?;
    resources.set_handler_token(token);
    let session = resources.frame_pool().CreateCaptureSession(item)?;
    resources.set_session(session);
    let capture_result = capture_first_frame(device, encoder, &resources, &signal_receiver);
    resources.finish(capture_result)
}
fn frame_handler(
    signal_sender: SyncSender<Result<(), String>>,
) -> TypedEventHandler<Direct3D11CaptureFramePool, IInspectable> {
    TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |incoming_pool, _| {
        let signal = if incoming_pool.is_none() {
            Err(windows::core::Error::from(E_POINTER).to_string())
        } else {
            Ok(())
        };
        match signal_sender.try_send(signal) {
            Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
            Err(TrySendError::Disconnected(_)) => {
                Err(windows::core::Error::new(E_POINTER, "首帧通知接收端已关闭"))
            }
        }
    })
}
fn capture_first_frame(
    device: &CaptureDevice,
    encoder: &FrameEncoder,
    resources: &CaptureResources,
    signal_receiver: &Receiver<Result<(), String>>,
) -> AppResult<Vec<u8>> {
    resources.session()?.StartCapture()?;
    let signal = signal_receiver
        .recv_timeout(Duration::from_secs(2))
        .context("等待首帧超时")?;
    signal.map_err(anyhow::Error::msg)?;
    let frame = resources.frame_pool().TryGetNextFrame()?;
    encoder.encode(device, frame)
}
