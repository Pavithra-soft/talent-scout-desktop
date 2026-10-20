use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};

const PREFIX: &str = "enc1:";

#[cfg(test)]
fn test_key() -> [u8; 32] {
    [9u8; 32]
}

pub fn field_key() -> Result<[u8; 32], String> {
    if let Ok(raw) = std::env::var("SCOUT_DATA_KEY") {
        let bytes = hex::decode(raw.trim()).map_err(|_| "The lock for saved resumes could not be read.".to_string())?;
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }
    }
    #[cfg(test)]
    {
        return Ok(test_key());
    }
    #[cfg(not(test))]
    {
        key_from_keyring()
    }
}

#[cfg(not(test))]
fn key_from_keyring() -> Result<[u8; 32], String> {
    static CACHED: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    if let Some(key) = CACHED.get() {
        return Ok(*key);
    }
    let key = load_or_create_key()?;
    let _ = CACHED.set(key);
    Ok(key)
}

#[cfg(not(test))]
fn load_or_create_key() -> Result<[u8; 32], String> {
    let entry = keyring::Entry::new("Talent Scout Desktop", "saved-search")
        .map_err(|_| "This computer could not open the lock for saved resumes. Restart the app and try again.".to_string())?;
    match entry.get_password() {
        Ok(existing) => decode_key(&existing),
        Err(_) => {
            let key = Aes256Gcm::generate_key(&mut OsRng);
            let encoded = hex::encode(key.as_slice());
            entry
                .set_password(&encoded)
                .map_err(|_| "This computer could not store the lock for saved resumes. Restart the app and try again.".to_string())?;
            let mut out = [0u8; 32];
            out.copy_from_slice(key.as_slice());
            Ok(out)
        }
    }
}

#[cfg(not(test))]
fn decode_key(existing: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(existing.trim()).map_err(|_| "The lock for saved resumes could not be read. Choose Start fresh.".to_string())?;
    if bytes.len() != 32 {
        return Err("The lock for saved resumes could not be read. Choose Start fresh.".to_string());
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

pub fn seal(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let key_bytes = field_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let cipher_text = cipher
        .encrypt(&nonce, value.as_bytes())
        .map_err(|_| "A candidate detail could not be saved. Choose Start fresh and read the folder again.".to_string())?;
    let mut packed = nonce.to_vec();
    packed.extend(cipher_text);
    Ok(format!("{PREFIX}{}", hex::encode(packed)))
}

pub fn open_field(value: &str) -> Result<String, String> {
    if value.is_empty() || !value.starts_with(PREFIX) {
        return Ok(value.to_string());
    }
    let packed = hex::decode(&value[PREFIX.len()..]).map_err(|_| "A saved candidate could not be unlocked. Choose Start fresh.".to_string())?;
    if packed.len() < 12 {
        return Err("A saved candidate could not be unlocked. Choose Start fresh.".to_string());
    }
    let (nonce_bytes, cipher_text) = packed.split_at(12);
    let key_bytes = field_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plain = cipher
        .decrypt(nonce, cipher_text)
        .map_err(|_| "A saved candidate could not be unlocked. Choose Start fresh.".to_string())?;
    String::from_utf8(plain).map_err(|_| "A saved candidate could not be unlocked. Choose Start fresh.".to_string())
}

#[cfg(test)]
static MAIL_LOCK: std::sync::Mutex<Option<(String, i64)>> = std::sync::Mutex::new(None);

#[cfg(test)]
pub fn store_mail_token(token: &str, until: i64) -> Result<(), String> {
    *MAIL_LOCK.lock().unwrap() = Some((token.to_string(), until));
    Ok(())
}

#[cfg(test)]
pub fn read_mail_token() -> Result<Option<(String, i64)>, String> {
    Ok(MAIL_LOCK.lock().unwrap().clone())
}

#[cfg(test)]
pub fn clear_mail_token() -> Result<(), String> {
    *MAIL_LOCK.lock().unwrap() = None;
    Ok(())
}

#[cfg(not(test))]
pub fn store_mail_token(token: &str, until: i64) -> Result<(), String> {
    let entry = keyring::Entry::new("Talent Scout Desktop", "email-drafts")
        .map_err(|_| "Email could not be connected on this computer. Try again.".to_string())?;
    entry
        .set_password(&format!("{until}:{token}"))
        .map_err(|_| "Email could not be connected on this computer. Try again.".to_string())
}

#[cfg(not(test))]
pub fn read_mail_token() -> Result<Option<(String, i64)>, String> {
    let entry = keyring::Entry::new("Talent Scout Desktop", "email-drafts")
        .map_err(|_| "Email could not be checked on this computer. Try again.".to_string())?;
    match entry.get_password() {
        Ok(raw) => {
            let (until, token) = raw.split_once(':').unwrap_or(("0", ""));
            let until = until.parse().unwrap_or(0);
            Ok(Some((token.to_string(), until)))
        }
        Err(_) => Ok(None),
    }
}

#[cfg(not(test))]
pub fn clear_mail_token() -> Result<(), String> {
    let entry = keyring::Entry::new("Talent Scout Desktop", "email-drafts")
        .map_err(|_| "Email could not be disconnected on this computer. Try again.".to_string())?;
    let _ = entry.delete_credential();
    Ok(())
}
