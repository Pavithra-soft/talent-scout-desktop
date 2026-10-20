use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;
use walkdir::WalkDir;

use crate::db::{self, Stored};
use crate::extract::{allowed_name, profile_from_bytes, profile_from_path};
use rusqlite::Connection;

#[derive(Clone, serde::Serialize)]
pub struct Progress {
    pub done: u32,
    pub total: u32,
    pub parsed: u32,
    pub skipped: u32,
    pub errors: u32,
    pub files_per_min: f64,
    pub current: String,
    pub state: String,
    pub messages: Vec<String>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            done: 0,
            total: 0,
            parsed: 0,
            skipped: 0,
            errors: 0,
            files_per_min: 0.0,
            current: String::new(),
            state: "idle".to_string(),
            messages: Vec::new(),
        }
    }
}

pub struct Control {
    pause: AtomicBool,
    cancel: AtomicBool,
    pair: Mutex<()>,
    cv: Condvar,
}

impl Control {
    pub fn new() -> Self {
        Self {
            pause: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            pair: Mutex::new(()),
            cv: Condvar::new(),
        }
    }

    pub fn pause(&self) {
        self.pause.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.pause.store(false, Ordering::SeqCst);
        self.cv.notify_all();
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        self.resume();
    }

    pub fn reset(&self) {
        self.cancel.store(false, Ordering::SeqCst);
        self.pause.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.pause.load(Ordering::SeqCst)
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    fn wait_if_paused(&self) -> bool {
        let mut guard = self.pair.lock().unwrap();
        while self.pause.load(Ordering::SeqCst) && !self.cancelled() {
            guard = self.cv.wait(guard).unwrap();
        }
        !self.cancelled()
    }
}

enum Item {
    File(PathBuf),
    Memory { path: String, filename: String, bytes: Vec<u8> },
    Failed { path: String, filename: String, message: String },
}

struct Ready {
    path: String,
    filename: String,
    size: i64,
    mtime: i64,
    hash: String,
    name: String,
    email: String,
    phone: String,
    location: String,
    skills: String,
    company: String,
    years: Option<i32>,
    notice_raw: String,
    notice_days: Option<i32>,
    error: String,
}

pub fn require_folder(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() || !path.exists() {
        return Err("That folder could not be opened. Choose the folder that holds the resumes.".to_string());
    }
    Ok(())
}

pub fn ingest_path(
    conn: &Mutex<Connection>,
    root: &Path,
    control: &Arc<Control>,
    progress: &Mutex<Progress>,
    on_update: impl Fn(&Progress),
) -> Result<(), String> {
    control.reset();
    let started = Instant::now();
    let root_text = root.to_string_lossy().to_string();
    {
        let connection = conn.lock().unwrap();
        db::set_setting(&connection, "root", &root_text)?;
    }
    let mut todo = Vec::new();
    let mut skipped = 0u32;
    for item in collect(root)? {
        if !control.wait_if_paused() {
            break;
        }
        if should_skip(conn, &item)? {
            skipped += 1;
        } else {
            todo.push(item);
        }
    }
    let total = skipped + todo.len() as u32;
    publish(
        progress,
        &on_update,
        Progress {
            done: skipped,
            total,
            parsed: 0,
            skipped,
            errors: 0,
            files_per_min: 0.0,
            current: String::new(),
            state: if control.cancelled() { "cancelled".into() } else { "running".into() },
            messages: Vec::new(),
        },
    );
    if todo.is_empty() || control.cancelled() {
        let mut current = progress.lock().unwrap().clone();
        current.state = if control.cancelled() { "cancelled".into() } else { "done".into() };
        publish(progress, &on_update, current);
        return Ok(());
    }

    let (tx, rx) = mpsc::channel::<Ready>();
    let job_count = todo.len();
    let worker_control = Arc::clone(control);
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
            let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
            pool.install(|| {
                todo.par_iter().for_each(|item| {
                    let ready = if worker_control.wait_if_paused() {
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_item(item))) {
                            Ok(ready) => ready,
                            Err(_) => failed_item(item, "This file could not be read. The other resumes will still be read."),
                        }
                    } else {
                        cancelled_item(item)
                    };
                    let _ = tx.send(ready);
                });
            });
        });
        let mut parsed = 0u32;
        let mut errors = 0u32;
        let mut messages = Vec::new();
        for index in 0..job_count {
            let ready = match rx.recv() {
                Ok(item) => item,
                Err(_) => break,
            };
            if ready.error == "cancelled" {
                continue;
            }
            let duplicate = ready.error.is_empty()
                && !ready.hash.is_empty()
                && db::hash_saved(&conn.lock().unwrap(), &root_text, &ready.hash);
            if duplicate {
                if messages.len() < 40 {
                    messages.push(format!(
                        "{}: this is a copy of a resume already read, so it was listed once.",
                        ready.filename
                    ));
                }
            } else if ready.error.is_empty() {
                parsed += 1;
            } else {
                errors += 1;
                if messages.len() < 40 {
                    messages.push(format!("{}: {}", ready.filename, ready.error));
                }
            }
            if duplicate {
                let done = skipped + index as u32 + 1;
                let minutes = (started.elapsed().as_secs_f64() / 60.0).max(0.001);
                publish(
                    progress,
                    &on_update,
                    Progress {
                        done,
                        total,
                        parsed,
                        skipped,
                        errors,
                        files_per_min: (done as f64) / minutes,
                        current: ready.filename,
                        state: if control.is_paused() { "paused".into() } else { "running".into() },
                        messages: messages.clone(),
                    },
                );
                continue;
            }
            {
                let connection = conn.lock().unwrap();
                let _ = db::save_profile(
                    &connection,
                    &root_text,
                    &Stored {
                        path: ready.path.clone(),
                        filename: ready.filename.clone(),
                        size: ready.size,
                        mtime: ready.mtime,
                        content_hash: ready.hash.clone(),
                        name: ready.name,
                        email: ready.email,
                        phone: ready.phone,
                        location: ready.location,
                        skills: ready.skills,
                        company: ready.company,
                        years: ready.years,
                        notice_raw: ready.notice_raw,
                        notice_days: ready.notice_days,
                        error: ready.error,
                        note: String::new(),
                        score: None,
                        matching: String::new(),
                        missing: String::new(),
                        rationale: String::new(),
                    },
                );
            }
            let done = skipped + index as u32 + 1;
            let minutes = (started.elapsed().as_secs_f64() / 60.0).max(0.001);
            publish(
                progress,
                &on_update,
                Progress {
                    done,
                    total,
                    parsed,
                    skipped,
                    errors,
                    files_per_min: (done as f64) / minutes,
                    current: ready.filename,
                    state: if control.is_paused() { "paused".into() } else { "running".into() },
                    messages: messages.clone(),
                },
            );
        }
    });
    {
        let connection = conn.lock().unwrap();
        let _ = db::mark_duplicate_emails(&connection, &root_text);
        let _ = db::set_setting(&connection, "saved_at", &chrono::Utc::now().timestamp().to_string());
        let done = progress.lock().unwrap().clone();
        let _ = db::audit(
            &connection,
            "folder read",
            &format!("new={} unchanged={} could_not_read={}", done.parsed, done.skipped, done.errors),
        );
    }
    let mut current = progress.lock().unwrap().clone();
    current.state = if control.cancelled() { "cancelled".into() } else { "done".into() };
    current.done = current.total;
    publish(progress, &on_update, current);
    Ok(())
}

