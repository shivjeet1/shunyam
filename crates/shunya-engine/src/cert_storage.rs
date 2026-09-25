use std::process::Command;
use std::fs;
use std::path::Path;

/// Provisions a tiny FAT32 partition at the end of the wiped drive and stores the certs.
pub fn store_certificate_on_drive(
    device: &str,
    json_content: &str,
    pdf_bytes: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    println!("Partitioning 20MB FAT32 certificate store at the end of {}...", device);
    
    // 1. Create a 20MB partition at the end of the device (type 0700: Microsoft Basic Data)
    let sgdisk_out = Command::new("sgdisk")
        .arg("-n")
        .arg("1:-20M:0")
        .arg("-t")
        .arg("1:0700")
        .arg(device)
        .output()
        .map_err(|e| format!("Failed to run sgdisk: {}", e))?;
        
    if !sgdisk_out.status.success() {
        return Err(format!("sgdisk failed: {}", String::from_utf8_lossy(&sgdisk_out.stderr)));
    }

    // Give the kernel a moment to re-read the partition table
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Construct the partition path (handles /dev/nvme0n1p1 vs /dev/sda1)
    let part_path = if device.contains("nvme") || device.contains("mmc") {
        format!("{}p1", device)
    } else {
        format!("{}1", device)
    };

    println!("Formatting {} as FAT32...", part_path);
    // 2. Format as FAT32
    let mkfs_out = Command::new("mkfs.vfat")
        .arg("-n")
        .arg("SHUNYA_CERT")
        .arg(&part_path)
        .output()
        .map_err(|e| format!("Failed to run mkfs.vfat: {}", e))?;
        
    if !mkfs_out.status.success() {
        return Err(format!("mkfs.vfat failed: {}", String::from_utf8_lossy(&mkfs_out.stderr)));
    }

    // 3. Mount it
    let mount_dir = "/tmp/shunya_cert_mount";
    fs::create_dir_all(mount_dir).unwrap_or_default();
    
    let mount_out = Command::new("mount")
        .arg(&part_path)
        .arg(mount_dir)
        .output()
        .map_err(|e| format!("Failed to run mount: {}", e))?;
        
    if !mount_out.status.success() {
        return Err(format!("mount failed: {}", String::from_utf8_lossy(&mount_out.stderr)));
    }

    // 4. Write the files
    let json_path = Path::new(mount_dir).join("manifest.json");
    let pdf_path = Path::new(mount_dir).join("certificate.pdf");
    let sig_path = Path::new(mount_dir).join("manifest.sig");

    if let Err(e) = fs::write(&json_path, json_content) {
        let _ = Command::new("umount").arg(mount_dir).output();
        return Err(format!("Failed to write manifest.json: {}", e));
    }
    
    if let Err(e) = fs::write(&pdf_path, pdf_bytes) {
        let _ = Command::new("umount").arg(mount_dir).output();
        return Err(format!("Failed to write certificate.pdf: {}", e));
    }

    if let Err(e) = fs::write(&sig_path, signature) {
        let _ = Command::new("umount").arg(mount_dir).output();
        return Err(format!("Failed to write manifest.sig: {}", e));
    }

    // 5. Unmount
    let umount_out = Command::new("umount")
        .arg(mount_dir)
        .output()
        .map_err(|e| format!("Failed to run umount: {}", e))?;
        
    if !umount_out.status.success() {
        return Err(format!("umount failed: {}", String::from_utf8_lossy(&umount_out.stderr)));
    }

    println!("Successfully secured certificates on the wiped block device.");
    Ok(())
}

/// Verifies the certs on a given drive.
pub fn verify_certificate_on_drive(device: &str) -> Result<(String, bool), String> {
    let part_path = if device.contains("nvme") || device.contains("mmc") {
        format!("{}p1", device)
    } else {
        format!("{}1", device)
    };

    // 1. Mount it
    let mount_dir = "/tmp/shunya_verify_mount";
    fs::create_dir_all(mount_dir).unwrap_or_default();
    
    let mount_out = Command::new("mount")
        .arg(&part_path)
        .arg(mount_dir)
        .output()
        .map_err(|e| format!("Failed to run mount: {}", e))?;
        
    if !mount_out.status.success() {
        return Err(format!("mount failed (is the partition provisioned?): {}", String::from_utf8_lossy(&mount_out.stderr)));
    }

    // 2. Read the files
    let json_path = Path::new(mount_dir).join("manifest.json");
    let sig_path = Path::new(mount_dir).join("manifest.sig");

    let json_content_res = fs::read_to_string(&json_path);
    let sig_content_res = fs::read(&sig_path);

    // 3. Unmount immediately
    let _ = Command::new("umount").arg(mount_dir).output();

    let json_content = json_content_res.map_err(|e| format!("Could not read manifest.json: {}", e))?;
    let signature = sig_content_res.map_err(|e| format!("Could not read manifest.sig: {}", e))?;

    // Verify logic (Mock fallback, or route to shunya-cert)
    let is_valid = shunya_cert::signer::PIVSigner::verify_manifest_mock(&json_content, &signature);

    Ok((json_content, is_valid))
}
