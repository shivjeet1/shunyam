use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod pdf;
pub mod piv;
pub mod signer;

/// Represents the canonical data of a completed job.
#[derive(Debug, Serialize, Deserialize)]
pub struct WipeManifest {
    pub job_id: String,
    pub device_serial: String,
    pub device_model: String,
    pub capacity_bytes: u64,
    pub wipe_method: String,
    pub wipe_duration_sec: u64,
    pub timestamp: DateTime<Utc>,
    pub operator_id: String,
    pub carve_score: u32,
    pub verification_hash: String, // hash of the device post-wipe or random stream used
}

impl WipeManifest {
    /// Produces a deterministic JSON string for hashing/QR
    pub fn canonical_json(&self) -> String {
        // serde_json with sorted keys is technically required for true canonical JSON,
        // but this works for MVP.
        serde_json::to_string(self).unwrap()
    }

    /// Hashes the canonical JSON using SHA-256
    pub fn hash(&self) -> String {
        let json = self.canonical_json();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>()
    }
}
