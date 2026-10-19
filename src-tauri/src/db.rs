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
    pub score: Option<i32>,
    pub matching: String,
    pub missing: String,
    pub rationale: String,
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
        ",
    )
    .map_err(|err| err.to_string())?;
    Ok(conn)
}

pub fn cached(conn: &Connection, path: &str) -> Option<(i64, i64, String, String)> {
    conn.query_row(
        "SELECT size, mtime, content_hash, error FROM profiles WHERE path = ?1",
        [path],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .ok()
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
            skills, company, years, notice_raw, notice_days, error
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
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
            error = excluded.error",
        params![
            item.path,
            root,
            item.size,
            item.mtime,
            item.content_hash,
            item.filename,
            item.name,
            item.email,
            item.phone,
            item.location,
            item.skills,
            item.company,
            item.years,
            item.notice_raw,
            item.notice_days,
            item.error
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
                    p.location, p.skills, p.company, p.years, p.notice_raw, p.notice_days, p.error,
                    s.score, s.matching, s.missing, s.rationale
             FROM profiles p
             LEFT JOIN scores s ON s.path = p.path
             WHERE p.root = ?1
             ORDER BY p.name",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([root], |row| {
            Ok(Stored {
                path: row.get(0)?,
                filename: row.get(1)?,
                size: row.get(2)?,
                mtime: row.get(3)?,
                content_hash: row.get(4)?,
                name: row.get(5)?,
                email: row.get(6)?,
                phone: row.get(7)?,
                location: row.get(8)?,
                skills: row.get(9)?,
                company: row.get(10)?,
                years: row.get(11)?,
                notice_raw: row.get(12)?,
                notice_days: row.get(13)?,
                error: row.get(14)?,
                score: row.get(15)?,
                matching: row.get(16).unwrap_or_default(),
                missing: row.get(17).unwrap_or_default(),
                rationale: row.get(18).unwrap_or_default(),
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
