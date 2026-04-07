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
use std::{io::Cursor, sync::mpsc};
use windows::{
    Foundation::TypedEventHandler,
    Graphics::Capture::{Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem},
    Graphics::DirectX::DirectXPixelFormat,
    Win32::{
        Foundation::E_POINTER,
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
) -> AppResult<Vec<u8>> {
    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &device.direct3d_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        item.Size()?,
    )?;
    let (sender, receiver) = mpsc::channel::<()>();
    let first_frame_sender = FirstFrameSender::new(sender);
    let token = frame_pool.FrameArrived(&TypedEventHandler::<
        Direct3D11CaptureFramePool,
        IInspectable,
    >::new(move |incoming_pool, _| {
        if incoming_pool.is_none() {
            return Err(windows::core::Error::from(E_POINTER));
        }
        if first_frame_sender.has_sent() {
            return Ok(());
        }
        first_frame_sender.try_send(());
        Ok(())
    }))?;
    let session = frame_pool.CreateCaptureSession(item)?;
    let result = (|| -> AppResult<Vec<u8>> {
        session.StartCapture()?;
        receiver
            .recv_timeout(Duration::from_secs(1))
            .map_err(|error| anyhow!("等待首帧失败: {error}"))?;
        let frame = frame_pool.TryGetNextFrame()?;
        build_first_frame_result(encode_frame(device, &frame), frame.Close())
            .map_err(anyhow::Error::msg)
    })();
    session.Close()?;
    frame_pool.RemoveFrameArrived(token)?;
    frame_pool.Close()?;
    result
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
#[derive(Clone)]
struct FirstFrameSender<T> {
    sender: mpsc::Sender<T>,
    sent: Arc<AtomicBool>,
}
impl<T> FirstFrameSender<T> {
    fn new(sender: mpsc::Sender<T>) -> Self {
        Self {
            sender,
            sent: Arc::new(AtomicBool::new(false)),
        }
    }
    fn has_sent(&self) -> bool {
        self.sent.load(Ordering::Acquire)
    }
    fn try_send(&self, value: T) {
        if self.sent.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Err(error) = self.sender.send(value) {
            tracing::error!("发送首帧结果失败: {error}");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{FirstFrameSender, build_first_frame_result};
    use anyhow::anyhow;
    use std::sync::mpsc;
    use windows::Win32::Foundation::E_POINTER;
    #[test]
    fn first_frame_sender_only_delivers_the_first_payload() {
        let (sender, receiver) = mpsc::channel();
        let first_frame_sender = FirstFrameSender::new(sender);
        first_frame_sender.try_send(Ok::<Vec<u8>, String>(vec![1, 2, 3]));
        first_frame_sender.try_send(Ok::<Vec<u8>, String>(vec![4, 5, 6]));
        assert_eq!(receiver.recv().unwrap().unwrap(), vec![1, 2, 3]);
        assert!(receiver.try_recv().is_err());
        assert!(first_frame_sender.has_sent());
    }
    #[test]
    fn first_frame_sender_tolerates_a_dropped_receiver() {
        let (sender, receiver) = mpsc::channel::<Result<Vec<u8>, String>>();
        drop(receiver);
        let first_frame_sender = FirstFrameSender::new(sender);
        first_frame_sender.try_send(Err("receiver dropped".to_string()));
        first_frame_sender.try_send(Ok(vec![7, 8, 9]));
        assert!(first_frame_sender.has_sent());
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
}
