use aes_gcm::{
    aead::{generic_array::typenum::U16, Aead, KeyInit, Payload},
    aes::Aes256,
    AesGcm, Nonce,
};
use oxiroute_core::{OxiError, Result};
use scrypt::{scrypt, Params};

const STATIC_SALT: &[u8] = b"omniroute-field-encryption-v1";

type Aes256Gcm16ByteNonce = AesGcm<Aes256, U16>;

pub fn derive_key(secret: &str) -> Result<[u8; 32]> {
    let mut key = [0u8; 32];
    let params = Params::new(14, 8, 1, 32)
        .map_err(|e| OxiError::Auth(format!("Scrypt params error: {}", e)))?;

    scrypt(secret.as_bytes(), STATIC_SALT, &params, &mut key)
        .map_err(|e| OxiError::Auth(format!("Scrypt derivation failed: {}", e)))?;

    Ok(key)
}

pub fn decrypt_field(encrypted_str: &str, key: &[u8; 32]) -> Result<String> {
    if !encrypted_str.starts_with("enc:v1:") {
        return Ok(encrypted_str.to_string());
    }

    let raw = &encrypted_str[7..];
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 3 {
        return Err(OxiError::Auth("Invalid encrypted field format".to_string()));
    }

    let iv_bytes = hex::decode(parts[0])
        .map_err(|e| OxiError::Auth(format!("Invalid IV hex: {}", e)))?;
    let ciphertext_bytes = hex::decode(parts[1])
        .map_err(|e| OxiError::Auth(format!("Invalid ciphertext hex: {}", e)))?;
    let tag_bytes = hex::decode(parts[2])
        .map_err(|e| OxiError::Auth(format!("Invalid tag hex: {}", e)))?;

    if iv_bytes.len() != 16 {
        return Err(OxiError::Auth(format!("Expected 16-byte IV, got {}", iv_bytes.len())));
    }

    let cipher = Aes256Gcm16ByteNonce::new_from_slice(key)
        .map_err(|e| OxiError::Auth(format!("AES-GCM init error: {}", e)))?;

    let nonce = Nonce::<U16>::from_slice(&iv_bytes);

    let mut combined = ciphertext_bytes;
    combined.extend_from_slice(&tag_bytes);

    let payload = Payload {
        msg: &combined,
        aad: &[],
    };

    let plaintext_bytes = cipher
        .decrypt(nonce, payload)
        .map_err(|e| OxiError::Auth(format!("Decryption failed: {}", e)))?;

    String::from_utf8(plaintext_bytes)
        .map_err(|e| OxiError::Auth(format!("Invalid UTF-8 in decrypted field: {}", e)))
}
