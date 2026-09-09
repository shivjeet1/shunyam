use crate::WipeManifest;
use crate::piv::PivToken;

pub struct PIVSigner;

impl PIVSigner {
    /// Attempts to sign the canonical manifest hash using a connected PIV SmartCard.
    /// Falls back to a mock signature if no card is present (for development/MVP).
    pub fn sign_manifest(manifest: &WipeManifest, pin: Option<&str>) -> Result<Vec<u8>, String> {
        let hash_str = manifest.hash();
        let hash_bytes = hash_str.as_bytes();

        match PivToken::connect() {
            Ok(mut token) => {
                println!("PIV SmartCard detected. Attempting hardware signature...");
                
                // If a PIN is provided (e.g. from UI), verify it to unlock the key slot
                if let Some(p) = pin {
                    if let Err(e) = token.verify_pin(p) {
                        return Err(format!("PIN Verification failed: {}", e));
                    }
                }

                match token.sign_data(hash_bytes) {
                    Ok(signature) => {
                        println!("Hardware signature successfully acquired!");
                        Ok(signature)
                    }
                    Err(e) => Err(format!("Failed to sign data on hardware token: {}", e))
                }
            }
            Err(e) => {
                // so the development pipeline isn't blocked by lacking a physical YubiKey.
                println!("WARNING: Could not connect to PIV token ({}). Falling back to mock signature.", e);
                let mock_signature = format!("mock_signature_of_{}", hash_str).into_bytes();
                Ok(mock_signature)
            }
        }
    }
}
