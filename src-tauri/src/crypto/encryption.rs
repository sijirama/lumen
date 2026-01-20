//INFO: Encryption utilities for Lumen
//NOTE: Uses AES-256-GCM for encrypting sensitive data before storing in database

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use std::path::PathBuf;

//INFO: Length of the encryption key in bytes (256 bits)
const KEY_LENGTH: usize = 32;

//INFO: Length of the nonce in bytes (96 bits for GCM)
const NONCE_LENGTH: usize = 12;

//INFO: Gets the path to the encryption key file
fn get_key_file_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("Failed to get config directory")?;
    Ok(config_dir.join("lumen").join(".key"))
}

//INFO: Gets or creates the encryption key
//NOTE: Key is stored in a separate file in the config directory
//NOTE: This is a simple approach - for production, consider using OS keyring
pub fn get_or_create_encryption_key() -> Result<[u8; KEY_LENGTH]> {
    let key_path = get_key_file_path()?;

    //INFO: Check if key file exists
    if key_path.exists() {
        //INFO: Read existing key
        let key_bytes = std::fs::read(&key_path).context("Failed to read encryption key")?;

        if key_bytes.len() != KEY_LENGTH {
            return Err(anyhow!("Invalid encryption key length"));
        }

        let mut key = [0u8; KEY_LENGTH];
        key.copy_from_slice(&key_bytes);
        Ok(key)
    } else {
        //INFO: Generate new key
        let mut key = [0u8; KEY_LENGTH];
        OsRng.fill_bytes(&mut key);

        //INFO: Ensure parent directory exists
        if let Some(parent) = key_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create key directory")?;
        }

        //INFO: Save key to file
        std::fs::write(&key_path, &key).context("Failed to write encryption key")?;

        Ok(key)
    }
}

//INFO: Encrypts a plaintext token using AES-256-GCM
//NOTE: Returns base64-encoded ciphertext with nonce prepended
pub fn encrypt_token(plaintext: &str) -> Result<String> {
    let key = get_or_create_encryption_key()?;

    //INFO: Create cipher instance
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

    //INFO: Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    //INFO: Encrypt the plaintext
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    //INFO: Combine nonce and ciphertext, then base64 encode
    let mut combined = Vec::with_capacity(NONCE_LENGTH + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&combined))
}

//INFO: Decrypts a base64-encoded ciphertext
//NOTE: Expects nonce to be prepended to ciphertext
pub fn decrypt_token(encrypted: &str) -> Result<String> {
    let key = get_or_create_encryption_key()?;

    //INFO: Decode base64
    let combined = BASE64
        .decode(encrypted)
        .context("Failed to decode base64")?;

    //INFO: Ensure we have at least nonce + some ciphertext
    if combined.len() < NONCE_LENGTH + 1 {
        return Err(anyhow!("Encrypted data too short"));
    }

    //INFO: Split nonce and ciphertext
    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LENGTH);
    let nonce = Nonce::from_slice(nonce_bytes);

    //INFO: Create cipher instance
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

    //INFO: Decrypt
    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("Decryption failed: {}", e))?;

    String::from_utf8(plaintext_bytes).context("Decrypted data is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let original = "test-api-key-12345";
        let encrypted = encrypt_token(original).unwrap();
        let decrypted = decrypt_token(&encrypted).unwrap();
        assert_eq!(original, decrypted);
    }
}
