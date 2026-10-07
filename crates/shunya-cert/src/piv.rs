use pcsc::{Card, Context, Protocols, Scope, ShareMode};
use x509_parser::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum PivError {
    #[error("PCSC Error: {0}")]
    Pcsc(#[from] pcsc::Error),
    #[error("SmartCard returned error status: {0:04X}")]
    CardError(u16),
    #[error("No SmartCard readers found")]
    NoReaders,
    #[error("Failed to parse certificate: {0}")]
    CertParse(String),
    #[error("Failed to extract public key: {0}")]
    PublicKey(String),
}

// APDU Commands for PIV (NIST SP 800-73)
const SELECT_PIV_APPLET: &[u8] = &[
    0x00, 0xA4, 0x04, 0x00, 0x09,
    0xA0, 0x00, 0x00, 0x03, 0x08, 0x00, 0x00, 0x10, 0x00,
];

// GET DATA command for slot 9C certificate
const GET_CERT_9C: &[u8] = &[
    0x00, 0xCB, 0x3F, 0xFF, 0x05, 0x5C, 0x03, 0x5F, 0xC1, 0x0A, 0x00,
];

// SHA-256 DigestInfo DER prefix (19 bytes)
// SEQUENCE { SEQUENCE { OID sha256, NULL }, OCTET STRING }
const SHA256_DIGEST_INFO_PREFIX: &[u8] = &[
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01,
    0x05, 0x00, 0x04, 0x20,
];

pub struct PivToken {
    card: Card,
}

impl PivToken {
    pub fn connect() -> Result<Self, PivError> {
        let ctx = Context::establish(Scope::User)?;

        let mut readers_buf = [0; 2048];
        let mut readers = ctx.list_readers(&mut readers_buf)?;
        let first_reader = readers.next().ok_or(PivError::NoReaders)?;

        let card = ctx.connect(first_reader, ShareMode::Shared, Protocols::ANY)?;

        let mut token = PivToken { card };
        token.transmit_apdu(SELECT_PIV_APPLET)?;

        Ok(token)
    }

    fn transmit_apdu(&mut self, cmd: &[u8]) -> Result<Vec<u8>, PivError> {
        let mut resp_buf = [0; 2048];
        let resp = self.card.transmit(cmd, &mut resp_buf)?;

        if resp.len() < 2 {
            return Err(PivError::CardError(0xFFFF));
        }

        let status = ((resp[resp.len() - 2] as u16) << 8) | (resp[resp.len() - 1] as u16);
        if status != 0x9000 {
            return Err(PivError::CardError(status));
        }

        Ok(resp[..resp.len() - 2].to_vec())
    }

    /// Sends a VERIFY APDU with the PIN to unlock the signing keys
    pub fn verify_pin(&mut self, pin: &str) -> Result<(), PivError> {
        let mut pin_bytes = [0xFF; 8];
        let bytes = pin.as_bytes();

        for (i, &b) in bytes.iter().enumerate().take(8) {
            pin_bytes[i] = b;
        }

        let mut cmd = vec![0x00, 0x20, 0x00, 0x80, 0x08];
        cmd.extend_from_slice(&pin_bytes);

        self.transmit_apdu(&cmd)?;
        Ok(())
    }

    /// Performs a cryptographic signature on a hash using the PIV GENERAL AUTHENTICATE command.
    /// Key slot 9C is used for Digital Signature.
    /// Algorithm 06 is RSA-2048, 11 is ECDSA P-256.
    pub fn sign_data(&mut self, hash: &[u8]) -> Result<Vec<u8>, PivError> {
        // Try ECDSA P-256 first (algorithm 11) — raw hash, no wrapping needed
        if let Ok(sig) = self.sign_with_algorithm(hash, 0x11) {
            return Ok(sig);
        }

        // Fall back to RSA-2048 (algorithm 06) — hash must be wrapped in DigestInfo
        let digest_info = Self::wrap_hash_in_digest_info(hash);
        self.sign_with_algorithm(&digest_info, 0x06)
    }

    fn sign_with_algorithm(&mut self, data: &[u8], algorithm: u8) -> Result<Vec<u8>, PivError> {
        // Construct the Dynamic Authentication Template (Tag 7C)
        let mut tlv_data = Vec::new();
        // Tag 82: Empty (Request signature)
        tlv_data.extend_from_slice(&[0x82, 0x00]);
        // Tag 81: Challenge (The hash/digest info)
        tlv_data.push(0x81);
        if data.len() > 127 {
            // Extended length encoding
            tlv_data.push(0x81);
            tlv_data.push(data.len() as u8);
        } else {
            tlv_data.push(data.len() as u8);
        }
        tlv_data.extend_from_slice(data);

        // Build the full APDU: GENERAL AUTHENTICATE
        // 0x00 0x87 <Algo> <Slot> <Lc> <Data> <Le>
        let mut cmd = vec![0x00, 0x87, algorithm, 0x9C, tlv_data.len() as u8];
        cmd.extend(tlv_data);
        cmd.push(0x00); // Le

        let resp = self.transmit_apdu(&cmd)?;

        // Parse the response TLV: 7C <len> [82 <len> <signature>]
        Self::extract_signature_from_response(&resp)
    }

    fn extract_signature_from_response(resp: &[u8]) -> Result<Vec<u8>, PivError> {
        if resp.len() < 4 || resp[0] != 0x7C {
            return Err(PivError::CardError(0x6A80));
        }

        // Determine the length of the 7C template
        let template_len = if resp[1] & 0x80 == 0 {
            resp[1] as usize
        } else {
            let num_len_bytes = (resp[1] & 0x7F) as usize;
            if resp.len() < 2 + num_len_bytes {
                return Err(PivError::CardError(0x6A80));
            }
            let mut len = 0usize;
            for i in 0..num_len_bytes {
                len = (len << 8) | resp[2 + i] as usize;
            }
            len
        };

        let template_start = if resp[1] & 0x80 == 0 { 2 } else { 2 + (resp[1] & 0x7F) as usize };
        let template_end = template_start + template_len;

        if template_end > resp.len() {
            return Err(PivError::CardError(0x6A80));
        }

        // Inside the 7C template, find tag 82 (signature)
        let inner = &resp[template_start..template_end];
        if inner.len() < 2 || inner[0] != 0x82 {
            return Err(PivError::CardError(0x6A80));
        }

        let sig_len = if inner[1] & 0x80 == 0 {
            inner[1] as usize
        } else {
            let num_len_bytes = (inner[1] & 0x7F) as usize;
            if inner.len() < 2 + num_len_bytes {
                return Err(PivError::CardError(0x6A80));
            }
            let mut len = 0usize;
            for i in 0..num_len_bytes {
                len = (len << 8) | inner[2 + i] as usize;
            }
            len
        };

        let sig_start = if inner[1] & 0x80 == 0 { 2 } else { 2 + (inner[1] & 0x7F) as usize };
        let sig_end = sig_start + sig_len;

        if sig_end > inner.len() {
            return Err(PivError::CardError(0x6A80));
        }

        Ok(inner[sig_start..sig_end].to_vec())
    }

    /// Reads the certificate from PIV slot 9C and extracts the public key as PEM.
    pub fn get_public_key_pem(&mut self) -> Result<String, PivError> {
        let resp = self.transmit_apdu(GET_CERT_9C)?;

        // Parse TLV: 71 <len> <cert> 72 <len> <cert_info>
        let cert_der = Self::extract_tag(&resp, 0x71)
            .ok_or_else(|| PivError::CertParse("Certificate tag 71 not found".to_string()))?;

        let (_, cert) = X509Certificate::from_der(cert_der)
            .map_err(|e| PivError::CertParse(e.to_string()))?;

        let spki = cert.public_key();
        let algorithm_oid = spki.algorithm.algorithm.to_id_string();

        // Reconstruct the public key PEM based on algorithm
        let pem = if algorithm_oid == "1.2.840.113549.1.1.1" {
            // RSA
            use rsa::pkcs1::DecodeRsaPublicKey;
            use rsa::pkcs8::EncodePublicKey;
            let rsa_key = rsa::RsaPublicKey::from_pkcs1_der(&spki.subject_public_key.data)
                .map_err(|e| PivError::PublicKey(e.to_string()))?;
            rsa_key
                .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
                .map_err(|e| PivError::PublicKey(e.to_string()))?
        } else if algorithm_oid == "1.2.840.10045.2.1" {
            // ECDSA — determine curve from the raw key length
            use p256::pkcs8::EncodePublicKey;
            use p256::ecdsa::VerifyingKey;
            let vk = VerifyingKey::from_sec1_bytes(&spki.subject_public_key.data)
                .map_err(|e| PivError::PublicKey(e.to_string()))?;
            vk.to_public_key_pem(p256::pkcs8::LineEnding::LF)
                .map_err(|e| PivError::PublicKey(e.to_string()))?
        } else {
            return Err(PivError::PublicKey(format!(
                "Unsupported algorithm OID: {}",
                algorithm_oid
            )));
        };

        Ok(pem)
    }

    fn extract_tag(data: &[u8], tag: u8) -> Option<&[u8]> {
        let mut offset = 0;
        while offset + 2 <= data.len() {
            if data[offset] == tag {
                let len = if data[offset + 1] & 0x80 == 0 {
                    data[offset + 1] as usize
                } else {
                    let num_len_bytes = (data[offset + 1] & 0x7F) as usize;
                    if offset + 2 + num_len_bytes > data.len() {
                        return None;
                    }
                    let mut len = 0usize;
                    for i in 0..num_len_bytes {
                        len = (len << 8) | data[offset + 2 + i] as usize;
                    }
                    len
                };
                let start = offset + 2;
                let end = start + len;
                if end <= data.len() {
                    return Some(&data[start..end]);
                }
                return None;
            }
            // Skip this TLV
            let len = if data[offset + 1] & 0x80 == 0 {
                data[offset + 1] as usize
            } else {
                let num_len_bytes = (data[offset + 1] & 0x7F) as usize;
                if offset + 2 + num_len_bytes > data.len() {
                    return None;
                }
                let mut len = 0usize;
                for i in 0..num_len_bytes {
                    len = (len << 8) | data[offset + 2 + i] as usize;
                }
                len
            };
            offset += 2 + len;
        }
        None
    }

    fn wrap_hash_in_digest_info(hash: &[u8]) -> Vec<u8> {
        let mut result = Vec::with_capacity(SHA256_DIGEST_INFO_PREFIX.len() + hash.len());
        result.extend_from_slice(SHA256_DIGEST_INFO_PREFIX);
        result.extend_from_slice(hash);
        result
    }
}
