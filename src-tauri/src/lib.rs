mod db;
mod excel;
mod extract;
mod fields;
mod filter;
mod ingest;
mod mail;
mod portal;
mod privacy;
mod rank;

#[cfg(test)]
mod round1_probe;
#[cfg(test)]
mod uat_scenarios;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db::Stored;
use crate::filter::Filters;
use crate::ingest::{Control, Progress};
use crate::rank::Weights;

struct Scout {
    db: Arc<Mutex<Connection>>,
    control: Arc<Control>,
    progress: Arc<Mutex<Progress>>,
    running: Arc<AtomicBool>,
}

#[derive(Serialize)]
struct GridRow {
    rank: i32,
    name: String,
    email: String,
    phone: String,
    location: String,
    skills: String,
    matching: String,
    missing: String,
    company: String,
    notice: String,
    score: Option<i32>,
    rationale: String,
    note: String,
}

#[derive(Serialize)]
struct Grid {
    rows: Vec<GridRow>,
    locations: Vec<String>,
    errors: Vec<String>,
    root: String,
    stored: i32,
    offline: bool,
}

#[derive(Serialize)]
struct Prepare {
    status: String,
}

#[derive(Deserialize)]
struct DraftPerson {
    name: String,
    email: String,
}

#[derive(Serialize)]
struct MailView {
    connected: bool,
    expired: bool,
}

#[tauri::command]
fn prepare_folder(state: State<Scout>, path: String) -> Result<Prepare, String> {
    let folder = PathBuf::from(&path);
    if path.trim().is_empty() || !folder.exists() {
        return Err("That folder could not be opened. Choose the folder that holds the resumes.".to_string());
    }
    let connection = state.db.lock().unwrap();
    let root = db::get_setting(&connection, "root");
    if !root.is_empty() && root != path && db::profile_count(&connection) > 0 {
        return Ok(Prepare { status: "confirm".to_string() });
    }
    Ok(Prepare { status: "ready".to_string() })
}

#[tauri::command]
fn ingest_start(app: AppHandle, state: State<Scout>, path: String, replace: bool) -> Result<(), String> {
    start_ingest(&app, &state, PathBuf::from(path), replace)
}

#[tauri::command]
fn load_samples(app: AppHandle) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?.join("samples");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    fs::write(dir.join("ananya-rao.txt"), include_str!("../samples/ananya-rao.txt")).map_err(|err| err.to_string())?;
    fs::write(dir.join("meera-iyer.txt"), include_str!("../samples/meera-iyer.txt")).map_err(|err| err.to_string())?;
    fs::write(dir.join("rahul-menon.txt"), include_str!("../samples/rahul-menon.txt")).map_err(|err| err.to_string())?;
    fs::write(dir.join("jd.txt"), include_str!("../samples/jd.txt")).map_err(|err| err.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
fn start_fresh(app: AppHandle, state: State<Scout>) -> Result<(), String> {
    let connection = state.db.lock().unwrap();
    db::clear_search(&connection)?;
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = fs::remove_dir_all(dir.join("drafts"));
        let _ = fs::remove_dir_all(dir.join("samples"));
    }
    Ok(())
}

#[tauri::command]
fn startup_note(state: State<Scout>) -> String {
    let connection = state.db.lock().unwrap();
    let note = db::get_setting(&connection, "startup_note");
    if !note.is_empty() {
        let _ = db::set_setting(&connection, "startup_note", "");
    }
    note
}

#[tauri::command]
fn connect_mail(state: State<Scout>) -> Result<(), String> {
    let connection = state.db.lock().unwrap();
    mail::connect(&connection)
}

#[tauri::command]
fn disconnect_mail(state: State<Scout>) -> Result<(), String> {
    let connection = state.db.lock().unwrap();
    mail::disconnect(&connection)
}

#[tauri::command]
fn mail_status() -> Result<MailView, String> {
    let current = mail::status()?;
    Ok(MailView { connected: current.connected, expired: current.expired })
}

#[tauri::command]
fn save_drafts(app: AppHandle, state: State<Scout>, subject: String, body: String, people: Vec<DraftPerson>) -> Result<u32, String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    let connection = state.db.lock().unwrap();
    let pairs: Vec<(String, String)> = people.into_iter().map(|person| (person.name, person.email)).collect();
    mail::save_drafts(&connection, &mail::draft_folder(&dir), &subject, &body, &pairs)
}

#[tauri::command]
fn portal_search(state: State<Scout>, jd_text: String) -> Result<portal::PortalPlan, String> {
    let connection = state.db.lock().unwrap();
    portal::search(&connection, &jd_text)
}

#[tauri::command]
fn portal_download(state: State<Scout>, path: String, ids: Vec<String>) -> Result<portal::DownloadResult, String> {
    let connection = state.db.lock().unwrap();
    portal::download(&connection, std::path::Path::new(&path), &ids)
}

#[tauri::command]
fn ingest_pause(state: State<Scout>) {
    state.control.pause();
    if let Ok(mut progress) = state.progress.lock() {
        if progress.state == "running" {
            progress.state = "paused".to_string();
        }
    }
}

#[tauri::command]
fn ingest_resume(state: State<Scout>) {
    state.control.resume();
}

#[tauri::command]
fn ingest_cancel(state: State<Scout>) {
    state.control.cancel();
}

#[tauri::command]
fn ingest_status(state: State<Scout>) -> Progress {
    state.progress.lock().map(|progress| progress.clone()).unwrap_or_default()
}

#[tauri::command]
fn sample_jd() -> String {
    include_str!("../samples/jd.txt").to_string()
}

