use zune_jpeg::JpegDecoder;
use std::fs::File;
use std::io::{BufReader, Cursor};

pub fn validate_jpeg(path: &str) -> bool {
    // Attempt full decode with zune-jpeg
    match std::fs::read(path) {
        Ok(data) => {
            let cursor = Cursor::new(data);
            let mut decoder = JpegDecoder::new(cursor);
            match decoder.decode() {
                Ok(_) => true,
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

pub fn validate_png(path: &str) -> bool {
    match File::open(path) {
        Ok(file) => {
            let reader = BufReader::new(file);
            let decoder = png::Decoder::new(reader);
            match decoder.read_info() {
                Ok(_) => true, // basic header check + chunk walk
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

pub fn validate_pdf(path: &str) -> bool {
    // lopdf check
    match lopdf::Document::load(path) {
        Ok(_) => true,
        Err(_) => false,
    }
}

pub fn validate_zip(path: &str) -> bool {
    match File::open(path) {
        Ok(file) => {
            match zip::ZipArchive::new(file) {
                Ok(_) => true,
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}
