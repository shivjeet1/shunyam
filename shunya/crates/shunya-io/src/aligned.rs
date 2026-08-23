use libc::{c_void, posix_memalign, free};
use std::ptr;
use std::slice;

pub struct AlignedBuffer {
    ptr: *mut u8,
    size: usize,
    capacity: usize,
}

impl AlignedBuffer {
    pub fn new(capacity: usize, alignment: usize) -> Self {
        assert!(alignment.is_power_of_two());
        assert!(capacity > 0);
        let mut ptr: *mut c_void = ptr::null_mut();
        
        let ret = unsafe { posix_memalign(&mut ptr, alignment, capacity) };
        if ret != 0 || ptr.is_null() {
            panic!("posix_memalign failed to allocate {} bytes with alignment {}", capacity, alignment);
        }

        AlignedBuffer {
            ptr: ptr as *mut u8,
            size: 0,
            capacity,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn set_size(&mut self, size: usize) {
        assert!(size <= self.capacity);
        self.size = size;
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr, self.size) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.capacity) }
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                free(self.ptr as *mut c_void);
            }
            self.ptr = ptr::null_mut();
        }
    }
}

// Ensure it's send/sync if we want to move it across threads safely
unsafe impl Send for AlignedBuffer {}
unsafe impl Sync for AlignedBuffer {}
