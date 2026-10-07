use crate::piv::PivToken;
use crate::WipeManifest;
use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey as RsaVerifyingKey};
use rsa::pkcs8::DecodePublicKey;
use rsa::RsaPublicKey;
use sha2::Sha256;
use signature::hazmat::PrehashVerifier;

pub struct PIVSigner;

impl PIVSigner {
    /// Signs the manifest hash using a connected PIV SmartCard.
    /// The manifest's `public_key` field is populated with the card's public key PEM.
    /// Returns the raw signature bytes.
    ///
    /// # Errors
    /// Returns an error if no PIV card is found, PIN verification fails, or signing fails.
    /// There is NO mock fallback — a physical PIV token is required.
    pub fn sign_manifest(manifest: &mut WipeManifest, pin: Option<&str>) -> Result<Vec<u8>, String> {
        let mut token = PivToken::connect().map_err(|e| format!("PIV token error: {}", e))?;

        eprintln!("PIV SmartCard detected.");

        if let Some(p) = pin {
            token
                .verify_pin(p)
                .map_err(|e| format!("PIN verification failed: {}", e))?;
        }

        // Extract the public key from the card and store it in the manifest
        let public_key_pem = token
            .get_public_key_pem()
            .map_err(|e| format!("Failed to extract public key: {}", e))?;
        manifest.public_key = public_key_pem;

        // Compute the hash and sign it
        let hash_str = manifest.hash();
        let hash_bytes = hex::decode(&hash_str).map_err(|e| format!("Hash decode error: {}", e))?;

        let signature = token
            .sign_data(&hash_bytes)
            .map_err(|e| format!("Failed to sign data on hardware token: {}", e))?;

        eprintln!("Hardware signature acquired successfully.");
        Ok(signature)
    }

    /// Verifies a PIV signature against a manifest.
    ///
    /// Parses the manifest JSON to extract the public key, recomputes the hash,
    /// and verifies the signature using the appropriate algorithm (RSA or ECDSA P-256).
    ///
    /// Returns `true` if the signature is valid, `false` otherwise.
    pub fn verify_manifest(manifest_json: &str, signature: &[u8]) -> Result<bool, String> {
        let manifest: WipeManifest =
            serde_json::from_str(manifest_json).map_err(|e| format!("JSON parse error: {}", e))?;

        if manifest.public_key.is_empty() {
            return Err("Manifest does not contain a public key".to_string());
        }

        let hash_str = manifest.hash();
        let hash_bytes = hex::decode(&hash_str).map_err(|e| format!("Hash decode error: {}", e))?;

        verify_signature(&manifest.public_key, &hash_bytes, signature)
    }
}

/// Verifies a signature against a hash using the given PEM-encoded public key.
/// Supports RSA-2048 (PKCS#1 v1.5 with SHA-256) and ECDSA P-256.
fn verify_signature(
    public_key_pem: &str,
    hash_bytes: &[u8],
    signature: &[u8],
) -> Result<bool, String> {
    // Try RSA first
    if let Ok(result) = verify_rsa(public_key_pem, hash_bytes, signature) {
        return Ok(result);
    }

    // Try ECDSA P-256
    if let Ok(result) = verify_ecdsa_p256(public_key_pem, hash_bytes, signature) {
        return Ok(result);
    }

    Ok(false)
}

