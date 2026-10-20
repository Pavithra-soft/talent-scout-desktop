use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::db;
use crate::privacy;

pub struct MailStatus {
    pub connected: bool,
    pub expired: bool,
    pub address: String,
}

pub fn status() -> Result<MailStatus, String> {
    match privacy::read_mail_token()? {
        None => Ok(MailStatus { connected: false, expired: false, address: String::new() }),
        Some((token, until)) => {
            let address = if looks_like_email(&token) { token } else { String::new() };
            let now = now_secs();
            if until < now {
                Ok(MailStatus { connected: false, expired: !address.is_empty(), address })
            } else if address.is_empty() {
                Ok(MailStatus { connected: false, expired: false, address })
            } else {
                Ok(MailStatus { connected: true, expired: false, address })
            }
        }
    }
}

pub fn connect(conn: &Connection, email: &str) -> Result<(), String> {
    let email = email.trim();
    if !looks_like_email(email) {
        return Err("Enter the email address you already use in Mail or Outlook.".to_string());
    }
    let until = now_secs() + 30 * 24 * 60 * 60;
    privacy::store_mail_token(email, until)?;
    db::audit(conn, "email connected", email)?;
    Ok(())
}

pub fn disconnect(conn: &Connection) -> Result<(), String> {
    privacy::clear_mail_token()?;
    db::audit(conn, "email disconnected", "drafts kept on this computer")?;
    Ok(())
}

#[derive(Clone)]
pub struct DraftPerson {
    pub name: String,
    pub email: String,
    pub skills: String,
    pub matching: String,
    pub years: Option<i32>,
    pub location: String,
    pub company: String,
}

pub fn save_drafts(conn: &Connection, folder: &Path, subject: &str, instructions: &str, jd: &str, people: &[DraftPerson]) -> Result<u32, String> {
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
    if jd.trim().is_empty() {
        return Err("Paste the job first. The draft is written from that job and the candidate. Nothing was sent.".to_string());
    }
    fs::create_dir_all(folder).map_err(|_| "The draft folder could not be created. Choose another folder and try again.".to_string())?;
    let from = status()?.address;
    let mut saved = 0u32;
    for person in people {
        let email = person.email.trim();
        if email.is_empty() {
            continue;
        }
        let letter = crate::letter::write_letter(
            jd,
            instructions,
            subject,
            &crate::letter::Person {
                name: person.name.clone(),
                skills: person.skills.clone(),
                matching: person.matching.clone(),
                years: person.years,
                location: person.location.clone(),
                company: person.company.clone(),
            },
        );
        let safe = email.replace(['/', '\\', ':'], "-");
        let path = folder.join(format!("{safe}.eml"));
        let message = format!(
            "X-Unsent: 1\r\nFrom: {from}\r\nTo: {email}\r\nSubject: {}\r\n\r\n{}\r\n",
            one_line(&letter.subject),
            letter.body
        );
        fs::write(&path, message).map_err(|_| "A draft could not be saved. Close the file if it is open, then try again.".to_string())?;
        hand_to_mail(&from, &person.name, email, &letter.subject, &letter.body, &path)?;
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

fn looks_like_email(value: &str) -> bool {
    let value = value.trim();
    let Some((name, host)) = value.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && !name.contains(' ')
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains(' ')
}

fn one_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_string()
}

fn hand_to_mail(from: &str, name: &str, email: &str, subject: &str, content: &str, eml: &Path) -> Result<(), String> {
    if cfg!(test) {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let script = apple_draft_script(from, name, email, subject, content);
        if run_osascript(&script).is_ok() {
            return Ok(());
        }
        return open_in_mail(eml);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = eml;
        let script = outlook_draft_script(email, subject, content);
        return run_outlook(&script);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (from, email, subject, content, eml);
        Ok(())
    }
}

fn open_in_mail(path: &Path) -> Result<(), String> {
    let status = Command::new("open")
        .arg("-a")
        .arg("Mail")
        .arg(path)
        .status()
        .map_err(|_| "Mail could not be opened. Open the Mail app, sign in, and try again. Nothing was sent.".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Allow Talent Scout Desktop to control Mail when macOS asks, then prepare the drafts again. Nothing was sent.".to_string())
    }
}

fn apple_draft_script(from: &str, name: &str, email: &str, subject: &str, content: &str) -> String {
    format!(
        "tell application \"Mail\"\nactivate\nset theMessage to make new outgoing message with properties {{sender:\"{from}\", subject:\"{subject}\", content:\"{content}\", visible:true}}\ntell theMessage\nmake new to recipient at end of to recipients with properties {{name:\"{name}\", address:\"{email}\"}}\nend tell\nsave theMessage\nend tell\n",
        from = apple_text(from),
        name = apple_text(name),
        email = apple_text(email),
        subject = apple_text(subject),
        content = apple_text(content),
    )
}

fn apple_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "")
        .replace('\n', "\" & return & \"")
}

