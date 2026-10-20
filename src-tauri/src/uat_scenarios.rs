use std::fs;
use std::sync::{Arc, Mutex};

use crate::db;
use crate::excel;
use crate::filter::{self, Filters};
use crate::ingest::{self, Control, Progress};
use crate::mail;
use crate::portal;
use crate::rank::{self, Weights};

fn resume(index: usize) -> String {
    let city = if index % 2 == 0 { "Bengaluru" } else { "Pune" };
    let notice = if index % 3 == 0 { "30 days" } else if index % 3 == 1 { "90 days" } else { "immediate" };
    let skills = if index % 4 == 0 { "Python, Django" } else { "Java, Spring Boot, Kafka, JavaScript" };
    format!(
        "Candidate {index} Rao\ncandidate{index}@example.com\n98765{index:05}\nLocation: {city}\nCompany: Infosys\n8 years experience\nNotice period: {notice}\nSkills: {skills}\n"
    )
}

#[test]
fn recruiter_scenarios_s1_to_s6() {
    let root = std::env::temp_dir().join(format!("scout-uat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let folder = root.join("batch");
    fs::create_dir_all(&folder).unwrap();
    for index in 0..150 {
        fs::write(folder.join(format!("resume-{index}.txt")), resume(index)).unwrap();
    }
    let db_path = root.join("scout.db");
    let conn = Mutex::new(db::open_db(&db_path).unwrap());
    let control = Arc::new(Control::new());
    let progress = Mutex::new(Progress::default());
    ingest::ingest_path(&conn, &folder, &control, &progress, |_| {}).unwrap();
    assert_eq!(progress.lock().unwrap().parsed, 150);

    let connection = conn.lock().unwrap();
    let job = rank::parse_job("Java developer, Bengaluru\nMust-have: Java, Spring Boot, Kafka\nNice-to-have: JavaScript\nExperience: 5-9 years\nLocation: Bengaluru\n");
    let mut rows = db::profiles_for_root(&connection, &folder.to_string_lossy()).unwrap();
    for row in &mut rows {
        let scored = rank::score_profile(&row.skills, row.years, &row.location, &job, &Weights::default());
        row.score = Some(scored.score);
        row.rationale = scored.rationale;
    }
    let shortlist = filter::visible_rows(
        rows.clone(),
        &Filters { notice: "30".into(), locations: vec!["Bengaluru".into()], ..Filters::default() },
    );
    assert!(!shortlist.is_empty());
    assert!(shortlist.iter().all(|row| row.location == "Bengaluru" && row.notice_days.unwrap_or(99) <= 30));
    let top = shortlist.iter().take(10).collect::<Vec<_>>();
    assert_eq!(top.len(), 10);
    let excel_path = root.join("shortlist.xlsx");
    excel::write_xlsx(&excel_path, &shortlist).unwrap();
    assert!(excel_path.metadata().unwrap().len() > 100);

    let other = rank::parse_job("Python developer, Pune\nMust-have: Python, Django\nExperience: 5-9 years\nLocation: Pune\n");
    let java_row = rows.iter().find(|row| row.skills.contains("java")).unwrap();
    let python_row = rows.iter().find(|row| row.skills.contains("python")).unwrap();
    let java_on_python_job = rank::score_profile(&java_row.skills, java_row.years, &java_row.location, &other, &Weights::default());
    let python_on_python_job = rank::score_profile(&python_row.skills, python_row.years, &python_row.location, &other, &Weights::default());
    assert!(python_on_python_job.score > java_on_python_job.score);

    let other_folder = root.join("other");
    fs::create_dir_all(&other_folder).unwrap();
    fs::write(other_folder.join("one.txt"), resume(1)).unwrap();
    let needs_confirm = db::get_setting(&connection, "root") != other_folder.to_string_lossy() && db::profile_count(&connection) > 0;
    assert!(needs_confirm);
    db::clear_profiles(&connection).unwrap();
    drop(connection);
    ingest::ingest_path(&conn, &other_folder, &control, &progress, |_| {}).unwrap();
    let connection = conn.lock().unwrap();
    assert_eq!(db::profile_count(&connection), 1);
    db::clear_search(&connection).unwrap();
    assert_eq!(db::profile_count(&connection), 0);

    mail::connect(&connection, "recruiter@example.com").unwrap();
    let drafts = root.join("drafts");
    let jd = "Java developer\nMust-have: Java\n";
    let people = [
        mail::DraftPerson { name: "Ananya Rao".into(), email: "a@example.com".into(), skills: "java".into(), matching: "java".into(), years: Some(6), location: "Bengaluru".into(), company: String::new() },
        mail::DraftPerson { name: "Meera Iyer".into(), email: "b@example.com".into(), skills: "java".into(), matching: "java".into(), years: Some(4), location: "Pune".into(), company: String::new() },
        mail::DraftPerson { name: "Rahul Menon".into(), email: "c@example.com".into(), skills: "java".into(), matching: "java".into(), years: Some(9), location: "Chennai".into(), company: String::new() },
    ];
    let saved = mail::save_drafts(&connection, &drafts, "Hello", "First wording", jd, &people).unwrap();
    assert_eq!(saved, 3);
    let edited = mail::save_drafts(
        &connection,
        &drafts,
        "Hello",
        "Edited wording",
        jd,
        &[mail::DraftPerson { name: "Ananya Rao".into(), email: "a@example.com".into(), skills: "java".into(), matching: "java".into(), years: Some(6), location: "Bengaluru".into(), company: String::new() }],
    )
    .unwrap();
    assert_eq!(edited, 1);
    let text = fs::read_to_string(drafts.join("a@example.com.eml")).unwrap();
    assert!(text.contains("Edited wording"));
    assert!(text.contains("X-Unsent: 1"));
    assert!(!text.to_lowercase().contains("status: sent"));

    let plan = portal::search(&connection, "Java developer in Bengaluru").unwrap();
    assert!(plan.summary.contains("practice data"));
    let picked: Vec<String> = plan.people.iter().take(2).map(|person| person.id.clone()).collect();
    let saved_portal = portal::download(&connection, &root.join("portal"), &picked).unwrap();
    assert_eq!(saved_portal.saved, 2);
    drop(connection);
    let portal_dir = root.join("portal");
    ingest::ingest_path(&conn, &portal_dir, &control, &progress, |_| {}).unwrap();
    let connection = conn.lock().unwrap();
    let downloaded = db::profiles_for_root(&connection, &portal_dir.to_string_lossy()).unwrap();
    assert!(downloaded.iter().any(|row| !row.name.is_empty()));
    drop(connection);

    let missing = ingest::require_folder(&root.join("no-such-folder"));
    assert!(missing.unwrap_err().contains("could not be opened"));
    let empty = root.join("empty");
    fs::create_dir_all(&empty).unwrap();
    ingest::ingest_path(&conn, &empty, &control, &progress, |_| {}).unwrap();
    assert_eq!(progress.lock().unwrap().parsed, 0);
    fs::write(folder.join("notes.csv"), "not a resume").unwrap();
    fs::write(folder.join("scan.png"), b"\x89PNG").unwrap();
    ingest::ingest_path(&conn, &folder, &control, &progress, |_| {}).unwrap();
    let connection = conn.lock().unwrap();
    let png = db::profiles_for_root(&connection, &folder.to_string_lossy()).unwrap();
    assert!(png.iter().any(|row| row.filename == "scan.png" && row.error.contains("photo")));
    assert!(rank::parse_job("").must_have.is_empty());
    let empty_job = rank::score_profile("Java", Some(8), "Bengaluru", &rank::parse_job(""), &Weights::default());
    assert_eq!(empty_job.score, 0);
    mail::connect(&connection, "recruiter@example.com").unwrap();
    crate::privacy::store_mail_token("recruiter@example.com", 1).unwrap();
    let expired = mail::save_drafts(
        &connection,
        &drafts,
        "Hello",
        "Hi",
        jd,
        &[mail::DraftPerson { name: "Ananya Rao".into(), email: "a@example.com".into(), skills: "java".into(), matching: "java".into(), years: None, location: String::new(), company: String::new() }],
    );
    assert!(expired.unwrap_err().contains("expired"));
    let offline = portal::search(&connection, "Java developer").unwrap();
    assert!(!offline.people.is_empty());
    let _ = fs::remove_dir_all(&root);
}
