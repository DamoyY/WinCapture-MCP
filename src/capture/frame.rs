use crate::capture::{AppResult, device::CaptureDevice};
use anyhow::Context as _;
use windows::{
    Graphics::Capture::Direct3D11CaptureFrame,
    Win32::{
        Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D},
        System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess,
    },
    core::Interface as _,
};
use windows_capture::{
    encoder::{ImageEncoder, ImageEncoderPixelFormat, ImageFormat},
    frame::Frame,
    settings::ColorFormat,
};
pub(super) struct FrameEncoder(ImageEncoder);
impl FrameEncoder {
    pub(super) fn new() -> AppResult<Self> {
        Ok(Self(ImageEncoder::new(
            ImageFormat::Png,
            ImageEncoderPixelFormat::Bgra8,
        )?))
    }
    pub(super) fn encode(
        &self,
        device: &CaptureDevice,
        frame: Direct3D11CaptureFrame,
    ) -> AppResult<Vec<u8>> {
        let frame_for_close = frame.clone();
        let encode_result = self.encode_inner(device, frame);
        let close_result = frame_for_close.Close();
        match (encode_result, close_result) {
            (Ok(png), Ok(())) => Ok(png),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error).context("关闭捕获帧失败"),
            (Err(error), Err(close_error)) => {
                Err(anyhow::anyhow!("{error:#}; 关闭捕获帧失败: {close_error}"))
            }
        }
    }
    fn encode_inner(
        &self,
        device: &CaptureDevice,
        frame: Direct3D11CaptureFrame,
    ) -> AppResult<Vec<u8>> {
        let surface = frame.Surface()?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
        let texture = unsafe { access.GetInterface::<ID3D11Texture2D>() }?;
        let mut description = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            texture.GetDesc(core::ptr::from_mut(&mut description));
        }
        let mut capture_frame = Frame::new(
            frame,
            &device.d3d_device,
            surface,
            texture,
            &device.d3d_context,
            description,
            ColorFormat::Bgra8,
            None,
        );
        let width = capture_frame.width();
        let height = capture_frame.height();
        let frame_buffer = capture_frame.buffer()?;
        let mut contiguous_buffer = Vec::new();
        let pixels = frame_buffer.as_nopadding_buffer(&mut contiguous_buffer);
        self.0
            .encode(pixels, width, height)
            .context("将捕获帧编码为 PNG 失败")
    }
}
