use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use zune_jpeg::JpegDecoder;

/// Validates a JPEG not just by decoding, but by verifying Start-Of-Image (SOI) 
/// and End-Of-Image (EOI) markers which are often missed in corrupted carved files.
pub fn validate_jpeg(path: &str) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut magic = [0u8; 2];
    if file.read_exact(&mut magic).is_err() || magic != [0xFF, 0xD8] {
        return false;
    }

    // Check for EOI at the end of the file
    if file.seek(SeekFrom::End(-2)).is_ok() {
        let mut eoi = [0u8; 2];
        if file.read_exact(&mut eoi).is_ok() && eoi != [0xFF, 0xD9] {
            // EOI is missing, file is likely truncated or corrupted
            return false;
        }
    } else {
        return false;
    }

    // Now attempt a full structural decode to ensure Huffman tables and SOS are intact
    match std::fs::read(path) {
        Ok(data) => {
            let cursor = std::io::Cursor::new(data);
            let mut decoder = JpegDecoder::new(cursor);
            decoder.decode().is_ok()
        }
        Err(_) => false,
    }
}

/// Validates a PNG by verifying the 8-byte magic header and the mandatory IEND chunk at the EOF.
pub fn validate_png(path: &str) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut magic = [0u8; 8];
    let expected_magic = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if file.read_exact(&mut magic).is_err() || magic != expected_magic {
        return false;
    }

    // A valid PNG MUST end with the IEND chunk: 
    // Length (00 00 00 00), Type (49 45 4E 44), CRC (AE 42 60 82)
    let expected_iend = [
        0x00, 0x00, 0x00, 0x00, // Length: 0
        0x49, 0x45, 0x4E, 0x44, // Type: 'IEND'
        0xAE, 0x42, 0x60, 0x82, // CRC
    ];

    if file.seek(SeekFrom::End(-12)).is_ok() {
        let mut iend = [0u8; 12];
        if file.read_exact(&mut iend).is_err() || iend != expected_iend {
            // IEND missing, file is truncated/corrupted
            return false;
        }
    } else {
        return false;
    }

    // Attempt logical chunk walk
    match std::fs::File::open(path) {
        Ok(f) => {
            let reader = std::io::BufReader::new(f);
            let decoder = png::Decoder::new(reader);
            decoder.read_info().is_ok()
        }
        Err(_) => false,
    }
}

pub fn validate_pdf(path: &str) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    // Check PDF Magic Header
    let mut magic = [0u8; 5];
    if file.read_exact(&mut magic).is_err() || &magic != b"%PDF-" {
        return false;
    }

    // Ensure it ends with %%EOF
    // Note: %%EOF might be followed by whitespace or null bytes in some carved files
    // But for strict validation, we check the last 1024 bytes for it.
    if file.seek(SeekFrom::End(-1024.min(file.metadata().map(|m| m.len() as i64).unwrap_or(0)))).is_ok() {
        let mut tail = Vec::new();
        let _ = file.read_to_end(&mut tail);
        if !tail.windows(5).any(|w| w == b"%%EOF") {
            return false;
        }
    }

    // Fallback to lopdf struct check
    lopdf::Document::load(path).is_ok()
}

pub fn validate_zip(path: &str) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    // PKZIP Magic
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() || magic != [0x50, 0x4B, 0x03, 0x04] {
        return false;
    }

    // Struct validation (reads central directory)
    zip::ZipArchive::new(file).is_ok()
}
