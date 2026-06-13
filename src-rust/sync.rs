use crate::config::{config_dir, config_path, save_config, backup_config, PiSwitchConfig};
use aes::cipher::{BlockEncrypt, KeyInit, BlockDecrypt};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use rand::Rng;
use sha2::{Sha256, Digest};
use std::path::PathBuf;

// ─── Encryption ───────────────────────────────────────────

fn derive_key(passphrase: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(passphrase.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

pub fn encrypt_config(passphrase: &str) -> Result<String, String> {
    if passphrase.len() < 8 {
        return Err("passphrase must be at least 8 characters".into());
    }

    let path = config_path();
    if !path.exists() {
        return Err(format!("No config at {}", path.display()));
    }

    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Read error: {}", e))?;

    let key = derive_key(passphrase);
    let iv: [u8; 16] = rand::thread_rng().gen();

    // PKCS7 padding
    let mut data = text.as_bytes().to_vec();
    let pad_len = 16 - (data.len() % 16);
    data.extend(std::iter::repeat(pad_len as u8).take(pad_len));

    // Encrypt using AES-256-CBC manually
    let cipher = aes::Aes256::new_from_slice(&key)
        .map_err(|e| format!("Cipher error: {:?}", e))?;

    for chunk in data.chunks_mut(16) {
        let mut block = aes::Block::default();
        block.copy_from_slice(chunk);
        let mut out = block;
        for i in 0..16 { out[i] ^= iv[i]; }
        cipher.encrypt_block(&mut out);
        chunk.copy_from_slice(&out);
    }

    let encrypted = serde_json::json!({
        "v": 1,
        "iv": B64.encode(&iv),
        "data": B64.encode(&data),
    });

    let export_dir = config_dir().join("exports");
    std::fs::create_dir_all(&export_dir)
        .map_err(|e| format!("Create dir error: {}", e))?;

    let ts = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S-%3fZ");
    let export_path = export_dir.join(format!("pi-switch-export-{}.json", ts));
    let json = serde_json::to_string_pretty(&encrypted)
        .map_err(|e| format!("Serialize error: {}", e))?;

    std::fs::write(&export_path, json)
        .map_err(|e| format!("Write error: {}", e))?;

    Ok(format!("Config exported to {}", export_path.display()))
}

pub fn import_config(file_path: &str, passphrase: &str) -> Result<String, String> {
    if passphrase.is_empty() {
        return Err("passphrase required".into());
    }

    let path = PathBuf::from(file_path);
    if !path.exists() {
        return Err(format!("file not found: {}", file_path));
    }

    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Read error: {}", e))?;

    let wrapper: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let v = wrapper.get("v").and_then(|v| v.as_u64()).unwrap_or(0);
    if v != 1 {
        return Err("unsupported encryption version".into());
    }

    let iv_b64 = wrapper.get("iv").and_then(|v| v.as_str()).unwrap_or("");
    let data_b64 = wrapper.get("data").and_then(|v| v.as_str()).unwrap_or("");

    let iv = B64.decode(iv_b64).map_err(|e| format!("IV decode: {}", e))?;
    let mut data = B64.decode(data_b64).map_err(|e| format!("Data decode: {}", e))?;

    if iv.len() != 16 {
        return Err("invalid IV length".into());
    }

    let key = derive_key(passphrase);
    let cipher = aes::Aes256::new_from_slice(&key)
        .map_err(|e| format!("Cipher error: {:?}", e))?;

    // Decrypt
    for chunk in data.chunks_mut(16) {
        let mut block = aes::Block::default();
        block.copy_from_slice(chunk);
        let mut out = block;
        cipher.decrypt_block(&mut out);
        for i in 0..16 { out[i] ^= iv[i]; }
        chunk.copy_from_slice(&out);
    }

    // Remove PKCS7 padding
    let pad_len = *data.last().unwrap_or(&0) as usize;
    if pad_len > 0 && pad_len <= 16 {
        data.truncate(data.len() - pad_len);
    }

    let config_text = String::from_utf8(data).map_err(|e| format!("UTF-8 error: {}", e))?;

    let mut new_config: PiSwitchConfig = serde_json::from_str(&config_text)
        .map_err(|e| format!("decrypted data is not valid JSON: {}", e))?;

    // Sanitize raw API keys
    let mut sanitized = 0u32;
    for name in new_config.profiles.keys().cloned().collect::<Vec<_>>() {
        let needs_sanitize = new_config.profiles.get(&name)
            .and_then(|v| v.get("apiKey"))
            .and_then(|v| v.as_str())
            .map_or(false, |key| !key.starts_with('$') && key.len() > 8);
        if needs_sanitize {
            if let Some(profile) = new_config.profiles.get_mut(&name) {
                profile["apiKey"] = serde_json::Value::String("$PI_SWITCH_IMPORTED_KEY".into());
                sanitized += 1;
            }
        }
    }

    // Backup existing config
    let _ = backup_config("pre-import");

    save_config(&new_config).map_err(|e| format!("Save error: {}", e))?;

    let profile_count = new_config.profiles.len();
    Ok(format!(
        "Imported {} profile(s) from {} ({} raw key(s) sanitized)",
        profile_count,
        path.file_name().unwrap_or_default().to_string_lossy(),
        sanitized,
    ))
}

pub fn export_dir() -> String {
    config_dir().join("exports").display().to_string()
}
