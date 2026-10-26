mod db;
mod expand;
mod dossier;
mod excel;
mod extract;
mod fields;
mod filter;
mod ingest;
mod letter;
mod mail;
mod portal;
mod privacy;
mod query;
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
    years: Option<i32>,
    company: String,
    notice: String,
    score: Option<i32>,
    rationale: String,
    note: String,
    path: String,
}

#[derive(Serialize)]
struct Grid {
    rows: Vec<GridRow>,
    locations: Vec<String>,
    errors: Vec<String>,
    root: String,
    stored: i32,
    jd: String,
    offline: bool,
    primary_search: Vec<String>,
    secondary_search: Vec<String>,
    added_primary: Vec<String>,
    added_secondary: Vec<String>,
    skill_note: String,
}

#[derive(Serialize)]
struct Prepare {
    status: String,
}

#[derive(Deserialize)]
struct DraftPerson {
    name: String,
    email: String,
    #[serde(default)]
    skills: String,
    #[serde(default)]
    matching: String,
    #[serde(default)]
    years: Option<i32>,
    #[serde(default)]
    location: String,
    #[serde(default)]
    company: String,
}

#[derive(Serialize)]
struct MailView {
    connected: bool,
    expired: bool,
    address: String,
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
fn prepare_files(state: State<Scout>, paths: Vec<String>) -> Result<Prepare, String> {
    let files = file_paths(&paths)?;
    let connection = state.db.lock().unwrap();
    if files_need_confirm(&connection, &files) {
        return Ok(Prepare { status: "confirm".to_string() });
    }
    Ok(Prepare { status: "ready".to_string() })
}

#[tauri::command]
fn ingest_files(app: AppHandle, state: State<Scout>, paths: Vec<String>, replace: bool) -> Result<(), String> {
    let files = file_paths(&paths)?;
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("A folder is already being read. Wait for it to finish, or press Cancel.".to_string());
    }
    {
        let connection = state.db.lock().unwrap();
        if files_need_confirm(&connection, &files) && !replace {
            state.running.store(false, Ordering::SeqCst);
            return Err("Start a new search? People already listed will be cleared from this computer.".to_string());
        }
        if files_need_confirm(&connection, &files) {
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
        if let Err(_err) = ingest::ingest_files(&db, &files, &control, &progress, |update| {
            let _ = app.emit("ingest-progress", update);
        }) {
            if let Ok(mut current) = progress.lock() {
                current.state = "done".to_string();
                current.messages.push("The selected files could not be finished. Choose them again. Files already read are kept.".to_string());
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

fn file_paths(paths: &[String]) -> Result<Vec<PathBuf>, String> {
    if paths.is_empty() {
        return Err("Choose at least one resume file.".to_string());
    }
    let mut files = Vec::new();
    for path in paths {
        let file = PathBuf::from(path);
        if path.trim().is_empty() || !file.is_file() {
            return Err("That file could not be opened. Choose the resume again.".to_string());
        }
        files.push(file);
    }
    Ok(files)
}

fn files_need_confirm(connection: &Connection, files: &[PathBuf]) -> bool {
    if db::profile_count(connection) == 0 {
        return false;
    }
    let root = db::get_setting(connection, "root");
    let saved = db::get_setting(connection, "file_list");
    root != ingest::SELECTED_FILES || saved != ingest::file_list_key(files)
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
fn connect_mail(state: State<Scout>, email: String) -> Result<(), String> {
    let connection = state.db.lock().unwrap();
    mail::connect(&connection, &email)
}

#[tauri::command]
fn disconnect_mail(state: State<Scout>) -> Result<(), String> {
    let connection = state.db.lock().unwrap();
    mail::disconnect(&connection)
}

#[tauri::command]
fn mail_status() -> Result<MailView, String> {
    let current = mail::status()?;
    Ok(MailView { connected: current.connected, expired: current.expired, address: current.address })
}

#[tauri::command]
fn save_drafts(app: AppHandle, state: State<Scout>, subject: String, instructions: String, jd_text: String, people: Vec<DraftPerson>) -> Result<u32, String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    let connection = state.db.lock().unwrap();
    let people: Vec<mail::DraftPerson> = people
        .into_iter()
        .map(|person| mail::DraftPerson {
            name: person.name,
            email: person.email,
            skills: person.skills,
            matching: person.matching,
            years: person.years,
            location: person.location,
            company: person.company,
        })
        .collect();
    mail::save_drafts(&connection, &mail::draft_folder(&dir), &subject, &instructions, &jd_text, &people)
}

#[tauri::command]
fn open_profile(state: State<Scout>, path: String) -> Result<dossier::Dossier, String> {
    let connection = state.db.lock().unwrap();
    known_resume(&connection, &path)?;
    let text = crate::extract::resume_text(std::path::Path::new(&path))?;
    Ok(dossier::from_text(&text))
}

#[tauri::command]
fn save_resume_copy(state: State<Scout>, source: String, dest: String) -> Result<(), String> {
    let connection = state.db.lock().unwrap();
    known_resume(&connection, &source)?;
    let from = std::path::Path::new(&source);
    let to = std::path::Path::new(&dest);
    if dest.trim().is_empty() || to.is_dir() {
        return Err("Choose a file name for the resume.".to_string());
    }
    if to.exists() {
        let same = std::fs::canonicalize(from).ok() == std::fs::canonicalize(to).ok();
        if same {
            return Err("Choose a different file name. The original resume stays in its folder.".to_string());
        }
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|_| "That folder could not be created.".to_string())?;
    }
    fs::copy(from, to).map_err(|_| "The resume could not be saved. Choose another folder and try again.".to_string())?;
    Ok(())
}

fn known_resume(connection: &Connection, path: &str) -> Result<(), String> {
    let found: i64 = connection
        .query_row("SELECT COUNT(*) FROM profiles WHERE path = ?1", [path], |row| row.get(0))
        .map_err(|err| err.to_string())?;
    if found == 0 {
        Err("That resume is not in the current search.".to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
fn gemini_status() -> bool {
    !expand::gemini_key().is_empty()
}

#[tauri::command]
fn save_gemini_key(key: String) -> Result<(), String> {
    expand::save_gemini_key(&key)
}

#[tauri::command]
fn portal_query(jd_text: String) -> Result<query::PortalQuery, String> {
    query::portal_query(&jd_text)
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
    let mut job = rank::parse_job(trimmed);
    let cached = {
        let connection = state.db.lock().unwrap();
        if db::get_setting(&connection, "root").is_empty() {
            return Err("Choose a resume folder first.".to_string());
        }
        (db::get_setting(&connection, "expand_jd"), db::get_setting(&connection, "expand_json"))
    };
    let fetched = if cached.0 == trimmed && !cached.1.is_empty() {
        Ok(cached.1)
    } else {
        expand::fetch_related(&job.must_have, &job.nice_to_have)
    };
    let connection = state.db.lock().unwrap();
    let plan = related_skills(&connection, trimmed, &mut job, fetched);
    let root = db::get_setting(&connection, "root");
    if root.is_empty() {
        return Err("Choose a resume folder first.".to_string());
    }
    db::set_setting(&connection, "jd", trimmed)?;
    for profile in db::profiles_for_root(&connection, &root)? {
        if !profile.error.is_empty() {
            continue;
        }
        let (skills, years, location, text) = skills_for_this_job(&profile);
        if skills != profile.skills {
            let _ = db::replace_skills(&connection, &profile.path, &skills);
        }
        let haystack = if text.is_empty() { skills.clone() } else { format!("{skills}\n{text}") };
        let scored = rank::score_profile(&haystack, years, &location, &job, &weights);
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
    let mut grid = grid_from(&connection, &filters)?;
    grid.primary_search = plan.primary;
    grid.secondary_search = plan.secondary;
    grid.added_primary = plan.added_primary;
    grid.added_secondary = plan.added_secondary;
    grid.skill_note = plan.note;
    Ok(grid)
}

fn related_skills(connection: &Connection, jd: &str, job: &mut rank::JobNeeds, fetched: Result<String, String>) -> expand::SkillPlan {
    let fresh = match fetched {
        Ok(body) => {
            let _ = db::set_setting(connection, "expand_jd", jd);
            let _ = db::set_setting(connection, "expand_json", &body);
            body
        }
        Err(note) => {
            let _ = db::set_setting(connection, "expand_jd", "");
            let _ = db::set_setting(connection, "expand_json", "");
            let _ = db::set_setting(connection, "skill_note", &note);
            return expand::SkillPlan {
                primary: job.must_have.clone(),
                secondary: job.nice_to_have.clone(),
                added_primary: Vec::new(),
                added_secondary: Vec::new(),
                note,
            };
        }
    };
    match expand::merge(job, &fresh) {
        Ok(plan) => {
            let _ = db::set_setting(connection, "skill_note", &plan.note);
            let _ = db::set_setting(connection, "primary_search", &plan.primary.join("\n"));
            let _ = db::set_setting(connection, "secondary_search", &plan.secondary.join("\n"));
            let _ = db::set_setting(connection, "added_primary", &plan.added_primary.join("\n"));
            let _ = db::set_setting(connection, "added_secondary", &plan.added_secondary.join("\n"));
            plan
        }
        Err(note) => {
            let _ = db::set_setting(connection, "skill_note", &note);
            expand::SkillPlan {
                primary: job.must_have.clone(),
                secondary: job.nice_to_have.clone(),
                added_primary: Vec::new(),
                added_secondary: Vec::new(),
                note,
            }
        }
    }
}

#[tauri::command]
fn skills_for_this_job(profile: &db::Stored) -> (String, Option<i32>, String, String) {
    let path = std::path::Path::new(&profile.path);
    if path.is_file() {
        if let Ok((fresh, text)) = crate::extract::profile_and_text(path) {
            let years = fresh.years.or(profile.years);
            let location = if fresh.location.is_empty() { profile.location.clone() } else { fresh.location };
            let skills = if fresh.skills.is_empty() { profile.skills.clone() } else { fresh.skills };
            return (skills, years, location, text);
        }
    }
    (profile.skills.clone(), profile.years, profile.location.clone(), String::new())
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
            years: row.years,
            company: row.company,
            notice: row.notice_raw,
            score: row.score,
            rationale: row.rationale,
            note: row.note,
            path: row.path,
        })
        .collect();
    Ok(Grid {
        rows,
        locations,
        errors,
        root,
        stored,
        jd: db::get_setting(conn, "jd"),
        offline: true,
        primary_search: stored_lines(conn, "primary_search"),
        secondary_search: stored_lines(conn, "secondary_search"),
        added_primary: stored_lines(conn, "added_primary"),
        added_secondary: stored_lines(conn, "added_secondary"),
        skill_note: db::get_setting(conn, "skill_note"),
    })
}

fn stored_lines(conn: &Connection, key: &str) -> Vec<String> {
    db::get_setting(conn, key)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            privacy::set_key_dir(&dir);
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
            prepare_files,
            ingest_files,
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
            portal_download,
            portal_query,
            gemini_status,
            save_gemini_key,
            open_profile,
            save_resume_copy
        ])
        .run(tauri::generate_context!())
        .expect("error while running Talent Scout Desktop");
}
