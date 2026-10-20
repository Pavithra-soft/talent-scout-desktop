use rusqlite::{params, Connection};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Stored {
    pub path: String,
    pub filename: String,
    pub size: i64,
    pub mtime: i64,
    pub content_hash: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub location: String,
    pub skills: String,
    pub company: String,
    pub years: Option<i32>,
    pub notice_raw: String,
    pub notice_days: Option<i32>,
    pub error: String,
    pub note: String,
    pub score: Option<i32>,
    pub matching: String,
    pub missing: String,
    pub rationale: String,
}

pub fn open_or_recover(path: &Path) -> Result<(Connection, String), String> {
    match open_db(path) {
        Ok(connection) => Ok((connection, String::new())),
        Err(_) => {
            let _ = std::fs::rename(path, path.with_extension("db.unreadable"));
            let connection = open_db(path)?;
            Ok((
                connection,
                "The saved search on this computer could not be opened, so a new one was started.".to_string(),
            ))
        }
    }
}

pub fn open_db(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let conn = Connection::open(path).map_err(|err| err.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS profiles (
            path TEXT PRIMARY KEY,
            root TEXT NOT NULL,
            size INTEGER NOT NULL,
            mtime INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            filename TEXT NOT NULL,
            name TEXT NOT NULL DEFAULT '',
            email TEXT NOT NULL DEFAULT '',
            phone TEXT NOT NULL DEFAULT '',
            location TEXT NOT NULL DEFAULT '',
            skills TEXT NOT NULL DEFAULT '',
            company TEXT NOT NULL DEFAULT '',
            years INTEGER,
            notice_raw TEXT NOT NULL DEFAULT '',
            notice_days INTEGER,
            error TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS scores (
            path TEXT PRIMARY KEY,
            score INTEGER,
            matching TEXT NOT NULL DEFAULT '',
            missing TEXT NOT NULL DEFAULT '',
            rationale TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            at TEXT NOT NULL,
            action TEXT NOT NULL,
            detail TEXT NOT NULL
        );
        ",
    )
    .map_err(|err| err.to_string())?;
    let _ = conn.execute("ALTER TABLE profiles ADD COLUMN note TEXT NOT NULL DEFAULT ''", []);
    Ok(conn)
}

fn hide(value: &str) -> Result<String, String> {
    crate::privacy::seal(value)
}

fn show(value: String) -> Result<String, String> {
    crate::privacy::open_field(&value)
}

fn reveal(value: String) -> (String, bool) {
    match show(value) {
        Ok(text) => (text, false),
        Err(_) => (String::new(), true),
    }
}

pub fn cached(conn: &Connection, path: &str) -> Option<(i64, i64, String, String)> {
    conn.query_row(
        "SELECT size, mtime, content_hash, error FROM profiles WHERE path = ?1",
        [path],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .ok()
}

pub fn hash_saved(conn: &Connection, root: &str, hash: &str) -> bool {
    if hash.is_empty() {
        return false;
    }
    conn.query_row(
        "SELECT 1 FROM profiles WHERE root = ?1 AND content_hash = ?2 AND error = '' LIMIT 1",
        params![root, hash],
        |_| Ok(1),
    )
    .is_ok()
}

pub fn touch_cache(conn: &Connection, path: &str, size: i64, mtime: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE profiles SET size = ?1, mtime = ?2 WHERE path = ?3",
        params![size, mtime, path],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn save_profile(conn: &Connection, root: &str, item: &Stored) -> Result<(), String> {
    conn.execute(
        "INSERT INTO profiles (
            path, root, size, mtime, content_hash, filename, name, email, phone, location,
            skills, company, years, notice_raw, notice_days, error, note
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
        ON CONFLICT(path) DO UPDATE SET
            root = excluded.root,
            size = excluded.size,
            mtime = excluded.mtime,
            content_hash = excluded.content_hash,
            filename = excluded.filename,
            name = excluded.name,
            email = excluded.email,
            phone = excluded.phone,
            location = excluded.location,
            skills = excluded.skills,
            company = excluded.company,
            years = excluded.years,
            notice_raw = excluded.notice_raw,
            notice_days = excluded.notice_days,
            error = excluded.error,
            note = excluded.note",
        params![
            item.path,
            root,
            item.size,
            item.mtime,
            item.content_hash,
            hide(&item.filename)?,
            hide(&item.name)?,
            hide(&item.email)?,
            hide(&item.phone)?,
            hide(&item.location)?,
            hide(&item.skills)?,
            hide(&item.company)?,
            item.years,
            hide(&item.notice_raw)?,
            item.notice_days,
            item.error,
            item.note
        ],
    )
    .map_err(|err| err.to_string())?;
    conn.execute("DELETE FROM scores WHERE path = ?1", [&item.path])
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn save_score(
    conn: &Connection,
    path: &str,
    score: i32,
    matching: &str,
    missing: &str,
    rationale: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO scores (path, score, matching, missing, rationale) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(path) DO UPDATE SET score = excluded.score, matching = excluded.matching, missing = excluded.missing, rationale = excluded.rationale",
        params![path, score, matching, missing, rationale],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn profiles_for_root(conn: &Connection, root: &str) -> Result<Vec<Stored>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT p.path, p.filename, p.size, p.mtime, p.content_hash, p.name, p.email, p.phone,
                    p.location, p.skills, p.company, p.years, p.notice_raw, p.notice_days, p.error, p.note,
                    s.score, s.matching, s.missing, s.rationale
             FROM profiles p
             LEFT JOIN scores s ON s.path = p.path
             WHERE p.root = ?1
             ORDER BY p.name",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([root], |row| {
            let path: String = row.get(0)?;
            let (filename, file_locked) = reveal(row.get(1)?);
            let (name, name_locked) = reveal(row.get(5)?);
            let (email, email_locked) = reveal(row.get(6)?);
            let (phone, phone_locked) = reveal(row.get(7)?);
            let (location, location_locked) = reveal(row.get(8)?);
            let (skills, skills_locked) = reveal(row.get(9)?);
            let (company, company_locked) = reveal(row.get(10)?);
            let (notice_raw, notice_locked) = reveal(row.get(12)?);
            let mut error: String = row.get(14)?;
            if error.is_empty()
                && (file_locked || name_locked || email_locked || phone_locked || location_locked || skills_locked || company_locked || notice_locked)
            {
                error = "This saved resume could not be unlocked. Click Start fresh and read the folder again.".to_string();
            }
            let filename = if filename.is_empty() {
                Path::new(&path)
                    .file_name()
                    .and_then(|item| item.to_str())
                    .unwrap_or("resume")
                    .to_string()
            } else {
                filename
            };
            Ok(Stored {
                path,
                filename,
                size: row.get(2)?,
                mtime: row.get(3)?,
                content_hash: row.get(4)?,
                name,
                email,
                phone,
                location,
                skills,
                company,
                years: row.get(11)?,
                notice_raw,
                notice_days: row.get(13)?,
                error,
                note: row.get(15).unwrap_or_default(),
                score: row.get(16)?,
                matching: row.get(17).unwrap_or_default(),
                missing: row.get(18).unwrap_or_default(),
                rationale: row.get(19).unwrap_or_default(),
            })
        })
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> String {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0))
        .unwrap_or_default()
}

pub fn profile_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
        .unwrap_or(0)
}

