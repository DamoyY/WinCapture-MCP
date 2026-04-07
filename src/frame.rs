extern crate alloc;
use crate::{d3d::CaptureDevice, error::AppResult};
use alloc::sync::Arc;
use anyhow::anyhow;
use core::{
    slice,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use garb::bytes::bgra_to_rgba_strided;
use png::{BitDepth, ColorType, Compression, Encoder};
use std::{io::Cursor, sync::Mutex};
use tokio::{runtime::Runtime, sync::Notify, time::timeout};
use windows::{
    Foundation::TypedEventHandler,
    Graphics::Capture::{Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem},
    Graphics::DirectX::DirectXPixelFormat,
    Win32::{
        Foundation::{E_FAIL, E_POINTER},
        Graphics::Direct3D11::{
            D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
            D3D11_USAGE_STAGING, ID3D11Resource, ID3D11Texture2D,
        },
        System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess,
    },
    core::IInspectable,
};
pub(crate) fn capture_png(
    device: &CaptureDevice,
    item: &GraphicsCaptureItem,
    runtime: &Runtime,
) -> AppResult<Vec<u8>> {
    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &device.direct3d_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        item.Size()?,
    )?;
    let first_frame_signal = Arc::new(FirstFrameSignal::new());
    let callback_signal = Arc::clone(&first_frame_signal);
    let token = frame_pool.FrameArrived(&TypedEventHandler::<
        Direct3D11CaptureFramePool,
        IInspectable,
    >::new(move |incoming_pool, _| {
        if incoming_pool.is_none() {
            callback_signal.try_signal(Err(windows::core::Error::from(E_POINTER).to_string()))?;
            return Ok(());
        }
        callback_signal.try_signal(Ok(()))?;
        Ok(())
    }))?;
    let session = frame_pool.CreateCaptureSession(item)?;
    let capture_result =
        capture_png_inner(device, &frame_pool, &session, &first_frame_signal, runtime);
    let session_close_result = session.Close();
    let remove_handler_result = frame_pool.RemoveFrameArrived(token);
    let frame_pool_close_result = frame_pool.Close();
    finalize_capture_result(
        capture_result,
        session_close_result,
        remove_handler_result,
        frame_pool_close_result,
    )
}
fn capture_png_inner(
    device: &CaptureDevice,
    frame_pool: &Direct3D11CaptureFramePool,
    session: &windows::Graphics::Capture::GraphicsCaptureSession,
    first_frame_signal: &FirstFrameSignal,
    runtime: &Runtime,
) -> AppResult<Vec<u8>> {
    session.StartCapture()?;
    wait_for_first_frame(runtime, first_frame_signal)?;
    let frame = frame_pool.TryGetNextFrame()?;
    build_first_frame_result(encode_frame(device, &frame), frame.Close())
        .map_err(anyhow::Error::msg)
}
fn wait_for_first_frame(runtime: &Runtime, first_frame_signal: &FirstFrameSignal) -> AppResult<()> {
    runtime.block_on(async {
        timeout(Duration::from_secs(1), first_frame_signal.wait())
            .await
            .map_err(|error| anyhow!("等待首帧超时: {error}"))?
    })
}
fn encode_frame(device: &CaptureDevice, frame: &Direct3D11CaptureFrame) -> AppResult<Vec<u8>> {
    device.with_multithread_lock(|| encode_frame_locked(device, frame))
}
fn encode_frame_locked(
    device: &CaptureDevice,
    frame: &Direct3D11CaptureFrame,
) -> AppResult<Vec<u8>> {
    let width =
        anyhow::Context::context(u32::try_from(frame.ContentSize()?.Width), "窗口宽度无效")?;
    let height =
        anyhow::Context::context(u32::try_from(frame.ContentSize()?.Height), "窗口高度无效")?;
    let surface = frame.Surface()?;
    let access: IDirect3DDxgiInterfaceAccess = windows::core::Interface::cast(&surface)?;
    let texture = unsafe { access.GetInterface::<ID3D11Texture2D>() }?;
    let staging = create_staging_texture(&device.d3d_device, &texture)?;
    let texture_resource: ID3D11Resource = windows::core::Interface::cast(&texture)?;
    let staging_resource: ID3D11Resource = windows::core::Interface::cast(&staging)?;
    unsafe {
        device
            .d3d_context
            .CopyResource(Some(&staging_resource), Some(&texture_resource));
    }
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe {
        device.d3d_context.Map(
            Some(&staging_resource),
            0,
            D3D11_MAP_READ,
            0,
            Some(core::ptr::from_mut(&mut mapped)),
        )?;
    }
    let row_pitch = anyhow::Context::context(usize::try_from(mapped.RowPitch), "行步长无效")?;
    let height_usize = anyhow::Context::context(usize::try_from(height), "窗口高度无效")?;
    let mapped_len =
        anyhow::Context::context(row_pitch.checked_mul(height_usize), "帧缓冲区长度溢出")?;
    let bytes = unsafe { slice::from_raw_parts(mapped.pData.cast::<u8>(), mapped_len) };
    let rgba_result = convert_bgra_frame(bytes, row_pitch, width, height);
    unsafe {
        device.d3d_context.Unmap(Some(&staging_resource), 0);
    }
    let rgba = rgba_result?;
    let mut output = Cursor::new(Vec::new());
    let mut encoder = Encoder::new(&mut output, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_compression(Compression::Fastest);
    {
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&rgba)?;
        writer.finish()?;
    }
    Ok(output.into_inner())
}
fn create_staging_texture(
    device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
    texture: &ID3D11Texture2D,
) -> AppResult<ID3D11Texture2D> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe {
        texture.GetDesc(core::ptr::from_mut(&mut desc));
    }
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.CPUAccessFlags =
        anyhow::Context::context(u32::try_from(D3D11_CPU_ACCESS_READ.0), "CPU 访问标志无效")?;
    desc.MiscFlags = 0;
    let mut staging = None;
    unsafe {
        device.CreateTexture2D(
            core::ptr::from_ref(&desc),
            None,
            Some(core::ptr::from_mut(&mut staging)),
        )?;
    }
    anyhow::Context::context(staging, "创建 staging 纹理失败")
}
fn convert_bgra_frame(
    source: &[u8],
    row_pitch: usize,
    width: u32,
    height: u32,
) -> AppResult<Vec<u8>> {
    let width_usize = anyhow::Context::context(usize::try_from(width), "窗口宽度无效")?;
    let height_usize = anyhow::Context::context(usize::try_from(height), "窗口高度无效")?;
    let pixel_row_len = anyhow::Context::context(width_usize.checked_mul(4), "像素行长度溢出")?;
    let rgba_len = anyhow::Context::context(
        pixel_row_len.checked_mul(height_usize),
        "RGBA 缓冲区长度溢出",
    )?;
    let mut rgba = vec![0; rgba_len];
    anyhow::Context::context(
        bgra_to_rgba_strided(
            source,
            &mut rgba,
            width_usize,
            height_usize,
            row_pitch,
            pixel_row_len,
        ),
        "BGRA 转 RGBA 失败",
    )?;
    Ok(rgba)
}
fn build_first_frame_result(
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
fn finalize_capture_result(
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
struct FirstFrameSignal {
    notify: Notify,
    result: Mutex<Option<Result<(), String>>>,
    sent: AtomicBool,
}
impl FirstFrameSignal {
    fn new() -> Self {
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
    fn try_signal(&self, value: Result<(), String>) -> windows::core::Result<()> {
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
#[cfg(test)]
mod tests {
    use super::{FirstFrameSignal, build_first_frame_result, finalize_capture_result};
    use anyhow::anyhow;
    use tokio::time::{Duration, timeout};
    use windows::Win32::Foundation::E_POINTER;
    #[tokio::test(flavor = "current_thread")]
    async fn first_frame_signal_only_delivers_the_first_payload() {
        let first_frame_signal = FirstFrameSignal::new();
        first_frame_signal
            .try_signal(Ok(()))
            .expect("首次通知应成功");
        first_frame_signal
            .try_signal(Err("ignored".to_string()))
            .expect("重复通知应被忽略");
        timeout(Duration::from_secs(1), first_frame_signal.wait())
            .await
            .expect("等待首帧不应超时")
            .expect("首次通知应返回成功");
        assert!(first_frame_signal.has_sent());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn first_frame_signal_preserves_the_first_error() {
        let first_frame_signal = FirstFrameSignal::new();
        first_frame_signal
            .try_signal(Err("receiver dropped".to_string()))
            .expect("首次通知应成功");
        first_frame_signal
            .try_signal(Ok(()))
            .expect("重复通知应被忽略");
        let error = first_frame_signal
            .wait()
            .await
            .expect_err("首个错误应被保留");
        assert!(error.to_string().contains("receiver dropped"));
        assert!(first_frame_signal.has_sent());
    }
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