#[tauri::command]
fn rank_profiles(state: State<Scout>, jd_text: String, weights: Weights, filters: Filters) -> Result<Grid, String> {
    let trimmed = jd_text.trim();
    if trimmed.is_empty() {
        return Err("Paste the job first, then rank the candidates.".to_string());
    }
    let job = rank::parse_job(trimmed);
    let connection = state.db.lock().unwrap();
    let root = db::get_setting(&connection, "root");
    if root.is_empty() {
        return Err("Choose a resume folder first.".to_string());
    }
    db::set_setting(&connection, "jd", trimmed)?;
    for profile in db::profiles_for_root(&connection, &root)? {
        if !profile.error.is_empty() {
            continue;
        }
        let scored = rank::score_profile(&profile.skills, profile.years, &profile.location, &job, &weights);
        db::save_score(
            &connection,
            &profile.path,
            scored.score,
            &scored.matching.join(", "),
            &scored.missing.join(", "),
            &scored.rationale,
        )?;
    }
    let _ = db::audit(&connection, "ranked", "current search");
    grid_from(&connection, &filters)
}

#[tauri::command]
fn list_results(state: State<Scout>, filters: Filters) -> Result<Grid, String> {
    let connection = state.db.lock().unwrap();
    grid_from(&connection, &filters)
}

#[tauri::command]
fn export_excel(state: State<Scout>, path: String, filters: Filters) -> Result<u32, String> {
    let connection = state.db.lock().unwrap();
    let rows = filtered(&connection, &filters)?;
    excel::write_xlsx(std::path::Path::new(&path), &rows)?;
    let _ = db::audit(&connection, "excel saved", &format!("rows={}", rows.len()));
    Ok(rows.len() as u32)
}

fn start_ingest(app: &AppHandle, state: &Scout, path: PathBuf, replace: bool) -> Result<(), String> {
    ingest::require_folder(&path)?;
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("A folder is already being read. Wait for it to finish, or press Cancel.".to_string());
    }
    let path_text = path.to_string_lossy().to_string();
    {
        let connection = state.db.lock().unwrap();
        let root = db::get_setting(&connection, "root");
        if !root.is_empty() && root != path_text && db::profile_count(&connection) > 0 && !replace {
            state.running.store(false, Ordering::SeqCst);
            return Err("Start a new search? People from the previous folder will be cleared from this computer.".to_string());
        }
        if root != path_text {
            db::clear_profiles(&connection).map_err(|err| {
                state.running.store(false, Ordering::SeqCst);
                err
            })?;
        }
    }
    let db = state.db.clone();
    let control = state.control.clone();
    let progress = state.progress.clone();
    let running = state.running.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(_err) = ingest::ingest_path(&db, &path, &control, &progress, |update| {
            let _ = app.emit("ingest-progress", update);
        }) {
            if let Ok(mut current) = progress.lock() {
                current.state = "done".to_string();
                current.messages.push("The folder could not be finished. Start the folder again. Files already read are kept.".to_string());
                let _ = app.emit("ingest-progress", &*current);
            }
        }
        running.store(false, Ordering::SeqCst);
        if let Ok(current) = progress.lock() {
            let _ = app.emit("ingest-progress", &*current);
        }
    });
    Ok(())
}

fn filtered(conn: &Connection, filters: &Filters) -> Result<Vec<Stored>, String> {
    let root = db::get_setting(conn, "root");
    let rows = db::profiles_for_root(conn, &root)?;
    Ok(filter::visible_rows(rows, filters))
}

fn grid_from(conn: &Connection, filters: &Filters) -> Result<Grid, String> {
    let root = db::get_setting(conn, "root");
    let all = db::profiles_for_root(conn, &root)?;
    let mut locations: Vec<String> = all
        .iter()
        .filter(|row| !row.location.is_empty())
        .map(|row| row.location.clone())
        .collect();
    locations.sort();
    locations.dedup();
    let errors: Vec<String> = all
        .iter()
        .filter(|row| !row.error.is_empty())
        .map(|row| format!("{}: {}", row.filename, row.error))
        .take(40)
        .collect();
    let stored = all.iter().filter(|row| row.error.is_empty()).count() as i32;
    let rows = filter::visible_rows(all, filters)
        .into_iter()
        .enumerate()
        .map(|(index, row)| GridRow {
            rank: (index + 1) as i32,
            name: row.name,
            email: row.email,
            phone: row.phone,
            location: row.location,
            skills: row.skills,
            matching: row.matching,
            missing: row.missing,
            company: row.company,
            notice: row.notice_raw,
            score: row.score,
            rationale: row.rationale,
            note: row.note,
        })
        .collect();
    Ok(Grid {
        rows,
        locations,
        errors,
        root,
        stored,
        offline: true,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let db_path = dir.join("scout.db");
            let (database, note) = db::open_or_recover(&db_path).map_err(|err| std::io::Error::other(err))?;
            if !note.is_empty() {
                let _ = db::set_setting(&database, "startup_note", &note);
            }
            let _ = db::expire_if_old(&database);
            app.manage(Scout {
                db: Arc::new(Mutex::new(database)),
                control: Arc::new(Control::new()),
                progress: Arc::new(Mutex::new(Progress::default())),
                running: Arc::new(AtomicBool::new(false)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            prepare_folder,
            ingest_start,
            load_samples,
            start_fresh,
            startup_note,
            ingest_pause,
            ingest_resume,
            ingest_cancel,
            ingest_status,
            sample_jd,
            rank_profiles,
            list_results,
            export_excel,
            connect_mail,
            disconnect_mail,
            mail_status,
            save_drafts,
            portal_search,
            portal_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running Talent Scout Desktop");
}
