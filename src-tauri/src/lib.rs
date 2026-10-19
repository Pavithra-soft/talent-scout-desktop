mod db;
mod excel;
mod extract;
mod fields;
mod filter;
mod ingest;
mod rank;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::Serialize;
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
}

#[derive(Serialize)]
struct Grid {
    rows: Vec<GridRow>,
    locations: Vec<String>,
    errors: Vec<String>,
    root: String,
    offline: bool,
}

#[tauri::command]
fn ingest_start(app: AppHandle, state: State<Scout>, path: String) -> Result<(), String> {
    start_ingest(&app, &state, PathBuf::from(path))
}

#[tauri::command]
fn load_samples(app: AppHandle, state: State<Scout>) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?.join("samples");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    fs::write(dir.join("ananya-rao.txt"), include_str!("../samples/ananya-rao.txt")).map_err(|err| err.to_string())?;
    fs::write(dir.join("meera-iyer.txt"), include_str!("../samples/meera-iyer.txt")).map_err(|err| err.to_string())?;
    fs::write(dir.join("rahul-menon.txt"), include_str!("../samples/rahul-menon.txt")).map_err(|err| err.to_string())?;
    fs::write(dir.join("jd.txt"), include_str!("../samples/jd.txt")).map_err(|err| err.to_string())?;
    start_ingest(&app, &state, dir)
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
fn rank_profiles(state: State<Scout>, jd_text: String, weights: Weights) -> Result<Grid, String> {
    let trimmed = jd_text.trim();
    if trimmed.is_empty() {
        return Err("Paste a job description first.".to_string());
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
    grid_from(&connection, &Filters::default())
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
    Ok(rows.len() as u32)
}

fn start_ingest(app: &AppHandle, state: &Scout, path: PathBuf) -> Result<(), String> {
    if path.as_os_str().is_empty() || !path.exists() {
        return Err("Choose a folder that exists.".to_string());
    }
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("A folder is already being read.".to_string());
    }
    let db = state.db.clone();
    let control = state.control.clone();
    let progress = state.progress.clone();
    let running = state.running.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = ingest::ingest_path(&db, &path, &control, &progress, |update| {
            let _ = app.emit("ingest-progress", update);
        });
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
        })
        .collect();
    Ok(Grid {
        rows,
        locations,
        errors,
        root,
        offline: true,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let database = db::open_db(&dir.join("scout.db")).expect("database");
            app.manage(Scout {
                db: Arc::new(Mutex::new(database)),
                control: Arc::new(Control::new()),
                progress: Arc::new(Mutex::new(Progress::default())),
                running: Arc::new(AtomicBool::new(false)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ingest_start,
            load_samples,
            ingest_pause,
            ingest_resume,
            ingest_cancel,
            ingest_status,
            sample_jd,
            rank_profiles,
            list_results,
            export_excel
        ])
        .run(tauri::generate_context!())
        .expect("error while running Talent Scout Desktop");
}
