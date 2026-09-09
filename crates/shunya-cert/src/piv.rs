use pcsc::{Card, Context, Protocols, Scope, ShareMode};

#[derive(Debug, thiserror::Error)]
pub enum PivError {
    #[error("PCSC Error: {0}")]
    Pcsc(#[from] pcsc::Error),
    #[error("SmartCard returned error status: {0:04X}")]
    CardError(u16),
    #[error("No SmartCard readers found")]
    NoReaders,
}

// APDU Commands for PIV (NIST SP 800-73)
const SELECT_PIV_APPLET: &[u8] = &[
    0x00, 0xA4, 0x04, 0x00, 0x09, 
    0xA0, 0x00, 0x00, 0x03, 0x08, 0x00, 0x00, 0x10, 0x00,
];

pub struct PivToken {
    card: Card,
}

impl PivToken {
    pub fn connect() -> Result<Self, PivError> {
        let ctx = Context::establish(Scope::User)?;
        
        // List readers
        let mut readers_buf = [0; 2048];
        let mut readers = ctx.list_readers(&mut readers_buf)?;
        let first_reader = readers.next().ok_or(PivError::NoReaders)?;

        // Connect to the token in the first reader
        let card = ctx.connect(first_reader, ShareMode::Shared, Protocols::ANY)?;

        let mut token = PivToken { card };
        
        // Select the PIV Applet
        token.transmit_apdu(SELECT_PIV_APPLET)?;
        
        Ok(token)
    }

    /// Helper to transmit a raw APDU and validate the SW1/SW2 status words.
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

    /// Performs a cryptographic signature on a payload using the PIV GENERAL AUTHENTICATE command.
    /// Key slot 9C is commonly used for Digital Signature. 
    /// Algorithm 06 is RSA-2048, 11 is ECDSA P-256.
    pub fn sign_data(&mut self, hash: &[u8]) -> Result<Vec<u8>, PivError> {
        // Construct the Dynamic Authentication Template (Tag 7C)
        let mut tlv_data = Vec::new();
        // Tag 82: Empty (Request signature)
        tlv_data.extend_from_slice(&[0x82, 0x00]); 
        // Tag 81: Challenge (The hash)
        tlv_data.push(0x81);
        tlv_data.push(hash.len() as u8);
        tlv_data.extend_from_slice(hash);

        // Build the full APDU: GENERAL AUTHENTICATE
        // 0x00 0x87 <Algo> <Slot> <Lc> <Data> <Le>
        let mut cmd = vec![0x00, 0x87, 0x06, 0x9C, tlv_data.len() as u8];
        cmd.extend(tlv_data);
        cmd.push(0x00); // Le

        let resp = self.transmit_apdu(&cmd)?;

        // The response is wrapped in a Tag 7C template, containing Tag 82 with the signature.
        if resp.len() > 4 && resp[0] == 0x7C {
            // Very naive TLV parsing
            let mut offset = 2; // Skip 7C and length
            if resp[1] == 0x81 || resp[1] == 0x82 { offset = 3; } // Extended length

            if resp[offset] == 0x82 {
                offset += 1;
                let sig_len = resp[offset] as usize;
                offset += 1;
                if resp.len() >= offset + sig_len {
                    return Ok(resp[offset..offset + sig_len].to_vec());
                }
            }
        }

        // Fallback if parsing fails (or just return the whole envelope for debugging)
        Ok(resp)
    }
}
