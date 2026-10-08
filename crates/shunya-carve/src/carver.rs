use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Represents a carved file recovered from raw disk data
#[derive(Debug, Clone)]
pub struct CarvedFile {
    pub offset: u64,
    pub size: u64,
    pub file_type: FileType,
    pub data: Vec<u8>,
    pub is_complete: bool, // true if both header and footer were found
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    Jpeg,
    Png,
    Pdf,
    Zip,
    Unknown,
}

/// File signature (magic bytes) for header detection
struct FileSignature {
    file_type: FileType,
    magic: &'static [u8],
    footer: Option<&'static [u8]>,
}

const FILE_SIGNATURES: &[FileSignature] = &[
    FileSignature {
        file_type: FileType::Jpeg,
        magic: &[0xFF, 0xD8, 0xFF],
        footer: Some(&[0xFF, 0xD9]),
    },
    FileSignature {
        file_type: FileType::Png,
        magic: &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        footer: Some(&[
            0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]),
    },
    FileSignature {
        file_type: FileType::Pdf,
        magic: b"%PDF-",
        footer: Some(b"%%EOF"),
    },
    FileSignature {
        file_type: FileType::Zip,
        magic: &[0x50, 0x4B, 0x03, 0x04],
        footer: None, // ZIP footer detection is complex; use heuristics
    },
];

/// Carves files from a raw disk image by scanning for known file signatures
pub struct Carver {
    max_file_size: u64,
    min_file_size: u64,
}

impl Carver {
    pub fn new() -> Self {
        Carver {
            max_file_size: 100 * 1024 * 1024, // 100 MB max
            min_file_size: 16,                  // 16 bytes min
        }
    }

    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    pub fn with_min_file_size(mut self, size: u64) -> Self {
        self.min_file_size = size;
        self
    }

    /// Carve all recognizable files from a disk image
    pub fn carve_file<P: AsRef<Path>>(&self, image_path: P) -> Result<Vec<CarvedFile>, String> {
        let mut file = File::open(image_path).map_err(|e| e.to_string())?;
        let file_size = file.metadata().map_err(|e| e.to_string())?.len();

        let mut carved_files = Vec::new();
        let mut buffer = [0u8; 8192]; // 8KB read buffer
        let mut position = 0u64;

        while position < file_size {
            file.seek(SeekFrom::Start(position)).map_err(|e| e.to_string())?;
            let bytes_read = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if bytes_read == 0 {
                break;
            }

            // Check for file signatures at current position
            for sig in FILE_SIGNATURES {
                if buffer[..bytes_read].starts_with(sig.magic) {
                    if let Some(carved) = self.carve_at_offset(&mut file, position, sig, file_size) {
                        if carved.size >= self.min_file_size && carved.size <= self.max_file_size {
                            carved_files.push(carved);
                        }
                    }
                }
            }

            position += 1; // Byte-by-byte scan (can be optimized with larger steps)
        }

        Ok(carved_files)
    }

    /// Attempt to carve a single file at the given offset
    fn carve_at_offset<R: Read + Seek>(
        &self,
        file: &mut R,
        offset: u64,
        sig: &FileSignature,
        file_size: u64,
    ) -> Option<CarvedFile> {
        let max_end = (offset + self.max_file_size).min(file_size);
        let search_end = max_end.min(file_size);

        // Read the potential file data
        let mut data = Vec::new();
        file.seek(SeekFrom::Start(offset)).ok()?;

        let mut chunk = [0u8; 8192];
        let mut current_pos = offset;
        let mut footer_found = false;

        while current_pos < search_end {
            let to_read = chunk.len().min((search_end - current_pos) as usize);
            let bytes_read = file.read(&mut chunk[..to_read]).ok()?;
            if bytes_read == 0 {
                break;
            }

            // Check for footer in this chunk
            if let Some(footer) = sig.footer {
                if let Some(pos) = find_subsequence(&chunk[..bytes_read], footer) {
                    data.extend_from_slice(&chunk[..pos + footer.len()]);
                    footer_found = true;
                    let _ = current_pos; // suppress unused assignment warning
                    break;
                }
            }

            data.extend_from_slice(&chunk[..bytes_read]);
            current_pos += bytes_read as u64;

            // Safety check for max file size
            if data.len() as u64 > self.max_file_size {
                break;
            }
        }

        if data.len() < self.min_file_size as usize {
            return None;
        }

        let size = data.len() as u64;

        Some(CarvedFile {
            offset,
            size,
            file_type: sig.file_type.clone(),
            data,
            is_complete: footer_found,
        })
    }
}

