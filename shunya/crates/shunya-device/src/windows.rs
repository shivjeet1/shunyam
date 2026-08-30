use crate::Device;
use std::ffi::c_void;
use windows::core::{PCWSTR, w};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{
    DeviceIoControl, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY, StorageDeviceProperty, PropertyStandardQuery,
    STORAGE_DEVICE_DESCRIPTOR,
};

pub fn enumerate_devices() -> Result<Vec<Device>, crate::DeviceError> {
    let mut devices = Vec::new();

    // Iterate PhysicalDrive0 to PhysicalDrive255 as a naive discovery loop
    for i in 0..256 {
        let path = format!("\\\\.\\PhysicalDrive{}", i);
        
        let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let pcwstr = PCWSTR::from_raw(path_w.as_ptr());

        // Attempt to open a handle
        let handle = unsafe {
            CreateFileW(
                pcwstr,
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };

        if let Ok(h) = handle {
            if !h.is_invalid() {
                // If we got a handle, we query its storage properties to see if it's NVMe
                if let Ok((is_nvme, model)) = query_device_descriptor(h) {
                    devices.push(Device {
                        path: path.clone(),
                        model,
                        is_nvme,
                    });
                }
                unsafe { let _ = CloseHandle(h); }
            }
        }
    }

    Ok(devices)
}

fn query_device_descriptor(handle: HANDLE) -> Result<(bool, String), crate::DeviceError> {
    let mut query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0; 1],
    };

    let mut out_buffer = vec![0u8; 1024];
    let mut bytes_returned: u32 = 0;

    let success = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&mut query as *mut _ as *mut c_void),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(out_buffer.as_mut_ptr() as *mut c_void),
            out_buffer.len() as u32,
            Some(&mut bytes_returned),
            None,
        )
    };

    if success.is_ok() && bytes_returned > 0 {
        let descriptor = unsafe { &*(out_buffer.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };
        
        // BusType 0x11 is NVMe (StorageBusTypeNvme)
        let is_nvme = descriptor.BusType == 0x11;
        
        let mut model = String::new();
        if descriptor.ProductIdOffset > 0 {
            let start = descriptor.ProductIdOffset as usize;
            if start < out_buffer.len() {
                // Read null-terminated string
                let mut end = start;
                while end < out_buffer.len() && out_buffer[end] != 0 {
                    end += 1;
                }
                model = String::from_utf8_lossy(&out_buffer[start..end]).trim().to_string();
            }
        }
        
        if model.is_empty() {
            model = "Unknown Windows Drive".to_string();
        }

        return Ok((is_nvme, model));
    }

    Err(crate::DeviceError::Io(std::io::Error::last_os_error()))
}
