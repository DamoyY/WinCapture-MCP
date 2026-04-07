use crate::{d3d::CaptureDevice, error::AppResult};
use anyhow::anyhow;
use core::{slice, time::Duration};
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
    let (sender, receiver) = mpsc::channel();
    let token = frame_pool.FrameArrived(&TypedEventHandler::<
        Direct3D11CaptureFramePool,
        IInspectable,
    >::new(move |incoming_pool, _| {
        let Some(arrived_frame_pool) = incoming_pool.as_ref() else {
            return Err(windows::core::Error::from(E_POINTER));
        };
        let frame = arrived_frame_pool.TryGetNextFrame()?;
        sender.send(frame).map_err(|error| {
            windows::core::Error::new(E_POINTER, format!("发送首帧失败: {error}"))
        })?;
        Ok(())
    }))?;
    let session = frame_pool.CreateCaptureSession(item)?;
    let result = (|| -> AppResult<Vec<u8>> {
        session.StartCapture()?;
        let frame = receiver
            .recv_timeout(Duration::from_secs(1))
            .map_err(|error| anyhow!("等待首帧失败: {error}"))?;
        encode_frame(device, &frame)
    })();
    frame_pool.RemoveFrameArrived(token)?;
    session.Close()?;
    frame_pool.Close()?;
    result
}
fn encode_frame(device: &CaptureDevice, frame: &Direct3D11CaptureFrame) -> AppResult<Vec<u8>> {
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
    let rgba = bgra_to_rgba(bytes, row_pitch, width, height)?;
    unsafe {
        device.d3d_context.Unmap(Some(&staging_resource), 0);
    }
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
fn bgra_to_rgba(source: &[u8], row_pitch: usize, width: u32, height: u32) -> AppResult<Vec<u8>> {
    let width_usize = anyhow::Context::context(usize::try_from(width), "窗口宽度无效")?;
    let height_usize = anyhow::Context::context(usize::try_from(height), "窗口高度无效")?;
    let pixel_row_len = anyhow::Context::context(width_usize.checked_mul(4), "像素行长度溢出")?;
    if row_pitch < pixel_row_len {
        return Err(anyhow!("行步长小于像素行长度"));
    }
    let rgba_len = anyhow::Context::context(
        pixel_row_len.checked_mul(height_usize),
        "RGBA 缓冲区长度溢出",
    )?;
    let expected_source_len =
        anyhow::Context::context(row_pitch.checked_mul(height_usize), "源缓冲区长度溢出")?;
    if source.len() < expected_source_len {
        return Err(anyhow!("源缓冲区长度不足"));
    }
    let mut rgba = Vec::with_capacity(rgba_len);
    for src_row in source.chunks_exact(row_pitch).take(height_usize) {
        let pixel_bytes =
            anyhow::Context::context(src_row.get(..pixel_row_len), "像素行长度超出缓冲区")?;
        for bgra in pixel_bytes.chunks_exact(4) {
            let [blue, green, red, alpha] =
                *anyhow::Context::context(<&[u8; 4]>::try_from(bgra), "像素块长度无效")?;
            rgba.extend_from_slice(&[red, green, blue, alpha]);
        }
    }
    if rgba.len() != rgba_len {
        return Err(anyhow!("RGBA 缓冲区长度不匹配"));
    }
    Ok(rgba)
}
