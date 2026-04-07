use crate::{
    cleanup::{build_first_frame_result, finalize_capture_result},
    d3d::CaptureDevice,
    error::AppResult,
    frame_signal::{FirstFrameSignal, wait_for_first_frame},
    texture_map::encode_frame,
};
use alloc::sync::Arc;
use windows::{
    Foundation::TypedEventHandler,
    Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem},
    Graphics::DirectX::DirectXPixelFormat,
    Win32::Foundation::E_POINTER,
    core::IInspectable,
};
pub(crate) fn capture_png(
    device: &CaptureDevice,
    item: &GraphicsCaptureItem,
    runtime: &tokio::runtime::Runtime,
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
    runtime: &tokio::runtime::Runtime,
) -> AppResult<Vec<u8>> {
    session.StartCapture()?;
    wait_for_first_frame(runtime, first_frame_signal)?;
    let frame = frame_pool.TryGetNextFrame()?;
    build_first_frame_result(encode_frame(device, &frame), frame.Close())
        .map_err(anyhow::Error::msg)
}
