use crate::{DeviceInfo, DeviceError};

#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use io_kit_sys::{IOServiceMatching, IOServiceGetMatchingServices, kIOMasterPortDefault};

pub fn enumerate() -> Result<Vec<DeviceInfo>, DeviceError> {
    // In a full implementation, we'd use IOServiceMatching("IOMedia")
    // and iterate through the registry dictionary to find BSD Name (e.g. disk0),
    // Size, and Device Characteristics.

    // Dummy implementation for MVP skeleton
    let devices = vec![
        DeviceInfo {
            path: "/dev/disk0".into(),
            model: "Apple AP0512M".into(),
            serial: "MAC-001".into(),
            firmware: "1.0".into(),
            capacity_bytes: 512 * 1024 * 1024 * 1024,
            is_nvme: true,
            is_sata: false,
            can_crypto_erase: false, // Apple silicon handles this via guided erase
            can_block_erase: false,
        }
    ];

    Ok(devices)
}