fn verify_rsa(
    public_key_pem: &str,
    hash_bytes: &[u8],
    signature: &[u8],
) -> Result<bool, String> {
    let public_key = RsaPublicKey::from_public_key_pem(public_key_pem)
        .map_err(|e| format!("Failed to parse RSA public key: {}", e))?;
    let verifying_key = RsaVerifyingKey::<Sha256>::new_unprefixed(public_key);
    let sig = RsaSignature::try_from(signature)
        .map_err(|e| format!("Invalid RSA signature format: {}", e))?;

    match verifying_key.verify_prehash(hash_bytes, &sig) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

fn verify_ecdsa_p256(
    public_key_pem: &str,
    hash_bytes: &[u8],
    signature: &[u8],
) -> Result<bool, String> {
    use p256::ecdsa::signature::hazmat::PrehashVerifier as _;
    use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
    use p256::pkcs8::DecodePublicKey as _;

    let verifying_key = P256VerifyingKey::from_public_key_pem(public_key_pem)
        .map_err(|e| format!("Failed to parse P-256 public key: {}", e))?;
    let sig = P256Signature::from_slice(signature)
        .map_err(|e| format!("Invalid P-256 signature format: {}", e))?;

    match verifying_key.verify_prehash(hash_bytes, &sig) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WipeManifest;
    use chrono::Utc;
    use rsa::pkcs1v15::SigningKey;
    use rsa::pkcs8::EncodePublicKey;
    use rsa::RsaPrivateKey;
    use sha2::Sha256;
    use signature::hazmat::PrehashSigner;
    use signature::SignatureEncoding;

    fn create_test_manifest() -> WipeManifest {
        WipeManifest {
            job_id: "test-job-001".to_string(),
            device_serial: "SN12345".to_string(),
            device_model: "TestSSD".to_string(),
            capacity_bytes: 512_000_000_000,
            wipe_method: "NIST 800-88 Purge".to_string(),
            wipe_duration_sec: 120,
            timestamp: Utc::now(),
            operator_id: "op-001".to_string(),
            carve_score: 95,
            verification_hash: "abc123".to_string(),
            public_key: String::new(),
        }
    }

    #[test]
    fn test_rsa_sign_and_verify() {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let public_key = RsaPublicKey::from(&private_key);

        let public_pem = public_key
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap();

        let mut manifest = create_test_manifest();
        manifest.public_key = public_pem;

        let hash_str = manifest.hash();
        let hash_bytes = hex::decode(&hash_str).unwrap();

        let signing_key = SigningKey::<Sha256>::new_unprefixed(private_key);
        let signature = signing_key.sign_prehash(&hash_bytes).unwrap().to_vec();

        let manifest_json = manifest.canonical_json();
        let result = PIVSigner::verify_manifest(&manifest_json, &signature).unwrap();
        assert!(result, "RSA signature should verify");
    }

    #[test]
    fn test_rsa_tampered_manifest_fails() {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let public_key = RsaPublicKey::from(&private_key);
        let public_pem = public_key
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap();

        let mut manifest = create_test_manifest();
        manifest.public_key = public_pem;

        let hash_str = manifest.hash();
        let hash_bytes = hex::decode(&hash_str).unwrap();

        let signing_key = SigningKey::<Sha256>::new_unprefixed(private_key);
        let signature = signing_key.sign_prehash(&hash_bytes).unwrap().to_vec();

        manifest.device_serial = "TAMPERED".to_string();
        let tampered_json = manifest.canonical_json();

        let result = PIVSigner::verify_manifest(&tampered_json, &signature).unwrap();
        assert!(!result, "Tampered manifest should fail verification");
    }

    #[test]
    fn test_ecdsa_p256_sign_and_verify() {
        use p256::ecdsa::SigningKey as P256SigningKey;
        use p256::pkcs8::EncodePublicKey as _;

        let mut rng = rand::thread_rng();
        let signing_key = P256SigningKey::random(&mut rng);
        let verifying_key = p256::ecdsa::VerifyingKey::from(&signing_key);

        let public_pem = verifying_key
            .to_public_key_pem(p256::pkcs8::LineEnding::LF)
            .unwrap();

        let mut manifest = create_test_manifest();
        manifest.public_key = public_pem;

        let hash_str = manifest.hash();
        let hash_bytes = hex::decode(&hash_str).unwrap();

        let signature: Vec<u8> = <P256SigningKey as PrehashSigner<p256::ecdsa::Signature>>::sign_prehash(&signing_key, &hash_bytes).unwrap().to_vec();

        let manifest_json = manifest.canonical_json();
        let result = PIVSigner::verify_manifest(&manifest_json, &signature).unwrap();
        assert!(result, "ECDSA P-256 signature should verify");
    }

    #[test]
    fn test_verify_rejects_empty_public_key() {
        let manifest = create_test_manifest();
        let manifest_json = manifest.canonical_json();
        let result = PIVSigner::verify_manifest(&manifest_json, &[0u8; 32]);
        assert!(result.is_err(), "Empty public key should return error");
    }

    #[test]
    fn test_verify_rejects_invalid_signature() {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let public_key = RsaPublicKey::from(&private_key);
        let public_pem = public_key
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap();

        let mut manifest = create_test_manifest();
        manifest.public_key = public_pem;

        let manifest_json = manifest.canonical_json();
        let fake_signature = vec![0u8; 256];

        let result = PIVSigner::verify_manifest(&manifest_json, &fake_signature).unwrap();
        assert!(!result, "Invalid signature should fail verification");
    }
}
