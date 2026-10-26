use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use std::path::{Path, PathBuf};

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

static KEY_FILE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn config_dir() -> Option<PathBuf> {
    KEY_FILE.get().and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
}

pub fn set_key_dir(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let _ = KEY_FILE.set(dir.join("search.key"));
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
fn key_path() -> PathBuf {
    if let Some(path) = KEY_FILE.get() {
        return path.clone();
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("com.talentscout.desktop").join("search.key");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("Library/Application Support/com.talentscout.desktop/search.key")
}

#[cfg(not(test))]
fn load_or_create_key() -> Result<[u8; 32], String> {
    let path = key_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "This computer could not store the lock for saved resumes.".to_string())?;
    }
    if let Ok(existing) = std::fs::read_to_string(&path) {
        if let Ok(key) = decode_key(existing.trim()) {
            return Ok(key);
        }
    }
    let key = Aes256Gcm::generate_key(&mut OsRng);
    let encoded = hex::encode(key.as_slice());
    std::fs::write(&path, &encoded).map_err(|_| "This computer could not store the lock for saved resumes.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(key.as_slice());
    Ok(out)
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
fn mail_path() -> PathBuf {
    key_path().with_file_name("email-drafts.txt")
}

#[cfg(not(test))]
pub fn store_mail_token(token: &str, until: i64) -> Result<(), String> {
    std::fs::write(mail_path(), format!("{until}:{token}")).map_err(|_| "Email could not be connected on this computer. Try again.".to_string())
}

#[cfg(not(test))]
pub fn read_mail_token() -> Result<Option<(String, i64)>, String> {
    let Ok(raw) = std::fs::read_to_string(mail_path()) else {
        return Ok(None);
    };
    let (until, token) = raw.split_once(':').unwrap_or(("0", ""));
    let until = until.parse().unwrap_or(0);
    Ok(Some((token.to_string(), until)))
}

#[cfg(not(test))]
pub fn clear_mail_token() -> Result<(), String> {
    let _ = std::fs::remove_file(mail_path());
    Ok(())
}
