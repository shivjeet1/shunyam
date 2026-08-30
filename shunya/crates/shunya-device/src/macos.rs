use crate::Device;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::dictionary::CFDictionary;
use io_kit_sys::{
    IOMasterPort, IOServiceMatching, IOServiceGetMatchingServices, IOIteratorNext, IORegistryEntryCreateCFProperty,
    IOObjectRelease,
};
use io_kit_sys::types::{io_iterator_t, io_object_t, mach_port_t};
use std::ffi::CString;

// Helper to convert CFString to Rust String
fn cfstring_to_string(cf_str: core_foundation::string::CFStringRef) -> Option<String> {
    unsafe {
        let string = core_foundation::string::CFString::wrap_under_get_rule(cf_str);
        Some(string.to_string())
    }
}

pub fn enumerate_devices() -> Result<Vec<Device>, crate::DeviceError> {
    let mut devices = Vec::new();
    
    unsafe {
        let mut master_port: mach_port_t = 0;
        IOMasterPort(0, &mut master_port);

        // We want to match all block storage devices ("IOMedia")
        let matching_dict = IOServiceMatching(CString::new("IOMedia").unwrap().as_ptr());
        let mut iterator: io_iterator_t = 0;
        
        let result = IOServiceGetMatchingServices(master_port, matching_dict, &mut iterator);
        if result != 0 {
            return Err(crate::DeviceError::Io(std::io::Error::last_os_error()));
        }

        loop {
            let service: io_object_t = IOIteratorNext(iterator);
            if service == 0 {
                break;
            }

            // Get BSD Name (e.g., /dev/disk0)
            let bsd_name_key = CFString::new("BSD Name");
            let bsd_name_ref = IORegistryEntryCreateCFProperty(
                service,
                bsd_name_key.as_CFTypeRef() as *const _,
                std::ptr::null_mut(),
                0,
            );

            // Get Model Name
            let model_key = CFString::new("Model");
            let model_ref = IORegistryEntryCreateCFProperty(
                service,
                model_key.as_CFTypeRef() as *const _,
                std::ptr::null_mut(),
                0,
            );
            
            // Determine if it's NVMe (by checking protocol/transport characteristics)
            let protocol_key = CFString::new("Physical Interconnect");
            let protocol_ref = IORegistryEntryCreateCFProperty(
                service,
                protocol_key.as_CFTypeRef() as *const _,
                std::ptr::null_mut(),
                0,
            );

            if !bsd_name_ref.is_null() && !model_ref.is_null() {
                let path = cfstring_to_string(bsd_name_ref as _).unwrap_or_default();
                let model = cfstring_to_string(model_ref as _).unwrap_or_default();
                
                let mut is_nvme = false;
                if !protocol_ref.is_null() {
                    let protocol = cfstring_to_string(protocol_ref as _).unwrap_or_default();
                    if protocol.contains("PCI-Express") || protocol.contains("NVMe") {
                        is_nvme = true;
                    }
                    core_foundation::base::CFRelease(protocol_ref);
                }

                // Apple formats disks as /dev/diskX
                let full_path = format!("/dev/{}", path);

                devices.push(Device {
                    path: full_path,
                    model,
                    is_nvme,
                });
            }

            if !bsd_name_ref.is_null() {
                core_foundation::base::CFRelease(bsd_name_ref);
            }
            if !model_ref.is_null() {
                core_foundation::base::CFRelease(model_ref);
            }

            IOObjectRelease(service);
        }
        
        IOObjectRelease(iterator);
    }

    Ok(devices)
}
