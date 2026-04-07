use crate::{
    capture_item::create_capture_item, d3d::create_capture_device, error::AppResult,
    frame::capture_png,
};
use anyhow::anyhow;
use windows::{
    Graphics::Capture::GraphicsCaptureSession,
    Win32::{
        Foundation::{HWND, RPC_E_CHANGED_MODE},
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
    },
};
pub(crate) fn capture_window_png(hwnd: HWND) -> AppResult<Vec<u8>> {
    let _com = initialize_com()?;
    if !GraphicsCaptureSession::IsSupported()? {
        return Err(anyhow!("当前系统不支持 Windows.Graphics.Capture"));
    }
    let item = create_capture_item(hwnd)?;
    let device = create_capture_device()?;
    capture_png(&device, &item)
}
fn initialize_com() -> AppResult<Option<ComGuard>> {
    match unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() } {
        Ok(()) => Ok(Some(ComGuard)),
        Err(error) if error.code() == RPC_E_CHANGED_MODE => Ok(None),
        Err(error) => Err(error.into()),
    }
}
struct ComGuard;
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