pub fn clear_profiles(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("DELETE FROM profiles; DELETE FROM scores;")
        .map_err(|err| err.to_string())
}

pub fn clear_search(conn: &Connection) -> Result<(), String> {
    clear_profiles(conn)?;
    conn.execute("DELETE FROM settings", [])
        .map_err(|err| err.to_string())?;
    audit(conn, "start fresh", "saved search cleared")
}

pub fn audit(conn: &Connection, action: &str, detail: &str) -> Result<(), String> {
    let at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    conn.execute(
        "INSERT INTO audit (at, action, detail) VALUES (?1, ?2, ?3)",
        params![at, action, detail],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn expire_if_old(conn: &Connection) -> Result<bool, String> {
    let saved = get_setting(conn, "saved_at");
    let Ok(saved) = saved.parse::<i64>() else {
        return Ok(false);
    };
    let now = chrono::Utc::now().timestamp();
    if now.saturating_sub(saved) <= 7 * 24 * 60 * 60 {
        return Ok(false);
    }
    clear_profiles(conn)?;
    set_setting(
        conn,
        "startup_note",
        "The previous search was more than 7 days old, so it was cleared from this computer.",
    )?;
    set_setting(conn, "saved_at", "")?;
    audit(conn, "expired", "cleared a search older than 7 days")?;
    Ok(true)
}

pub fn mark_duplicate_emails(conn: &Connection, root: &str) -> Result<(), String> {
    let rows = profiles_for_root(conn, root)?;
    let mut counts = std::collections::HashMap::<String, i32>::new();
    for row in &rows {
        if row.error.is_empty() && !row.email.is_empty() {
            *counts.entry(row.email.to_lowercase()).or_insert(0) += 1;
        }
    }
    for row in &rows {
        let note = if !row.email.is_empty() && counts.get(&row.email.to_lowercase()).copied().unwrap_or(0) > 1 {
            "Same email as another candidate in this search"
        } else {
            ""
        };
        conn.execute("UPDATE profiles SET note = ?1 WHERE path = ?2", params![note, row.path])
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_email_is_not_plain_text_and_start_fresh_removes_it() {
        let path = std::env::temp_dir().join(format!("scout-seal-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = open_db(&path).unwrap();
        save_profile(
            &conn,
            "/tmp/resumes",
            &Stored {
                path: "/tmp/resumes/a.txt".into(),
                filename: "a.txt".into(),
                size: 1,
                mtime: 1,
                content_hash: "abc".into(),
                name: "Ananya Rao".into(),
                email: "ananya.rao@example.com".into(),
                phone: "+919876543210".into(),
                location: "Bengaluru".into(),
                skills: "java".into(),
                company: "Infosys".into(),
                years: Some(8),
                notice_raw: "30 days".into(),
                notice_days: Some(30),
                error: String::new(),
                note: String::new(),
                score: None,
                matching: String::new(),
                missing: String::new(),
                rationale: String::new(),
            },
        )
        .unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(FULL);").ok();
        let bytes = std::fs::read(&path).unwrap_or_default();
        assert!(!bytes.windows(12).any(|window| window == b"@example.com"));
        let loaded = profiles_for_root(&conn, "/tmp/resumes").unwrap();
        assert_eq!(loaded[0].email, "ananya.rao@example.com");
        clear_search(&conn).unwrap();
        assert_eq!(profile_count(&conn), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn duplicate_emails_are_marked_and_an_old_search_expires() {
        let path = std::env::temp_dir().join(format!("scout-dup-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = open_db(&path).unwrap();
        for (file, email) in [("a.txt", "same@example.com"), ("b.txt", "same@example.com")] {
            save_profile(
                &conn,
                "/tmp/resumes",
                &Stored {
                    path: format!("/tmp/resumes/{file}"),
                    filename: file.into(),
                    size: 1,
                    mtime: 1,
                    content_hash: file.into(),
                    name: "Ananya Rao".into(),
                    email: email.into(),
                    phone: String::new(),
                    location: "Bengaluru".into(),
                    skills: "java".into(),
                    company: "Infosys".into(),
                    years: Some(5),
                    notice_raw: "0 days".into(),
                    notice_days: Some(0),
                    error: String::new(),
                    note: String::new(),
                    score: None,
                    matching: String::new(),
                    missing: String::new(),
                    rationale: String::new(),
                },
            )
            .unwrap();
        }
        mark_duplicate_emails(&conn, "/tmp/resumes").unwrap();
        let rows = profiles_for_root(&conn, "/tmp/resumes").unwrap();
        assert!(rows.iter().all(|row| row.note.contains("Same email")));
        set_setting(&conn, "saved_at", "1").unwrap();
        assert!(expire_if_old(&conn).unwrap());
        assert_eq!(profile_count(&conn), 0);
        let bad = std::env::temp_dir().join(format!("scout-bad-{}", std::process::id()));
        std::fs::write(&bad, b"this is not a database").unwrap();
        let (_recovered, note) = open_or_recover(&bad).unwrap();
        assert!(note.contains("could not be opened"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&bad);
    }
}