fn run_osascript(script: &str) -> Result<(), String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|_| "Mail could not be opened. Open the Mail app, sign in, and try again. Nothing was sent.".to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&output.stderr);
    if err.contains("-1743") || err.to_lowercase().contains("not authorized") {
        return Err("Allow Talent Scout Desktop to control Mail when macOS asks, then prepare the drafts again. Nothing was sent.".to_string());
    }
    Err("Mail could not prepare the draft. Sign in to Mail with your email and try again. Nothing was sent.".to_string())
}

fn outlook_draft_script(email: &str, subject: &str, content: &str) -> String {
    format!(
        "$outlook = New-Object -ComObject Outlook.Application; $mail = $outlook.CreateItem(0); $mail.To = '{email}'; $mail.Subject = '{subject}'; $mail.Body = '{content}'; $mail.Save()",
        email = outlook_text(email),
        subject = outlook_text(subject),
        content = outlook_text(content),
    )
}

fn outlook_text(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(target_os = "windows")]
fn run_outlook(script: &str) -> Result<(), String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|_| "Outlook could not be opened. Open Outlook, sign in, and try again. Nothing was sent.".to_string())?;
    if output.status.success() {
        return Ok(());
    }
    Err("Outlook could not prepare the draft. Sign in to Outlook and try again. Nothing was sent.".to_string())
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
        let person = DraftPerson {
            name: "Ananya Rao".into(),
            email: "ananya@example.com".into(),
            skills: "java, spring boot".into(),
            matching: "java, spring boot".into(),
            years: Some(8),
            location: "Bengaluru".into(),
            company: "Infosys".into(),
        };
        let jd = "Backend engineer, Bengaluru\nMust-have: Java, Spring Boot\n";
        let expired = save_drafts(&conn, &dir, "Hello", "We liked your resume.", jd, &[person.clone()]);
        assert!(expired.unwrap_err().contains("Connect email"));
        connect(&conn, "recruiter@example.com").unwrap();
        privacy::store_mail_token("recruiter@example.com", 1).unwrap();
        let expired = save_drafts(&conn, &dir, "Hello", "We liked your resume.", jd, &[person.clone()]);
        assert!(expired.unwrap_err().contains("expired"));
        connect(&conn, "recruiter@example.com").unwrap();
        let count = save_drafts(&conn, &dir, "Hello", "mention that the team is hiring now", jd, &[person]).unwrap();
        assert_eq!(count, 1);
        let text = fs::read_to_string(dir.join("ananya@example.com.eml")).unwrap();
        assert!(text.contains("X-Unsent: 1"));
        assert!(text.contains("From: recruiter@example.com"));
        assert!(text.contains("Hello Ananya,"));
        assert!(text.contains("Spring Boot"));
        assert!(text.contains("The team is hiring now."));
        assert!(!text.to_lowercase().contains("status: sent"));
        let script = apple_draft_script("recruiter@example.com", "Ananya Rao", "ananya@example.com", "Hello", "We liked your resume.");
        assert!(script.contains("activate"));
        assert!(script.contains("visible:true"));
        assert!(script.contains("save theMessage"));
        assert!(!script.contains("send theMessage"));
        let outlook = outlook_draft_script("ananya@example.com", "Hello", "We liked your resume.");
        assert!(outlook.contains(".Save()"));
        assert!(!outlook.contains(".Send("));
        disconnect(&conn).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }
}
