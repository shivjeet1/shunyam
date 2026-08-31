use crate::aligned::AlignedBuffer;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WipeError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Device too small or invalid capacity")]
    InvalidCapacity,
}

pub struct Wiper {
    device_path: String,
    capacity: u64,
    // Using ChaCha20 as per PRD for secure verified overwrite
    rng: ChaCha20Rng,
}

impl Wiper {
    pub fn new(device_path: &str, capacity: u64, seed: [u8; 32]) -> Self {
        Wiper {
            device_path: device_path.to_string(),
            capacity,
            rng: ChaCha20Rng::from_seed(seed),
        }
    }

    /// Performs a single-pass overwrite using ChaCha20 random stream, writing in 4MiB aligned chunks.
    pub fn overwrite(&mut self) -> Result<(), WipeError> {
        #[cfg(target_os = "macos")]
        if let Err(e) = crate::macos::prepare_device(&self.device_path) {
            eprintln!("macOS prepare warning: {}", e);
        }

        let chunk_size = 4 * 1024 * 1024; // 4 MiB
        let alignment = 4096;

        let mut buffer = AlignedBuffer::new(chunk_size, alignment);
        
        // Open device with O_DIRECT and O_SYNC for direct IO
        let mut file = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_DIRECT | libc::O_SYNC)
            .open(&self.device_path)?;

        let mut written: u64 = 0;
        
        while written < self.capacity {
            let remaining = self.capacity - written;
            let current_chunk = if remaining < chunk_size as u64 {
                remaining as usize
            } else {
                chunk_size
            };

            // Needs to be block aligned. We assume block size is at least 512, usually 4096.
            // If the drive capacity is not a multiple of block size, it's an anomaly but we round down here or pad.
            // For now, we assume capacity is block aligned.
            
            // Fill buffer with random stream
            self.rng.fill_bytes(buffer.as_mut_slice());
            
            let data_to_write = &buffer.as_slice()[..current_chunk];
            file.write_all(data_to_write)?;
            
            written += current_chunk as u64;
            
            // Note: In real implementation, we'd emit progress events here
        }

        // flush just in case
        file.sync_all()?;
        
        Ok(())
    }
}
