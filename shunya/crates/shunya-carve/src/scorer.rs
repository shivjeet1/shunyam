use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryScore {
    pub header_footer: u32,  // Max 20
    pub structure: u32,      // Max 35
    pub full_decode: u32,    // Max 20
    pub entropy: u32,        // Max 10
    pub metadata: u32,       // Max 15
    pub tsk_run: bool,       // If Sleuth Kit module ran
}

impl RecoveryScore {
    pub fn new(tsk_run: bool) -> Self {
        RecoveryScore {
            header_footer: 0,
            structure: 0,
            full_decode: 0,
            entropy: 0,
            metadata: 0,
            tsk_run,
        }
    }

    pub fn compute_final_score(&self) -> u32 {
        let raw_total = self.header_footer + self.structure + self.full_decode + self.entropy + self.metadata;
        if !self.tsk_run {
            // Scale out of 85 to 100
            ((raw_total as f64 / 85.0) * 100.0).round() as u32
        } else {
            raw_total
        }
    }
}