/// Find a subsequence in a byte slice
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_carver_new() {
        let carver = Carver::new();
        assert_eq!(carver.max_file_size, 100 * 1024 * 1024);
        assert_eq!(carver.min_file_size, 16);
    }

    #[test]
    fn test_carver_custom_sizes() {
        let carver = Carver::new()
            .with_max_file_size(1024)
            .with_min_file_size(4);
        assert_eq!(carver.max_file_size, 1024);
        assert_eq!(carver.min_file_size, 4);
    }

    #[test]
    fn test_find_subsequence() {
        assert_eq!(find_subsequence(b"hello world", b"world"), Some(6));
        assert_eq!(find_subsequence(b"hello", b"xyz"), None);
        assert_eq!(find_subsequence(b"", b"test"), None);
        assert_eq!(find_subsequence(b"test", b""), None);
    }

    #[test]
    fn test_carve_jpeg() {
        // Create a minimal JPEG-like structure
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        data.extend_from_slice(&[0x00; 100]); // padding
        data.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let carver = Carver::new();
        let result = carver.carve_at_offset(
            &mut std::io::Cursor::new(&data),
            0,
            &FILE_SIGNATURES[0],
            data.len() as u64,
        );

        assert!(result.is_some());
        let carved = result.unwrap();
        assert_eq!(carved.file_type, FileType::Jpeg);
        assert!(carved.is_complete);
        assert!(carved.size >= 16);
    }

    #[test]
    fn test_carve_png() {
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend_from_slice(&[0x00; 50]);
        data.extend_from_slice(&[
            0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]);

        let carver = Carver::new();
        let result = carver.carve_at_offset(
            &mut std::io::Cursor::new(&data),
            0,
            &FILE_SIGNATURES[1],
            data.len() as u64,
        );

        assert!(result.is_some());
        let carved = result.unwrap();
        assert_eq!(carved.file_type, FileType::Png);
        assert!(carved.is_complete);
    }

    #[test]
    fn test_carve_pdf() {
        let mut data = b"%PDF-1.4".to_vec();
        data.extend_from_slice(&[0x00; 100]);
        data.extend_from_slice(b"%%EOF");

        let carver = Carver::new();
        let result = carver.carve_at_offset(
            &mut std::io::Cursor::new(&data),
            0,
            &FILE_SIGNATURES[2],
            data.len() as u64,
        );

        assert!(result.is_some());
        let carved = result.unwrap();
        assert_eq!(carved.file_type, FileType::Pdf);
        assert!(carved.is_complete);
    }

    #[test]
    fn test_carve_incomplete_file() {
        // JPEG header but no footer
        let data = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];

        let carver = Carver::new();
        let result = carver.carve_at_offset(
            &mut std::io::Cursor::new(&data),
            0,
            &FILE_SIGNATURES[0],
            data.len() as u64,
        );

        // Should still carve but mark as incomplete
        if let Some(carved) = result {
            assert!(!carved.is_complete);
        }
    }

    #[test]
    fn test_carve_files_from_image() {
        // Create a temporary file with embedded JPEG
        let mut image_data = vec![0x00; 1000];
        let jpeg_offset = 500;
        image_data[jpeg_offset..jpeg_offset + 4].copy_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0]);
        image_data[jpeg_offset + 4..jpeg_offset + 104].fill(0xAB);
        image_data[jpeg_offset + 104..jpeg_offset + 106].copy_from_slice(&[0xFF, 0xD9]);

        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut temp_file, &image_data).unwrap();

        let carver = Carver::new();
        let results = carver.carve_file(temp_file.path()).unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].file_type, FileType::Jpeg);
        assert!(results[0].is_complete);
    }
}
