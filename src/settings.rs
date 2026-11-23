//! Preferences saved under `HKEY_CURRENT_USER\Software\Rozkladka`.

use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD, RegGetValueW, RegSetKeyValueW,
};
use windows::core::{PCWSTR, w};

const KEY: PCWSTR = w!(r"Software\Rozkladka");
const AUTO: PCWSTR = w!("Auto");

/// Automatic mode; on unless the person turned it off.
pub fn auto() -> bool {
    let mut value = 1_u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: The data pointer and size describe one DWORD.
    let _ = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            KEY,
            AUTO,
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut size),
        )
    };
    value != 0
}

pub fn set_auto(enabled: bool) {
    let value = u32::from(enabled);
    // SAFETY: The data pointer and size describe one DWORD.
    let _ = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            KEY,
            AUTO,
            REG_DWORD.0,
            Some((&raw const value).cast()),
            std::mem::size_of::<u32>() as u32,
        )
    };
}
