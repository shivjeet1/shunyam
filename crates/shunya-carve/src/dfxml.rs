use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Dfxml {
    #[serde(rename = "creator")]
    pub creator: Creator,
    
    #[serde(rename = "volume")]
    pub volume: Option<Volume>,
}

#[derive(Debug, Deserialize)]
pub struct Creator {
    pub program: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct Volume {
    #[serde(rename = "fileobject")]
    pub files: Vec<FileObject>,
}

#[derive(Debug, Deserialize)]
pub struct FileObject {
    pub filename: String,
    pub filesize: u64,
    
    #[serde(rename = "byte_runs")]
    pub byte_runs: Option<ByteRuns>,
}

#[derive(Debug, Deserialize)]
pub struct ByteRuns {
    #[serde(rename = "byte_run")]
    pub runs: Vec<ByteRun>,
}

#[derive(Debug, Deserialize)]
pub struct ByteRun {
    #[serde(rename = "@file_offset")]
    pub file_offset: u64,
    #[serde(rename = "@img_offset")]
    pub img_offset: u64,
    #[serde(rename = "@len")]
    pub len: u64,
}
