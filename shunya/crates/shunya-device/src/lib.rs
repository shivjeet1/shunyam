#[derive(Debug)]
pub struct DeviceInfo {
    pub path: String,
    pub model: String,
    pub serial: String,
    pub firmware: String,
    pub capacity_bytes: u64,
    pub is_nvme: bool,
    pub is_sata: bool,
    pub can_crypto_erase: bool,
    pub can_block_erase: bool,
}

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("OS error: {0}")]
    OsError(String),
    #[error("Not supported on this OS")]
    NotSupported,
}

/// Discovers all available storage devices and their sanitization capabilities.
pub fn enumerate_devices() -> Result<Vec<DeviceInfo>, DeviceError> {
    #[cfg(target_os = "linux")]
    return linux::enumerate();

    #[cfg(target_os = "windows")]
    return windows::enumerate();

    #[cfg(target_os = "macos")]
    return macos::enumerate();

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    return Err(DeviceError::NotSupported);
}
