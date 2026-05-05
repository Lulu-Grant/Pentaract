use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand_core::{OsRng, RngCore};

use crate::errors::{PentaractError, PentaractResult};

const MAGIC: &[u8] = b"PENTARACTENC01";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Clone, Debug)]
pub struct EncryptionKey([u8; KEY_LEN]);

impl EncryptionKey {
    pub fn from_hex(input: &str) -> PentaractResult<Self> {
        let input = input.trim();
        if input.len() != KEY_LEN * 2 {
            return Err(PentaractError::InvalidEncryptionKey);
        }

        let mut key = [0u8; KEY_LEN];
        for (i, chunk) in input.as_bytes().chunks_exact(2).enumerate() {
            let high = Self::hex_value(chunk[0])?;
            let low = Self::hex_value(chunk[1])?;
            key[i] = (high << 4) | low;
        }

        Ok(Self(key))
    }

    fn hex_value(byte: u8) -> PentaractResult<u8> {
        match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            b'A'..=b'F' => Ok(byte - b'A' + 10),
            _ => Err(PentaractError::InvalidEncryptionKey),
        }
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new_from_slice(&self.0).expect("validated key length")
    }
}

pub struct FileCipher {
    key: EncryptionKey,
}

impl FileCipher {
    pub fn new(key: EncryptionKey) -> Self {
        Self { key }
    }

    pub fn encrypt_chunk(&self, plaintext: &[u8]) -> PentaractResult<Vec<u8>> {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);

        let ciphertext = self
            .key
            .cipher()
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|e| {
                tracing::error!("file chunk encryption failed: {e}");
                PentaractError::EncryptionError
            })?;

        let mut encrypted = Vec::with_capacity(MAGIC.len() + NONCE_LEN + ciphertext.len());
        encrypted.extend_from_slice(MAGIC);
        encrypted.extend_from_slice(&nonce);
        encrypted.extend_from_slice(&ciphertext);
        Ok(encrypted)
    }

    pub fn decrypt_chunk(&self, data: &[u8]) -> PentaractResult<Vec<u8>> {
        if !data.starts_with(MAGIC) {
            tracing::warn!("downloaded a legacy unencrypted file chunk");
            return Ok(data.to_vec());
        }

        let payload = &data[MAGIC.len()..];
        if payload.len() < NONCE_LEN {
            return Err(PentaractError::DecryptionError);
        }

        let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
        self.key
            .cipher()
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|e| {
                tracing::error!("file chunk decryption failed: {e}");
                PentaractError::DecryptionError
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{EncryptionKey, FileCipher};

    #[test]
    fn encrypts_and_decrypts_chunk() {
        let key = EncryptionKey::from_hex(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .unwrap();
        let cipher = FileCipher::new(key);
        let plaintext = b"hello, encrypted telegram storage";

        let encrypted = cipher.encrypt_chunk(plaintext).unwrap();

        assert_ne!(encrypted, plaintext);
        assert_eq!(cipher.decrypt_chunk(&encrypted).unwrap(), plaintext);
    }

    #[test]
    fn rejects_invalid_keys() {
        assert!(EncryptionKey::from_hex("short").is_err());
        assert!(EncryptionKey::from_hex(
            "zz0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e"
        )
        .is_err());
    }
}
