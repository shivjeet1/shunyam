use std::process::Command;
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum MacOsError {
    #[error("Failed to execute command: {0}")]
    ExecutionFailed(#[from] io::Error),
    #[error("System Integrity Protection (SIP) is enabled. Cannot wipe internal drives. Disable SIP in recovery mode.")]
    SipEnabled,
    #[error("Failed to unmount device {0}: {1}")]
    UnmountFailed(String, String),
}

/// Checks System Integrity Protection (SIP) status.
/// Raw writes to internal SSDs are blocked if SIP is enabled.
pub fn check_sip_status() -> Result<(), MacOsError> {
    let output = Command::new("csrutil")
        .arg("status")
        .output()?;
        
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("System Integrity Protection status: enabled") {
        return Err(MacOsError::SipEnabled);
    }
    
    Ok(())
}

/// Force unmounts the disk and all its partitions using diskutil.
/// This is required to acquire an exclusive lock for O_DIRECT.
pub fn unmount_disk(device_path: &str) -> Result<(), MacOsError> {
    let output = Command::new("diskutil")
        .arg("unmountDisk")
        .arg("force")
        .arg(device_path)
        .output()?;
        
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacOsError::UnmountFailed(device_path.to_string(), stderr.to_string()));
    }
    
    Ok(())
}

/// Prepares the macOS device for raw block access.
pub fn prepare_device(device_path: &str) -> Result<(), MacOsError> {
    // 1. Log/Check SIP 
    if let Err(e) = check_sip_status() {
        eprintln!("WARNING: {}", e);
    }

    // 2. Unmount the disk via DiskArbitration / diskutil
    println!("Unmounting {} to acquire exclusive raw access...", device_path);
    unmount_disk(device_path)?;
    
    Ok(())
}
