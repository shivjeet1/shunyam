#!/bin/sh
# Shunya Secure Wipe - ISO Certificate Extraction Script
# This script scans for mounted or mountable non-system FAT32/exFAT/NTFS/ext4 USB drives
# and extracts the cryptographically signed wipe certificates.

set -e

CERT_DIR="/var/log/shunya/certs"
if [ ! -d "$CERT_DIR" ]; then
    echo "No certificates generated yet."
    exit 1
fi

echo "Scanning for external USB storage devices..."
# For Alpine, we can look at block devices in /sys/block that are removable
for dev in /sys/block/sd*; do
    [ -d "$dev" ] || continue
    
    # Check if removable
    removable=$(cat "$dev/removable")
    if [ "$removable" = "1" ]; then
        dev_name=$(basename "$dev")
        echo "Found removable device: $dev_name"
        
        # Try to mount the first partition or the raw device
        for part in /dev/${dev_name}1 /dev/${dev_name}; do
            if [ -b "$part" ]; then
                mnt_point="/mnt/shunya_export_$(basename "$part")"
                mkdir -p "$mnt_point"
                
                # Attempt to mount (handles FAT32, ext4 natively; exfat/ntfs if tools installed)
                if mount "$part" "$mnt_point" 2>/dev/null; then
                    echo "Mounted $part at $mnt_point. Exporting certificates..."
                    
                    mkdir -p "$mnt_point/Shunya_Certificates"
                    cp -v $CERT_DIR/*.json "$mnt_point/Shunya_Certificates/" 2>/dev/null || true
                    cp -v $CERT_DIR/*.pdf "$mnt_point/Shunya_Certificates/" 2>/dev/null || true
                    
                    sync
                    umount "$mnt_point"
                    rmdir "$mnt_point"
                    
                    echo "Export complete. You may safely remove the USB drive."
                    exit 0
                fi
            fi
        done
    fi
done

echo "No writable external USB drive found or unable to mount."
exit 1
