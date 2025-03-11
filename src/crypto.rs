use anyhow::{Result, Context};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use std::fs::{read, write};
use std::path::Path;

pub fn encrypt_file(path: &Path, key: &str) -> Result<()> {
    let mut data = read(path)?;
    let rand = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rand.fill(&mut nonce_bytes)
        .map_err(|e| anyhow::anyhow!("RNG error: {:?}", e))?;

    let mut key_bytes = vec![0u8; 32];
    let input_bytes = key.as_bytes();
    key_bytes[..input_bytes.len().min(32)].copy_from_slice(&input_bytes[..input_bytes.len().min(32)]);

    let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
        .map_err(|e| anyhow::anyhow!("Key error: {:?}", e))?;
    let key = LessSafeKey::new(unbound_key);
    key.seal_in_place_append_tag(
        Nonce::try_assume_unique_for_key(&nonce_bytes)
            .map_err(|e| anyhow::anyhow!("Nonce error: {:?}", e))?,
        Aad::empty(),
        &mut data,
    )
    .map_err(|e| anyhow::anyhow!("Encryption error: {:?}", e))?;

    let mut encrypted_data = nonce_bytes.to_vec();
    encrypted_data.extend_from_slice(&data);
    write(path, encrypted_data)?;
    Ok(())
}

pub fn decrypt_file(path: &Path, key: &str) -> Result<()> {
    let encrypted_data = read(path)?;
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);

    let mut key_bytes = vec![0u8; 32];
    let input_bytes = key.as_bytes();
    key_bytes[..input_bytes.len().min(32)].copy_from_slice(&input_bytes[..input_bytes.len().min(32)]);

    let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
        .map_err(|e| anyhow::anyhow!("Key error: {:?}", e))?;
    let key = LessSafeKey::new(unbound_key);
    let mut data = ciphertext.to_vec();
    let plaintext = key
        .open_in_place(
            Nonce::try_assume_unique_for_key(nonce_bytes)
                .map_err(|e| anyhow::anyhow!("Nonce error: {:?}", e))?,
            Aad::empty(),
            &mut data,
        )
        .map_err(|e| anyhow::anyhow!("Decryption error: {:?}", e))?;

    write(path, plaintext)?;
    Ok(())
}