#!/bin/sh

profile_shunya() {
    profile_standard
    kernel_flavors="lts"
    kernel_addons="zfs"
    apks="$apks nvme-cli hdparm sg3_utils mmc-utils util-linux gptfdisk smartmontools testdisk android-tools e2fsprogs dosfstools exfatprogs ntfs-3g-progs pciutils usbutils mesa libinput fontconfig pcsc-lite ccid chrony iwd"
    local _k _a
    for _k in $kernel_flavors; do
        apks="$apks linux-$_k"
        for _a in $kernel_addons; do
            apks="$apks $_a-$_k"
        done
    done
    apks="$apks linux-firmware"
    
    # Initfs features for USB boot and KMS
    initfs_features="base bootchart ext4 kms squashfs usb"

    # Custom boot options
    # We set console=tty1 by default but also provide a serial console option in the bootloader.
    kernel_cmdline="console=tty1 console=ttyS0,115200"

    # Generate custom syslinux/grub configs for the boot menu
    syslinux_serial="0 115200"
}
