use crate::capture::AppResult;
use windows::{
    Graphics::DirectX::Direct3D11::IDirect3DDevice,
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device, ID3D11DeviceContext,
            },
            Dxgi::IDXGIDevice,
        },
        System::{
            Com::{CO_MTA_USAGE_COOKIE, CoDecrementMTAUsage, CoIncrementMTAUsage},
            WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice,
        },
    },
    core::Interface as _,
};
pub(super) struct CaptureDevice {
    pub(super) d3d_device: ID3D11Device,
    pub(super) d3d_context: ID3D11DeviceContext,
    direct3d_device: IDirect3DDevice,
    _mta_usage: MtaUsage,
}
impl CaptureDevice {
    pub(super) fn new() -> AppResult<Self> {
        let mta_usage = MtaUsage::new()?;
        let (d3d_device, d3d_context) = create_warp_device()?;
        let dxgi_device: IDXGIDevice = d3d_device.cast()?;
        let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }?;
        let direct3d_device = inspectable.cast()?;
        Ok(Self {
            d3d_device,
            d3d_context,
            direct3d_device,
            _mta_usage: mta_usage,
        })
    }
    pub(super) const fn direct3d_device(&self) -> &IDirect3DDevice {
        &self.direct3d_device
    }
}
struct MtaUsage(CO_MTA_USAGE_COOKIE);
impl MtaUsage {
    fn new() -> AppResult<Self> {
        Ok(Self(unsafe { CoIncrementMTAUsage() }?))
    }
}
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop 是编译器内部提供的默认方法，普通 Drop 类型只实现 drop"
)]
impl Drop for MtaUsage {
    fn drop(&mut self) {
        if let Err(error) = unsafe { CoDecrementMTAUsage(self.0) } {
            tracing::error!("释放 MTA 使用计数失败: {error}");
        }
    }
}
fn create_warp_device() -> AppResult<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(core::ptr::from_mut(&mut device)),
            None,
            Some(core::ptr::from_mut(&mut context)),
        )?;
    }
    Ok((
        anyhow::Context::context(device, "WARP D3D11 设备为空")?,
        anyhow::Context::context(context, "WARP D3D11 上下文为空")?,
    ))
}
#[cfg(test)]
mod tests {
    use super::CaptureDevice;
    use crate::capture::AppResult;
    use windows::{Win32::Graphics::Dxgi::IDXGIDevice, core::Interface as _};
    #[test]
    fn capture_device_uses_microsoft_warp_adapter() {
        let vendor_id_result = warp_vendor_id();
        let Ok(vendor_id) = vendor_id_result else {
            panic!("应能创建 WARP 捕获设备: {vendor_id_result:?}");
        };
        assert_eq!(vendor_id, 0x1414);
    }
    fn warp_vendor_id() -> AppResult<u32> {
        let device = CaptureDevice::new()?;
        let dxgi_device: IDXGIDevice = device.d3d_device.cast()?;
        let adapter = unsafe { dxgi_device.GetAdapter() }?;
        Ok(unsafe { adapter.GetDesc() }?.VendorId)
    }
}
