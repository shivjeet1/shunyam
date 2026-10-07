use std::process::Command;
use std::fs;
use std::path::Path;

/// Provisions a tiny ISO9660 partition at the start of the wiped drive and stores the certs.
pub fn store_certificate_on_drive(
    device: &str,
    json_content: &str,
    pdf_bytes: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    eprintln!("Partitioning 20MB ISO9660 certificate store at the start of {}...", device);
    
    // Zap existing partition tables
    // 1. Create a 20MB partition at the start of the device (type Microsoft Basic Data) for certs
    // 2. Create a second partition using the remaining space for user data
    use std::io::Write;
    let mut sfdisk_child = Command::new("sfdisk")
        .arg("--wipe").arg("always")
        .arg("--wipe-partitions").arg("always")
        .arg(device)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn sfdisk: {}", e))?;

    let sfdisk_script = "label: gpt\nsize=20M, type=EBD0A0A2-B9E5-4433-87C0-68B6B72699C7, attrs=\"60,63\"\ntype=EBD0A0A2-B9E5-4433-87C0-68B6B72699C7\n";
    
    if let Some(mut stdin) = sfdisk_child.stdin.take() {
        stdin.write_all(sfdisk_script.as_bytes()).map_err(|e| format!("Failed to write to sfdisk stdin: {}", e))?;
    }

    let sfdisk_out = sfdisk_child.wait_with_output().map_err(|e| format!("Failed to wait for sfdisk: {}", e))?;
    if !sfdisk_out.status.success() {
        return Err(format!("sfdisk failed: {}", String::from_utf8_lossy(&sfdisk_out.stderr)));
    }

    // Force kernel to re-read partition table
    let _ = Command::new("partprobe").arg(device).status();
    let _ = Command::new("udevadm").arg("settle").status();

    // Construct the partition paths
    let (part1_path, part2_path) = if device.contains("nvme") || device.contains("mmc") || device.contains("loop") {
        (format!("{}p1", device), format!("{}p2", device))
    } else {
        (format!("{}1", device), format!("{}2", device))
    };

    eprintln!("Formatting {} as exFAT...", part2_path);
    // 2. Format Part 2 as exFAT (User Data)
    let mkfs_exfat_out = Command::new("mkfs.exfat")
        .arg("-n")
        .arg("DATA")
        .arg(&part2_path)
        .output()
        .map_err(|e| format!("Failed to run mkfs.exfat on data partition: {}", e))?;
        
    if !mkfs_exfat_out.status.success() {
        return Err(format!("mkfs.exfat failed (is exfatprogs installed?): {}", String::from_utf8_lossy(&mkfs_exfat_out.stderr)));
    }

    // 3. Create a temporary directory for ISO contents
    let mktemp_out = Command::new("mktemp")
        .arg("-d")
        .arg("/tmp/shunya_iso_XXXXXX")
        .output()
        .map_err(|e| format!("Failed to run mktemp: {}", e))?;
    if !mktemp_out.status.success() {
        return Err(format!("mktemp failed: {}", String::from_utf8_lossy(&mktemp_out.stderr)));
    }
    let iso_dir = String::from_utf8_lossy(&mktemp_out.stdout).trim().to_string();
    
    // 4. Write the files to the temp directory
    let dest_json = Path::new(&iso_dir).join("manifest.json");
    let dest_pdf = Path::new(&iso_dir).join("certificate.pdf");
    let dest_sig = Path::new(&iso_dir).join("manifest.sig");

    let write_res = (|| -> Result<(), std::io::Error> {
        fs::write(&dest_json, json_content)?;
        fs::write(&dest_pdf, pdf_bytes)?;
        fs::write(&dest_sig, signature)?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_dir_all(&iso_dir);
        return Err(format!("Failed to write files: {}", e));
    }

    // 5. Generate the ISO9660 image
    let iso_path = format!("{}.iso", iso_dir);
    let geniso_out = Command::new("genisoimage")
        .arg("-V").arg("SHUNYA_CERT")
        .arg("-J").arg("-R")
        .arg("-o").arg(&iso_path)
        .arg(&iso_dir)
        .output()
        .map_err(|e| format!("Failed to run genisoimage: {}", e))?;
        
    if !geniso_out.status.success() {
        let _ = fs::remove_dir_all(&iso_dir);
        return Err(format!("genisoimage failed: {}", String::from_utf8_lossy(&geniso_out.stderr)));
    }

    // 6. Write ISO directly to the partition using dd
    eprintln!("Writing ISO9660 to {}...", part1_path);
    let dd_out = Command::new("dd")
        .arg(format!("if={}", iso_path))
        .arg(format!("of={}", part1_path))
        .arg("bs=1M")
        .arg("conv=fdatasync")
        .output()
        .map_err(|e| format!("Failed to run dd for ISO: {}", e))?;
        
    if !dd_out.status.success() {
        let _ = fs::remove_dir_all(&iso_dir);
        let _ = fs::remove_file(&iso_path);
        return Err(format!("dd failed writing ISO: {}", String::from_utf8_lossy(&dd_out.stderr)));
    }

    // 7. Clean up temp files
    let _ = fs::remove_dir_all(&iso_dir);
    let _ = fs::remove_file(&iso_path);

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

    // Verify the PIV signature using the public key stored in the manifest
    let is_valid = shunya_cert::signer::PIVSigner::verify_manifest(&json_content, &signature)
        .map_err(|e| format!("Signature verification error: {}", e))?;

    Ok((json_content, is_valid))
}
