use crate::error::AppResult;
use anyhow::anyhow;
use windows::Win32::Foundation::HWND;
pub(crate) fn parse_hwnd(raw: &str) -> AppResult<HWND> {
    let value = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        anyhow::Context::with_context(usize::from_str_radix(hex, 16), || {
            format!("无效的 HWND: {raw}")
        })?
    } else {
        anyhow::Context::with_context(raw.parse::<usize>(), || format!("无效的 HWND: {raw}"))?
    };
    if value == 0 {
        return Err(anyhow!("HWND 不能为 0"));
    }
    Ok(HWND(core::ptr::with_exposed_provenance_mut::<
        core::ffi::c_void,
    >(value)))
}
pub(crate) fn format_hwnd(hwnd: HWND) -> String {
    format!("0x{:016X}", hwnd.0.addr())
}
#[cfg(test)]
mod tests {
    use super::{format_hwnd, parse_hwnd};
    #[test]
    fn parses_hex_hwnd() {
        let hwnd = parse_hwnd("0x2A").expect("hex should parse");
        assert_eq!(format_hwnd(hwnd), "0x000000000000002A");
    }
    #[test]
    fn parses_decimal_hwnd() {
        let hwnd = parse_hwnd("42").expect("decimal should parse");
        assert_eq!(format_hwnd(hwnd), "0x000000000000002A");
    }
}
