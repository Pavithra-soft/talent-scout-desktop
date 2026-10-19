use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::db;
use crate::rank::{self, Weights};
use rusqlite::Connection;

#[derive(Clone, serde::Deserialize, Serialize)]
pub struct PortalPerson {
    pub id: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub location: String,
    pub skills: String,
    pub company: String,
    pub years: i32,
    pub notice: String,
}

#[derive(Serialize)]
pub struct PortalPlan {
    pub summary: String,
    pub people: Vec<PortalPerson>,
}

#[derive(Debug, Serialize)]
pub struct DownloadResult {
    pub saved: u32,
    pub skipped: u32,
    pub failed: u32,
}

const QUOTA: usize = 20;

fn catalog() -> Vec<PortalPerson> {
    serde_json::from_str(include_str!("../samples/mock-portal.json")).unwrap_or_default()
}

pub fn search(conn: &Connection, jd_text: &str) -> Result<PortalPlan, String> {
    let trimmed = jd_text.trim();
    if trimmed.is_empty() {
        return Err("Paste the job first. The sample portal has nothing to match yet.".to_string());
    }
    let job = rank::parse_job(trimmed);
    let mut people = catalog();
    people.sort_by_key(|person| {
        let scored = rank::score_profile(&person.skills, Some(person.years), &person.location, &job, &Weights::default());
        std::cmp::Reverse(scored.score)
    });
    let summary = format!(
        "Sample portal only. This is practice data on this computer, not a live job site. {} profiles match this job.",
        people.len()
    );
    db::audit(conn, "sample portal", &format!("profiles={}", people.len()))?;
    Ok(PortalPlan { summary, people })
}

pub fn download(conn: &Connection, folder: &Path, ids: &[String]) -> Result<DownloadResult, String> {
    if ids.is_empty() {
        return Err("Choose at least one sample profile, then save the resumes.".to_string());
    }
    if ids.len() > QUOTA {
        return Err("The sample portal limit for this search is 20 profiles. Choose fewer and try again.".to_string());
    }
    fs::create_dir_all(folder).map_err(|_| "That folder could not be created. Choose another folder and try again.".to_string())?;
    let catalog = catalog();
    let mut saved = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;
    let mut seen = std::collections::HashSet::new();
    for id in ids {
        let Some(person) = catalog.iter().find(|item| &item.id == id) else {
            failed += 1;
            continue;
        };
        let email_key = person.email.to_lowercase();
        if !email_key.is_empty() && !seen.insert(email_key.clone()) {
            skipped += 1;
            continue;
        }
        let file_name = format!("{}-{}.txt", slug(&person.name), slug(&person.email));
        let path = folder.join(file_name);
        if path.exists() {
            skipped += 1;
            continue;
        }
        let body = format!(
            "{}\nEmail: {}\nPhone: {}\nLocation: {}\nCompany: {}\n{} years experience\nNotice: {}\nSkills: {}\n",
            person.name, person.email, person.phone, person.location, person.company, person.years, person.notice, person.skills
        );
        if fs::write(&path, &body).is_err() {
            if fs::write(&path, &body).is_err() {
                failed += 1;
                continue;
            }
        }
        saved += 1;
    }
    let log = format!("Saved {saved}. Skipped {skipped} already present. Could not save {failed}.");
    let _ = fs::write(folder.join("talent-scout-download-log.txt"), &log);
    db::audit(conn, "sample download", &log)?;
    Ok(DownloadResult { saved, skipped, failed })
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '@' || ch == '.' {
            out.push('-');
        }
    }
    if out.is_empty() { "candidate".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_portal_skips_duplicates_and_logs_counts_only() {
        let dir = std::env::temp_dir().join(format!("scout-portal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let conn = db::open_db(&dir.join("scout.db")).unwrap();
        let plan = search(&conn, "Java developer in Bengaluru, 5-9 years").unwrap();
        assert!(plan.summary.contains("practice data"));
        assert!(plan.people.len() >= 5);
        let first = download(&conn, &dir, &["p1".into()]).unwrap();
        assert_eq!(first.saved, 1);
        let second = download(&conn, &dir, &["p1".into(), "p1".into()]).unwrap();
        assert_eq!(second.saved, 0);
        assert!(second.skipped >= 1);
        let log = fs::read_to_string(dir.join("talent-scout-download-log.txt")).unwrap();
        assert!(!log.contains("@"));
        let too_many: Vec<String> = (0..21).map(|index| format!("p{index}")).collect();
        assert!(download(&conn, &dir, &too_many).unwrap_err().contains("20"));
        let _ = fs::remove_dir_all(&dir);
    }
}
