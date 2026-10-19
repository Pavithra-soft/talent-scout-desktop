use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::db;
use crate::privacy;

pub struct MailStatus {
    pub connected: bool,
    pub expired: bool,
}

pub fn status() -> Result<MailStatus, String> {
    match privacy::read_mail_token()? {
        None => Ok(MailStatus { connected: false, expired: false }),
        Some((_, until)) => {
            let now = now_secs();
            if until < now {
                Ok(MailStatus { connected: false, expired: true })
            } else {
                Ok(MailStatus { connected: true, expired: false })
            }
        }
    }
}

pub fn connect(conn: &Connection) -> Result<(), String> {
    let until = now_secs() + 30 * 24 * 60 * 60;
    privacy::store_mail_token("local-draft-only", until)?;
    db::audit(conn, "email connected", "drafts only, nothing sent")?;
    Ok(())
}

pub fn disconnect(conn: &Connection) -> Result<(), String> {
    privacy::clear_mail_token()?;
    db::audit(conn, "email disconnected", "drafts kept on this computer")?;
    Ok(())
}

pub fn save_drafts(conn: &Connection, folder: &Path, subject: &str, body: &str, people: &[(String, String)]) -> Result<u32, String> {
    let current = status()?;
    if current.expired {
        return Err("Your email login has expired. Connect again. Nothing was sent.".to_string());
    }
    if !current.connected {
        return Err("Connect email before writing a draft. Nothing was sent.".to_string());
    }
    if people.is_empty() {
        return Err("Choose at least one candidate, then save the draft. Nothing was sent.".to_string());
    }
    if subject.trim().is_empty() || body.trim().is_empty() {
        return Err("Add a subject and a message, then save the draft. Nothing was sent.".to_string());
    }
    fs::create_dir_all(folder).map_err(|_| "The draft folder could not be created. Choose another folder and try again.".to_string())?;
    let mut saved = 0u32;
    for (name, email) in people {
        if email.trim().is_empty() {
            continue;
        }
        let safe = email.replace(['/', '\\', ':'], "-");
        let path = folder.join(format!("{safe}.eml"));
        let message = format!(
            "X-Unsent: 1\r\nTo: {email}\r\nSubject: {subject}\r\n\r\nHello {name},\r\n\r\n{body}\r\n"
        );
        fs::write(&path, message).map_err(|_| "A draft could not be saved. Close the file if it is open, then try again.".to_string())?;
        saved += 1;
    }
    if saved == 0 {
        return Err("The selected candidates have no email address. Nothing was sent.".to_string());
    }
    db::audit(conn, "draft saved", &format!("drafts={saved}"))?;
    Ok(saved)
}

pub fn draft_folder(app_data: &Path) -> PathBuf {
    app_data.join("drafts")
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|item| item.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_are_saved_and_never_marked_sent() {
        let _ = privacy::clear_mail_token();
        let dir = std::env::temp_dir().join(format!("scout-drafts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let db_path = dir.join("scout.db");
        fs::create_dir_all(&dir).unwrap();
        let conn = db::open_db(&db_path).unwrap();
        let expired = save_drafts(&conn, &dir, "Hello", "We liked your resume.", &[("Ananya Rao".into(), "ananya@example.com".into())]);
        assert!(expired.unwrap_err().contains("Connect email"));
        connect(&conn).unwrap();
        privacy::store_mail_token("local-draft-only", 1).unwrap();
        let expired = save_drafts(&conn, &dir, "Hello", "We liked your resume.", &[("Ananya Rao".into(), "ananya@example.com".into())]);
        assert!(expired.unwrap_err().contains("expired"));
        connect(&conn).unwrap();
        let count = save_drafts(
            &conn,
            &dir,
            "Hello",
            "We liked your resume.",
            &[("Ananya Rao".into(), "ananya@example.com".into())],
        )
        .unwrap();
        assert_eq!(count, 1);
        let text = fs::read_to_string(dir.join("ananya@example.com.eml")).unwrap();
        assert!(text.contains("X-Unsent: 1"));
        assert!(!text.to_lowercase().contains("status: sent"));
        disconnect(&conn).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }
}
