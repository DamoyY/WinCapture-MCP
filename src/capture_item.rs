use crate::error::AppResult;
use windows::{
    Graphics::Capture::GraphicsCaptureItem,
    Win32::{Foundation::HWND, System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop},
    core::factory,
};
pub(crate) fn create_capture_item(hwnd: HWND) -> AppResult<GraphicsCaptureItem> {
    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    let item = unsafe { interop.CreateForWindow(hwnd) }?;
    Ok(item)
}