fn publish(progress: &Mutex<Progress>, on_update: &impl Fn(&Progress), next: Progress) {
    *progress.lock().unwrap() = next.clone();
    on_update(&next);
}

fn should_skip(conn: &Mutex<Connection>, item: &Item) -> Result<bool, String> {
    match item {
        Item::Failed { .. } => Ok(false),
        Item::File(path) => {
            let meta = match fs::metadata(path) {
                Ok(meta) => meta,
                Err(_) => return Ok(false),
            };
            if meta.len() > 15 * 1024 * 1024 {
                return Ok(false);
            }
            let size = meta.len() as i64;
            let mtime = mtime_of(&meta);
            let connection = conn.lock().unwrap();
            let Some((cached_size, cached_mtime, cached_hash, error)) = db::cached(&connection, &path.to_string_lossy()) else {
                return Ok(false);
            };
            if error.is_empty() && cached_size == size && cached_mtime == mtime {
                return Ok(true);
            }
            if error.is_empty() {
                let bytes = fs::read(path).map_err(|_| "This file could not be opened. Close it in another program and read the folder again.".to_string())?;
                if hash_bytes(&bytes) == cached_hash {
                    db::touch_cache(&connection, &path.to_string_lossy(), size, mtime)?;
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Item::Memory { path, bytes, .. } => {
            let connection = conn.lock().unwrap();
            let Some((_, _, cached_hash, error)) = db::cached(&connection, path) else {
                return Ok(false);
            };
            Ok(error.is_empty() && hash_bytes(bytes) == cached_hash)
        }
    }
}

fn collect(root: &Path) -> Result<Vec<Item>, String> {
    if root.is_file() {
        return Ok(file_item(root));
    }
    let mut items = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|item| item.ok()) {
        if entry.file_type().is_file() {
            items.extend(file_item(entry.path()));
        }
    }
    Ok(items)
}

fn file_item(path: &Path) -> Vec<Item> {
    let name = path.file_name().and_then(|item| item.to_str()).unwrap_or("");
    if !allowed_name(name) {
        return Vec::new();
    }
    if name.to_lowercase().ends_with(".zip") {
        return zip_items(path);
    }
    vec![Item::File(path.to_path_buf())]
}

fn zip_items(path: &Path) -> Vec<Item> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    let mut total = 0u64;
    for index in 0..archive.len().min(500) {
        let Ok(mut entry) = archive.by_index(index) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name.contains("..") || !allowed_name(&name) || name.to_lowercase().ends_with(".zip") {
            continue;
        }
        let filename = Path::new(&name)
            .file_name()
            .and_then(|item| item.to_str())
            .unwrap_or("resume")
            .to_string();
        if entry.size() > 8 * 1024 * 1024 {
            items.push(Item::Failed {
                path: format!("{}!/{}", path.display(), name),
                filename,
                message: "This file inside the zip is too large to read. Split it or send a smaller copy.".into(),
            });
            continue;
        }
        total += entry.size();
        if total > 80 * 1024 * 1024 {
            items.push(Item::Failed {
                path: format!("{}!/{}", path.display(), name),
                filename,
                message: "The zip is too large to finish. Split it into smaller zip files.".into(),
            });
            break;
        }
        let mut bytes = Vec::new();
        if entry.read_to_end(&mut bytes).is_err() {
            continue;
        }
        items.push(Item::Memory {
            path: format!("{}!/{}", path.display(), name),
            filename,
            bytes,
        });
    }
    items
}

fn parse_item(item: &Item) -> Ready {
    match item {
        Item::Failed { path, filename, message } => {
            ready(path.clone(), filename.clone(), 0, 0, String::new(), crate::fields::Profile::blank(), message)
        }
        Item::File(path) => {
            let filename = file_label(path);
            let meta = fs::metadata(path).ok();
            let size = meta.as_ref().map(|item| item.len() as i64).unwrap_or(0);
            let mtime = meta.as_ref().map(mtime_of).unwrap_or(0);
            if size > 15 * 1024 * 1024 {
                return ready(
                    path.to_string_lossy().to_string(),
                    filename,
                    size,
                    mtime,
                    String::new(),
                    crate::fields::Profile::blank(),
                    "This file is too large to read. Split it or send a smaller copy.",
                );
            }
            let bytes = fs::read(path).unwrap_or_default();
            let hash = hash_bytes(&bytes);
            match profile_from_path(path) {
                Ok(profile) => ready(path.to_string_lossy().to_string(), filename, size, mtime, hash, profile, ""),
                Err(error) => ready(path.to_string_lossy().to_string(), filename, size, mtime, hash, crate::fields::Profile::blank(), &error),
            }
        }
        Item::Memory { path, filename, bytes } => {
            let hash = hash_bytes(bytes);
            match profile_from_bytes(filename, bytes) {
                Ok(profile) => ready(path.clone(), filename.clone(), bytes.len() as i64, 0, hash, profile, ""),
                Err(error) => ready(path.clone(), filename.clone(), bytes.len() as i64, 0, hash, crate::fields::Profile::blank(), &error),
            }
        }
    }
}

fn failed_item(item: &Item, message: &str) -> Ready {
    let (path, filename) = match item {
        Item::File(path) => (path.to_string_lossy().to_string(), file_label(path)),
        Item::Memory { path, filename, .. } | Item::Failed { path, filename, .. } => (path.clone(), filename.clone()),
    };
    ready(path, filename, 0, 0, String::new(), crate::fields::Profile::blank(), message)
}

fn cancelled_item(item: &Item) -> Ready {
    let (path, filename) = match item {
        Item::File(path) => (path.to_string_lossy().to_string(), file_label(path)),
        Item::Memory { path, filename, .. } | Item::Failed { path, filename, .. } => (path.clone(), filename.clone()),
    };
    ready(path, filename, 0, 0, String::new(), crate::fields::Profile::blank(), "cancelled")
}

fn ready(
    path: String,
    filename: String,
    size: i64,
    mtime: i64,
    hash: String,
    mut profile: crate::fields::Profile,
    error: &str,
) -> Ready {
    if profile.name.trim().is_empty() {
        profile.name = crate::fields::name_from_filename(&filename);
    }
    Ready {
        path,
        filename,
        size,
        mtime,
        hash,
        name: profile.name,
        email: profile.email,
        phone: profile.phone,
        location: profile.location,
        skills: profile.skills,
        company: profile.company,
        years: profile.years,
        notice_raw: profile.notice_raw,
        notice_days: profile.notice_days,
        error: error.to_string(),
    }
}

fn file_label(path: &Path) -> String {
    path.file_name().and_then(|item| item.to_str()).unwrap_or("resume").to_string()
}

fn mtime_of(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|time| time.as_secs() as i64)
        .unwrap_or(0)
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn second_scan_skips_unchanged_files_and_a_thousand_finish_quickly() {
        let dir = std::env::temp_dir().join(format!("scout-bench-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for index in 0..1000 {
            let body = format!(
                "Candidate {index} Rao\ncandidate{index}@example.com\n98765{:05}\nLocation: Bengaluru\nCompany: Infosys\n8 years experience\nNotice period: 30 days\nSkills: Java, Spring Boot, Kafka\n",
                index % 100000
            );
            fs::write(dir.join(format!("resume-{index}.txt")), body).unwrap();
        }
        let db_path = dir.join("scout.db");
        let conn = Mutex::new(db::open_db(&db_path).unwrap());
        let control = Arc::new(Control::new());
        let progress = Mutex::new(Progress::default());
        let started = Instant::now();
        ingest_path(&conn, &dir, &control, &progress, |_| {}).unwrap();
        let elapsed = started.elapsed();
        assert!(elapsed.as_secs() < 60, "1000 resumes took {:?}", elapsed);
        assert_eq!(progress.lock().unwrap().parsed, 1000);
        ingest_path(&conn, &dir, &control, &progress, |_| {}).unwrap();
        assert_eq!(progress.lock().unwrap().skipped, 1000);
        assert_eq!(progress.lock().unwrap().parsed, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn identical_files_are_listed_once() {
        let dir = std::env::temp_dir().join(format!("scout-dup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let body = "Gowla Kiran\nkiran@example.com\n4.1 years experience\nSkills: Java, Spring Boot\n";
        fs::write(dir.join("kiran-a.txt"), body).unwrap();
        fs::write(dir.join("kiran-b.txt"), body).unwrap();
        let conn = Mutex::new(db::open_db(&dir.join("scout.db")).unwrap());
        let control = Arc::new(Control::new());
        let progress = Mutex::new(Progress::default());
        ingest_path(&conn, &dir, &control, &progress, |_| {}).unwrap();
        let connection = conn.lock().unwrap();
        let rows = db::profiles_for_root(&connection, &dir.to_string_lossy()).unwrap();
        assert_eq!(rows.len(), 1, "{:?}", rows.iter().map(|row| &row.filename).collect::<Vec<_>>());
        let _ = fs::remove_dir_all(&dir);
    }
}
