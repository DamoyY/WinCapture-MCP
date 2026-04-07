use crate::error::AppResult;
use windows::{
    Graphics::DirectX::Direct3D11::IDirect3DDevice,
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{
                D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL,
                D3D_FEATURE_LEVEL_11_0,
            },
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device, ID3D11DeviceContext,
            },
            Dxgi::IDXGIDevice,
        },
        System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice,
    },
};
#[derive(Clone)]
pub(crate) struct CaptureDevice {
    pub(crate) d3d_device: ID3D11Device,
    pub(crate) d3d_context: ID3D11DeviceContext,
    pub(crate) direct3d_device: IDirect3DDevice,
}
pub(crate) fn create_capture_device() -> AppResult<CaptureDevice> {
    let (d3d_device, _, d3d_context) = create_raw_device(D3D_DRIVER_TYPE_HARDWARE)
        .or_else(|_| create_raw_device(D3D_DRIVER_TYPE_WARP))?;
    let dxgi_device: IDXGIDevice = windows::core::Interface::cast(&d3d_device)?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }?;
    let direct3d_device = windows::core::Interface::cast(&inspectable)?;
    Ok(CaptureDevice {
        d3d_device,
        d3d_context,
        direct3d_device,
    })
}
fn create_raw_device(
    driver_type: D3D_DRIVER_TYPE,
) -> AppResult<(ID3D11Device, D3D_FEATURE_LEVEL, ID3D11DeviceContext)> {
    let mut device = None;
    let mut context = None;
    let mut feature_level = D3D_FEATURE_LEVEL_11_0;
    unsafe {
        D3D11CreateDevice(
            None,
            driver_type,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(core::ptr::from_mut(&mut device)),
            Some(core::ptr::from_mut(&mut feature_level)),
            Some(core::ptr::from_mut(&mut context)),
        )?;
    }
    Ok((
        anyhow::Context::context(device, "D3D11 设备为空")?,
        feature_level,
        anyhow::Context::context(context, "D3D11 上下文为空")?,
    ))
}
