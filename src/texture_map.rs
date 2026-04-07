use crate::{d3d::CaptureDevice, error::AppResult};
use core::slice;
use garb::bytes::bgra_to_rgba_strided;
use png::{BitDepth, ColorType, Compression, Encoder};
use std::io::Cursor;
use windows::{
    Graphics::Capture::Direct3D11CaptureFrame,
    Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
        D3D11_USAGE_STAGING, ID3D11Resource, ID3D11Texture2D,
    },
    Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess,
};
pub(crate) fn encode_frame(
    device: &CaptureDevice,
    frame: &Direct3D11CaptureFrame,
) -> AppResult<Vec<u8>> {
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
    encode_png(&rgba, width, height)
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
fn encode_png(rgba: &[u8], width: u32, height: u32) -> AppResult<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    let mut encoder = Encoder::new(&mut output, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_compression(Compression::Fastest);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    Ok(output.into_inner())
}
