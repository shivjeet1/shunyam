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
    eprintln!("Partitioning 20MB FAT32 certificate store at the end of {}...", device);
    
    // Zap existing partition tables
    let zap_out = Command::new("sgdisk")
        .arg("-Z")
        .arg(device)
        .output()
        .map_err(|e| format!("Failed to run sgdisk -Z: {}", e))?;
    if !zap_out.status.success() {
        return Err(format!("sgdisk -Z failed: {}", String::from_utf8_lossy(&zap_out.stderr)));
    }

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

    // Force kernel to re-read partition table
    let _ = Command::new("partprobe").arg(device).status();

    // Give the kernel a moment to re-read the partition table via udevadm
    let _ = Command::new("udevadm").arg("settle").status();

    // Construct the partition path (handles /dev/nvme0n1p1 vs /dev/sda1)
    let part_path = if device.contains("nvme") || device.contains("mmc") || device.contains("loop") {
        format!("{}p1", device)
    } else {
        format!("{}1", device)
    };

    eprintln!("Formatting {} as FAT32...", part_path);
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

    // 3. Mount it using a dynamic temp directory
    let mktemp_out = Command::new("mktemp")
        .arg("-d")
        .arg("/tmp/shunya_cert_XXXXXX")
        .output()
        .map_err(|e| format!("Failed to run mktemp: {}", e))?;
    if !mktemp_out.status.success() {
        return Err(format!("mktemp failed: {}", String::from_utf8_lossy(&mktemp_out.stderr)));
    }
    let mount_dir = String::from_utf8_lossy(&mktemp_out.stdout).trim().to_string();
    
    let mount_out = Command::new("mount")
        .arg(&part_path)
        .arg(&mount_dir)
        .output()
        .map_err(|e| format!("Failed to run mount: {}", e))?;
        
    if !mount_out.status.success() {
        let _ = fs::remove_dir_all(&mount_dir);
        return Err(format!("mount failed: {}", String::from_utf8_lossy(&mount_out.stderr)));
    }

    // 4. Write the files
    let json_path = Path::new(&mount_dir).join("manifest.json");
    let pdf_path = Path::new(&mount_dir).join("certificate.pdf");
    let sig_path = Path::new(&mount_dir).join("manifest.sig");

    let write_res = (|| -> Result<(), std::io::Error> {
        fs::write(&json_path, json_content)?;
        fs::write(&pdf_path, pdf_bytes)?;
        fs::write(&sig_path, signature)?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = Command::new("umount").arg(&mount_dir).output();
        let _ = fs::remove_dir_all(&mount_dir);
        return Err(format!("Failed to write files: {}", e));
    }

    // 5. Unmount
    let umount_out = Command::new("umount")
        .arg(&mount_dir)
        .output()
        .map_err(|e| format!("Failed to run umount: {}", e))?;
        
    let _ = fs::remove_dir_all(&mount_dir);

    if !umount_out.status.success() {
        return Err(format!("umount failed: {}", String::from_utf8_lossy(&umount_out.stderr)));
    }

    eprintln!("Successfully secured certificates on the wiped block device.");
    Ok(())
}

/// Verifies the certs on a given drive.
pub fn verify_certificate_on_drive(device: &str) -> Result<(String, bool), String> {
    let part_path = if device.contains("nvme") || device.contains("mmc") || device.contains("loop") {
        format!("{}p1", device)
    } else {
        format!("{}1", device)
    };

    // 1. Mount it read-only
    let mktemp_out = Command::new("mktemp")
        .arg("-d")
        .arg("/tmp/shunya_verify_XXXXXX")
        .output()
        .map_err(|e| format!("Failed to run mktemp: {}", e))?;
    if !mktemp_out.status.success() {
        return Err(format!("mktemp failed: {}", String::from_utf8_lossy(&mktemp_out.stderr)));
    }
    let mount_dir = String::from_utf8_lossy(&mktemp_out.stdout).trim().to_string();
    
    let mount_out = Command::new("mount")
        .arg("-o")
        .arg("ro")
        .arg(&part_path)
        .arg(&mount_dir)
        .output()
        .map_err(|e| format!("Failed to run mount: {}", e))?;
        
    if !mount_out.status.success() {
        let _ = fs::remove_dir_all(&mount_dir);
        return Err(format!("mount failed (is the partition provisioned?): {}", String::from_utf8_lossy(&mount_out.stderr)));
    }

    // 2. Read the files
    let json_path = Path::new(&mount_dir).join("manifest.json");
    let sig_path = Path::new(&mount_dir).join("manifest.sig");

    let json_content_res = fs::read_to_string(&json_path);
    let sig_content_res = fs::read(&sig_path);

    // 3. Unmount immediately and clean up
    let _ = Command::new("umount").arg(&mount_dir).output();
    let _ = fs::remove_dir_all(&mount_dir);

    let json_content = json_content_res.map_err(|e| format!("Could not read manifest.json: {}", e))?;
    let signature = sig_content_res.map_err(|e| format!("Could not read manifest.sig: {}", e))?;

    // Verify logic (Mock fallback, or route to shunya-cert)
    let is_valid = shunya_cert::signer::PIVSigner::verify_manifest_mock(&json_content, &signature);

    Ok((json_content, is_valid))
}
