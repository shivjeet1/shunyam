use crate::{DeviceInfo, DeviceError};

pub fn enumerate() -> Result<Vec<DeviceInfo>, DeviceError> {
    // Linux uses nvme-cli, hdparm natively via Go daemon for detailed wipe capability probing.
    let mut devices = Vec::new();

    if let Ok(entries) = std::fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Filter out loopback and ram devices
            if name.starts_with("loop") || name.starts_with("ram") {
                continue;
            }

            devices.push(DeviceInfo {
                path: format!("/dev/{}", name),
                model: "Linux Block Device".into(),
                serial: "unknown".into(),
                firmware: "unknown".into(),
                capacity_bytes: 0,
                is_nvme: name.starts_with("nvme"),
                is_sata: name.starts_with("sd"),
                can_crypto_erase: false,
                can_block_erase: false,
            });
        }
    }

    Ok(devices)
}
