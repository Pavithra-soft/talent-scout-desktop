use chrono::Datelike;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::db::{self, Stored};
use crate::excel;
use crate::extract::profile_from_bytes;
use crate::fields::profile_from_text;
use crate::filter::Filters;
use crate::ingest::{self, Control, Progress};
use crate::rank::{self, Weights};

struct Label {
    rel: String,
    name: String,
    email: String,
    phone: String,
    location: String,
    skills: Vec<String>,
    company: String,
    notice_days: Option<i32>,
    expect_error: bool,
}

struct Counts {
    tp: i32,
    fp: i32,
    fns: i32,
}

impl Counts {
    fn new() -> Self {
        Self { tp: 0, fp: 0, fns: 0 }
    }

    fn add_exact(&mut self, expected: &str, actual: &str) {
        let expected = expected.trim();
        let actual = actual.trim();
        if expected.is_empty() && actual.is_empty() {
            return;
        }
        if expected.eq_ignore_ascii_case(actual) {
            self.tp += 1;
        } else if actual.is_empty() {
            self.fns += 1;
        } else if expected.is_empty() {
            self.fp += 1;
        } else {
            self.fp += 1;
            self.fns += 1;
        }
    }

    fn add_option(&mut self, expected: Option<i32>, actual: Option<i32>) {
        match (expected, actual) {
            (None, None) => {}
            (Some(left), Some(right)) if left == right => self.tp += 1,
            (Some(_), None) => self.fns += 1,
            (None, Some(_)) => self.fp += 1,
            (Some(_), Some(_)) => {
                self.fp += 1;
                self.fns += 1;
            }
        }
    }

    fn line(&self) -> String {
        let precision = ratio(self.tp, self.tp + self.fp);
        let recall = ratio(self.tp, self.tp + self.fns);
        format!("tp={} fp={} fn={} precision={precision:.3} recall={recall:.3}", self.tp, self.fp, self.fns)
    }
}

fn ratio(part: i32, total: i32) -> f64 {
    if total == 0 {
        1.0
    } else {
        f64::from(part) / f64::from(total)
    }
}

fn lwd_days() -> i32 {
    let today = chrono::Local::now().date_naive();
    let mut lwd = chrono::NaiveDate::from_ymd_opt(today.year(), 11, 15).unwrap();
    if lwd < today {
        lwd = chrono::NaiveDate::from_ymd_opt(today.year() + 1, 11, 15).unwrap();
    }
    (lwd - today).num_days() as i32
}

fn person(index: usize) -> (String, String, String, String, String, Vec<String>, String, String, Option<i32>) {
    let names = ["Ananya Rao", "Meera Iyer", "Rahul Menon", "Kavya Nair", "Arjun Shah"];
    let cities = ["Bengaluru", "Pune", "Hyderabad", "Chennai", "Mumbai"];
    let companies = ["Infosys", "Wipro", "Tcs", "Hcl", "Accenture"];
    let skill_sets = [
        ("Java, Spring Boot, Kafka, JavaScript", vec!["java", "spring boot", "kafka", "javascript"]),
        ("Python, Django", vec!["python", "django"]),
        ("Java, k8s", vec!["java", "kubernetes"]),
        ("React, Node, SQL", vec!["react", "node", "sql"]),
        ("AWS, Docker, Jenkins", vec!["aws", "docker", "jenkins"]),
    ];
    let notices = [
        ("Notice period: immediate", Some(0)),
        ("Notice period: serving notice", None),
        ("Notice period: 2 months", Some(60)),
        ("Notice period: 60 days", Some(60)),
        ("Notice period: 30 days", Some(30)),
        ("Notice period: LWD 15 Nov", Some(lwd_days())),
        ("Notice: 15 days", Some(15)),
    ];
    let phones = [
        ("9876543210", "+919876543210"),
        ("+91 98765 43210", "+919876543210"),
        ("98765-43210", "+919876543210"),
        ("09876543210", "+919876543210"),
        ("", ""),
    ];
    let name = if index == 7 {
        "José Alvarez".to_string()
    } else if index % 11 == 0 {
        String::new()
    } else {
        names[index % names.len()].to_string()
    };
    let email = if index % 13 == 0 {
        String::new()
    } else {
        format!("candidate{index}@example.com")
    };
    let (phone_raw, phone) = phones[index % phones.len()];
    let city = if index % 17 == 0 { "" } else { cities[index % cities.len()] };
    let (skill_text, skills) = &skill_sets[index % skill_sets.len()];
    let company = if index % 19 == 0 { "" } else { companies[index % companies.len()] };
    let (notice_line, notice_days) = notices[index % notices.len()];
    let years = 3 + (index % 8) as i32;
    let mut body = String::new();
    if !name.is_empty() {
        body.push_str(&name);
        body.push('\n');
    }
    if !email.is_empty() {
        body.push_str(&format!("Email: {email}\n"));
        if index % 23 == 0 {
            body.push_str("Email: other.person@example.com\n");
        }
    }
    if !phone_raw.is_empty() {
        body.push_str(&format!("Phone: {phone_raw}\nAlt: 9123456780\n"));
    }
    if !city.is_empty() {
        body.push_str(&format!("Location: {city}\n"));
    }
    if !company.is_empty() {
        body.push_str(&format!("Company: {company}\n"));
    }
    body.push_str(&format!("{years} years experience\n{notice_line}\nSkills: {skill_text}\nNote: expérience\n"));
    (
        body,
        name,
        email,
        phone.to_string(),
        city.to_string(),
        skills.iter().map(|item| (*item).to_string()).collect(),
        company.to_string(),
        notice_line.to_string(),
        notice_days,
    )
}

fn write_docx(path: &Path, text: &str) {
    let mut file = File::create(path).unwrap();
    let mut writer = ZipWriter::new(&mut file);
    writer.start_file("word/document.xml", SimpleFileOptions::default()).unwrap();
    let mut xml = String::from("<w:document>");
    for line in text.lines() {
        xml.push_str("<w:p><w:r><w:t>");
        xml.push_str(&line.replace('&', "and").replace('<', " "));
        xml.push_str("</w:t></w:r></w:p>");
    }
    xml.push_str("</w:document>");
    writer.write_all(xml.as_bytes()).unwrap();
    writer.finish().unwrap();
}

fn write_rtf(path: &Path, text: &str) {
    let escaped = text.replace('\\', "").replace('{', "").replace('}', "").replace('\n', "\\par ");
    fs::write(path, format!("{{\\rtf1\\ansi {escaped}}}")).unwrap();
}

fn write_doc(path: &Path, text: &str) {
    let mut bytes = Vec::new();
    for ch in text.encode_utf16() {
        bytes.extend(ch.to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

fn pdf_bytes(text: &str) -> Vec<u8> {
    let flat = text.replace(['(', ')', '\\', '\n', '\r'], " ");
    let content = format!("BT /F1 12 Tf 72 720 Td ({flat}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{object}\nendobj\n", index + 1).into_bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n", objects.len() + 1).into_bytes());
    out.extend(b"0000000000 65535 f \n");
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").into_bytes());
    }
    out.extend(format!("trailer<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF", objects.len() + 1).into_bytes());
    out
}

fn tiny_png() -> Vec<u8> {
    b"\x89PNG\r\n\x1a\nnot-a-real-scan".to_vec()
}

#[test]
fn probe_round1_records_current_behaviour() {
    let root = std::env::temp_dir().join(format!("scout-round1-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("nested/team")).unwrap();
    fs::create_dir_all(root.join("scans")).unwrap();
    fs::create_dir_all(root.join("bad")).unwrap();
    let mut labels: Vec<Label> = Vec::new();
    let mut file_count = 0u32;

    for index in 0..120 {
        let (body, name, email, phone, location, skills, company, _notice, notice_days) = person(index);
        let rel = if index % 5 == 0 {
            format!("nested/team/resume-{index}.txt")
        } else {
            format!("resume-{index}.txt")
        };
        fs::write(root.join(&rel), &body).unwrap();
        labels.push(Label {
            rel,
            name,
            email,
            phone,
            location,
            skills,
            company,
            notice_days,
            expect_error: false,
        });
        file_count += 1;
    }

    for index in 200..220 {
        let (body, name, email, phone, location, skills, company, _, notice_days) = person(index);
        let rel = format!("word/resume-{index}.docx");
        fs::create_dir_all(root.join("word")).unwrap();
        write_docx(&root.join(&rel), &body);
        labels.push(Label { rel, name, email, phone, location, skills, company, notice_days, expect_error: false });
        file_count += 1;
    }
    for index in 220..235 {
        let (body, name, email, phone, location, skills, company, _, notice_days) = person(index);
        let rel = format!("rtf/resume-{index}.rtf");
        fs::create_dir_all(root.join("rtf")).unwrap();
        write_rtf(&root.join(&rel), &body);
        labels.push(Label { rel, name, email, phone, location, skills, company, notice_days, expect_error: false });
        file_count += 1;
    }
    for index in 235..245 {
        let (body, name, email, phone, location, skills, company, _, notice_days) = person(index);
        let rel = format!("pdf/resume-{index}.pdf");
        fs::create_dir_all(root.join("pdf")).unwrap();
        fs::write(root.join(&rel), pdf_bytes(&body)).unwrap();
        labels.push(Label { rel, name, email, phone, location, skills, company, notice_days, expect_error: false });
        file_count += 1;
    }
    for index in 245..253 {
        let (body, name, email, phone, location, skills, company, _, notice_days) = person(index);
        let rel = format!("doc/resume-{index}.doc");
        fs::create_dir_all(root.join("doc")).unwrap();
        write_doc(&root.join(&rel), &body);
        labels.push(Label { rel, name, email, phone, location, skills, company, notice_days, expect_error: false });
        file_count += 1;
    }

    let table = "Name | Priya Sharma\nEmail | priya.sharma@example.com\nPhone | 9876543210\nLocation | Bengaluru\nCompany | Infosys\n8 years experience\nNotice period: 30 days\nSkills: Java, Spring Boot\n";
    fs::write(root.join("tables/columns.txt"), table).ok();
    fs::create_dir_all(root.join("tables")).unwrap();
    fs::write(root.join("tables/columns.txt"), table).unwrap();
    labels.push(Label {
        rel: "tables/columns.txt".into(),
        name: "Priya Sharma".into(),
        email: "priya.sharma@example.com".into(),
        phone: "+919876543210".into(),
        location: "Bengaluru".into(),
        skills: vec!["java".into(), "spring boot".into()],
        company: "Infosys".into(),
        notice_days: Some(30),
        expect_error: false,
    });
    file_count += 1;

    for index in 0..8 {
        fs::write(root.join(format!("scans/scan-{index}.png")), tiny_png()).unwrap();
        labels.push(Label {
            rel: format!("scans/scan-{index}.png"),
            name: String::new(),
            email: String::new(),
            phone: String::new(),
            location: String::new(),
            skills: Vec::new(),
            company: String::new(),
            notice_days: None,
            expect_error: true,
        });
        file_count += 1;
    }
    for index in 0..6 {
        let rel = format!("bad/empty-{index}.txt");
        fs::write(root.join(&rel), "").unwrap();
        labels.push(Label {
            rel,
            name: String::new(),
            email: String::new(),
            phone: String::new(),
            location: String::new(),
            skills: Vec::new(),
            company: String::new(),
            notice_days: None,
            expect_error: true,
        });
        file_count += 1;
    }
    for index in 0..4 {
        let rel = format!("bad/corrupt-{index}.pdf");
        fs::write(root.join(&rel), b"not a pdf and not encrypted").unwrap();
        labels.push(Label {
            rel,
            name: String::new(),
            email: String::new(),
            phone: String::new(),
            location: String::new(),
            skills: Vec::new(),
            company: String::new(),
            notice_days: None,
            expect_error: true,
        });
        file_count += 1;
    }
    fs::write(root.join("bad/locked.pdf"), b"%PDF-1.4\n1 0 obj<< /Encrypt 2 0 R >>endobj\n%%EOF").unwrap();
    labels.push(Label {
        rel: "bad/locked.pdf".into(),
        name: String::new(),
        email: String::new(),
        phone: String::new(),
        location: String::new(),
        skills: Vec::new(),
        company: String::new(),
        notice_days: None,
        expect_error: true,
    });
    file_count += 1;

    let mut zip_file = File::create(root.join("pack.zip")).unwrap();
    let mut zip_writer = ZipWriter::new(&mut zip_file);
    for index in 300..312 {
        let (body, name, email, phone, location, skills, company, _, notice_days) = person(index);
        let rel = format!("pack.zip!/inside/resume-{index}.txt");
        zip_writer.start_file(format!("inside/resume-{index}.txt"), SimpleFileOptions::default()).unwrap();
        zip_writer.write_all(body.as_bytes()).unwrap();
        labels.push(Label { rel, name, email, phone, location, skills, company, notice_days, expect_error: false });
        file_count += 1;
    }
    zip_writer.start_file("../escape.txt", SimpleFileOptions::default()).unwrap();
    zip_writer.write_all(b"Name: Evil Path\nEmail: evil@example.com\n").unwrap();
    zip_writer.finish().unwrap();

    let huge = root.join("bad/huge.txt");
    {
        let mut file = File::create(&huge).unwrap();
        let chunk = vec![b'a'; 1024 * 1024];
        for _ in 0..16 {
            file.write_all(&chunk).unwrap();
        }
        file.write_all(b"\nHuge Person\nhuge.person@example.com\n9876543210\nLocation: Pune\nCompany: Wipro\n6 years experience\nNotice period: 30 days\nSkills: Java\n").unwrap();
    }
    labels.push(Label {
        rel: "bad/huge.txt".into(),
        name: "Huge Person".into(),
        email: "huge.person@example.com".into(),
        phone: "+919876543210".into(),
        location: "Pune".into(),
        skills: vec!["java".into()],
        company: "Wipro".into(),
        notice_days: Some(30),
        expect_error: true,
    });
    file_count += 1;

    let db_path = root.join("scout.db");
    let conn = Mutex::new(db::open_db(&db_path).unwrap());
    let control = Arc::new(Control::new());
    let progress = Mutex::new(Progress::default());
    let started = Instant::now();
    let saw_cancel_window = std::sync::atomic::AtomicBool::new(false);
    let ingest_result = ingest::ingest_path(&conn, &root, &control, &progress, |update| {
        if update.state == "running" {
            saw_cancel_window.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let elapsed = started.elapsed();
    let files_per_min = file_count as f64 / (elapsed.as_secs_f64() / 60.0).max(0.001);

    let started_again = Instant::now();
    ingest::ingest_path(&conn, &root, &control, &progress, |_| {}).unwrap();
    let rescan = started_again.elapsed();
    let second = progress.lock().unwrap().clone();

    let connection = conn.lock().unwrap();
    let stored = db::profiles_for_root(&connection, &root.to_string_lossy()).unwrap();
    connection.execute_batch("PRAGMA wal_checkpoint(FULL);").ok();
    drop(connection);
    let mut db_bytes = fs::read(&db_path).unwrap_or_default();
    let wal_path = PathBuf::from(format!("{}-wal", db_path.display()));
    if let Ok(wal) = fs::read(&wal_path) {
        db_bytes.extend(wal);
    }
    let plaintext_email = db_bytes.windows(12).any(|window| window == b"@example.com");
    let escape_saved = stored.iter().any(|row| row.email == "evil@example.com" || row.path.contains("escape"));

    let mut name_c = Counts::new();
    let mut email_c = Counts::new();
    let mut phone_c = Counts::new();
    let mut location_c = Counts::new();
    let mut skill_c = Counts::new();
    let mut company_c = Counts::new();
    let mut notice_c = Counts::new();
    let mut error_ok = 0;
    let mut error_bad = 0;
    let mut missing_rows = 0;
    let mut sample_misses = Vec::new();
    for label in &labels {
        let row = stored.iter().find(|item| item.path.ends_with(&label.rel) || item.path.contains(&label.rel));
        let Some(row) = row else {
            missing_rows += 1;
            if sample_misses.len() < 8 {
                sample_misses.push(format!("missing {}", label.rel));
            }
            continue;
        };
        let failed = !row.error.is_empty();
        if failed == label.expect_error {
            error_ok += 1;
        } else {
            error_bad += 1;
            if sample_misses.len() < 12 {
                sample_misses.push(format!("{} error='{}' expected_error={}", label.rel, row.error, label.expect_error));
            }
        }
        if label.expect_error {
            continue;
        }
        if sample_misses.len() < 15
            && !label.name.is_empty()
            && !label.name.eq_ignore_ascii_case(row.name.trim())
        {
            sample_misses.push(format!("name {} expected '{}' got '{}'", label.rel, label.name, row.name));
        }
        if sample_misses.len() < 20 && !label.email.is_empty() && !label.email.eq_ignore_ascii_case(row.email.trim()) {
            sample_misses.push(format!("email {} expected '{}' got '{}'", label.rel, label.email, row.email));
        }
        if sample_misses.len() < 25 && !label.company.is_empty() && !label.company.eq_ignore_ascii_case(row.company.trim()) {
            sample_misses.push(format!("company {} expected '{}' got '{}'", label.rel, label.company, row.company));
        }
        name_c.add_exact(&label.name, &row.name);
        email_c.add_exact(&label.email, &row.email);
        phone_c.add_exact(&label.phone, &row.phone);
        location_c.add_exact(&label.location, &row.location);
        company_c.add_exact(&label.company, &row.company);
        notice_c.add_option(label.notice_days, row.notice_days);
        let actual_skills: Vec<String> = row.skills.split(',').map(|item| item.trim().to_string()).filter(|item| !item.is_empty()).collect();
        for skill in &label.skills {
            if actual_skills.iter().any(|item| item == skill) {
                skill_c.tp += 1;
            } else {
                skill_c.fns += 1;
            }
        }
        for skill in &actual_skills {
            if !label.skills.iter().any(|item| item == skill) {
                skill_c.fp += 1;
            }
        }
    }

    let job = "Backend engineer, Bengaluru\nMust-have: Java, Spring Boot, Kafka\nNice-to-have: JavaScript, Kubernetes\nExperience: 5-9 years\nLocation: Bengaluru\n";
    let parsed = rank::parse_job(job);
    let strong = rank::score_profile("Java, Spring Boot, Kafka, JS", Some(8), "Bengaluru", &parsed, &Weights::default());
    let weak = rank::score_profile("Python, Django", Some(4), "Pune", &parsed, &Weights::default());
    let again = rank::score_profile("Java, Spring Boot, Kafka, JS", Some(8), "Bengaluru", &parsed, &Weights::default());
    let short = rank::parse_job("Need someone in Bengaluru soon");
    let short_score = rank::score_profile("Python", None, "Pune", &short, &Weights::default());
    let no_skill_job = rank::parse_job("Hiring now");
    let blank_score = rank::score_profile("", None, "", &no_skill_job, &Weights::default());

    let mut ranked = stored.clone();
    for row in &mut ranked {
        if row.error.is_empty() {
            let scored = rank::score_profile(&row.skills, row.years, &row.location, &parsed, &Weights::default());
            row.score = Some(scored.score);
            row.matching = scored.matching.join(", ");
            row.missing = scored.missing.join(", ");
        }
    }
    let notice_30 = filter_only(&ranked, "30", &[], "", None);
    let city = filter_only(&ranked, "any", &["Bengaluru"], "", None);
    let both = filter_only(&ranked, "30", &["Bengaluru"], "", Some(1));
    let none = filter_only(&ranked, "any", &[], "zzz-no-such-name", None);
    let special = filter_only(&ranked, "any", &[], "José", None);
    let ordered = {
        let mut rows = filter_only(&ranked, "any", &[], "", None);
        let mut ok = true;
        for pair in rows.windows(2) {
            if pair[0].score.unwrap_or(-1) < pair[1].score.unwrap_or(-1) {
                ok = false;
            }
        }
        let _ = &mut rows;
        ok
    };

    let excel_path = root.join("out.xlsx");
    let visible = crate::filter::visible_rows(ranked.clone(), &Filters { notice: "30".into(), ..Filters::default() });
    let excel_result = excel::write_xlsx(&excel_path, &visible);
    let mut headers = String::new();
    if excel_path.exists() {
        if let Ok(file) = File::open(&excel_path) {
            if let Ok(mut archive) = zip::ZipArchive::new(file) {
                if let Ok(mut sheet) = archive.by_name("xl/sharedStrings.xml") {
                    let mut xml = String::new();
                    let _ = sheet.read_to_string(&mut xml);
                    headers = xml.chars().take(800).collect();
                }
            }
        }
    }

    let isolated = profile_from_bytes("scan.png", &tiny_png());
    let empty = profile_from_bytes("empty.txt", b"");
    let text_lwd = profile_from_text("Notice period: LWD 15 Nov\n");
    let jose = profile_from_text("José Alvarez\njose@example.com\n");

    let mut report = String::new();
    report.push_str(&format!("environment=darwin local engine, rust test, synthetic only\n"));
    report.push_str(&format!("files_labeled={file_count}\n"));
    report.push_str(&format!("ingest_ok={}\n", ingest_result.is_ok()));
    report.push_str(&format!("ingest_err={:?}\n", ingest_result.as_ref().err()));
    report.push_str(&format!("elapsed_ms={}\n", elapsed.as_millis()));
    report.push_str(&format!("files_per_min={files_per_min:.1}\n"));
    report.push_str(&format!("stored_rows={}\n", stored.len()));
    report.push_str(&format!("rescan_ms={}\n", rescan.as_millis()));
    report.push_str(&format!(
        "rescan_skipped={} parsed={} errors={} state={}\n",
        second.skipped, second.parsed, second.errors, second.state
    ));
    report.push_str(&format!("plaintext_email_in_db={plaintext_email}\n"));
    report.push_str(&format!("zip_escape_saved={escape_saved}\n"));
    report.push_str(&format!("error_match={error_ok} error_mismatch={error_bad} missing_rows={missing_rows}\n"));
    report.push_str(&format!("name {}\n", name_c.line()));
    report.push_str(&format!("email {}\n", email_c.line()));
    report.push_str(&format!("phone {}\n", phone_c.line()));
    report.push_str(&format!("location {}\n", location_c.line()));
    report.push_str(&format!("skills {}\n", skill_c.line()));
    report.push_str(&format!("company {}\n", company_c.line()));
    report.push_str(&format!("notice {}\n", notice_c.line()));
    report.push_str(&format!("strong_score={} weak_score={} stable={} short_job_score={} empty_job_score={}\n", strong.score, weak.score, strong.score == again.score, short_score.score, blank_score.score));
    report.push_str(&format!("strong_rationale={}\n", strong.rationale));
    report.push_str(&format!("filter_30={} city_bengaluru={} both={} empty_name={} jose_rows={} sort_ok={}\n", notice_30.len(), city.len(), both.len(), none.len(), special.len(), ordered));
    report.push_str(&format!("excel_ok={:?} excel_rows={}\n", excel_result.is_ok(), visible.len()));
    report.push_str(&format!("excel_strings={headers}\n"));
    report.push_str(&format!("png_err={isolated:?}\n"));
    report.push_str(&format!("empty_err={empty:?}\n"));
    report.push_str(&format!("lwd_days={:?} expected_lwd={}\n", text_lwd.notice_days, lwd_days()));
    report.push_str(&format!("jose_name='{}'\n", jose.name));
    report.push_str(&format!(
        "saw_running={}\n",
        saw_cancel_window.load(std::sync::atomic::Ordering::SeqCst)
    ));
    report.push_str("misses:\n");
    for miss in &sample_misses {
        report.push_str(miss);
        report.push('\n');
    }
    let locked = stored.iter().find(|row| row.filename == "locked.pdf");
    report.push_str(&format!("locked={locked:?}\n"));
    let huge_row = stored.iter().find(|row| row.filename == "huge.txt");
    report.push_str(&format!("huge_error={:?} huge_email={:?}\n", huge_row.map(|row| row.error.clone()), huge_row.map(|row| row.email.clone())));
    let table_row = stored.iter().find(|row| row.filename == "columns.txt");
    report.push_str(&format!("table_name={:?} table_email={:?}\n", table_row.map(|row| row.name.clone()), table_row.map(|row| row.email.clone())));

    let cancel_dir = root.join("cancel-set");
    fs::create_dir_all(&cancel_dir).unwrap();
    for index in 0..80 {
        fs::write(cancel_dir.join(format!("c-{index}.txt")), person(index).0).unwrap();
    }
    let cancel_db = Mutex::new(db::open_db(&root.join("cancel.db")).unwrap());
    let cancel_control = Arc::new(Control::new());
    let cancel_progress = Mutex::new(Progress::default());
    let flag = cancel_control.clone();
    let cancel_result = ingest::ingest_path(&cancel_db, &cancel_dir, &cancel_control, &cancel_progress, move |update| {
        if update.done >= 1 {
            flag.cancel();
        }
    });
    let after_cancel = cancel_progress.lock().unwrap().clone();
    report.push_str(&format!(
        "cancel_ok={:?} cancel_state={} cancel_parsed={} cancel_total={}\n",
        cancel_result.is_ok(),
        after_cancel.state,
        after_cancel.parsed,
        after_cancel.total
    ));

    let pause_dir = root.join("pause-set");
    fs::create_dir_all(&pause_dir).unwrap();
    for index in 0..40 {
        fs::write(pause_dir.join(format!("p-{index}.txt")), person(index).0).unwrap();
    }
    let pause_db = Mutex::new(db::open_db(&root.join("pause.db")).unwrap());
    let pause_control = Arc::new(Control::new());
    let pause_progress = Mutex::new(Progress::default());
    let pause_flag = pause_control.clone();
    let resumed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let resumed_flag = resumed.clone();
    ingest::ingest_path(&pause_db, &pause_dir, &pause_control, &pause_progress, move |update| {
        if update.parsed >= 1 && !resumed_flag.swap(true, std::sync::atomic::Ordering::SeqCst) {
            pause_flag.pause();
            let control = pause_flag.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(150));
                control.resume();
            });
        }
    })
    .unwrap();
    let after_pause = pause_progress.lock().unwrap().clone();
    report.push_str(&format!(
        "pause_state={} pause_parsed={} pause_total={} pause_used={}\n",
        after_pause.state, after_pause.parsed, after_pause.total, resumed.load(std::sync::atomic::Ordering::SeqCst)
    ));

    let evidence = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/quality/round1-evidence.txt");
    fs::create_dir_all(evidence.parent().unwrap()).unwrap();
    fs::write(&evidence, &report).unwrap();
    println!("{report}");
    let _ = fs::remove_dir_all(&root);
}

fn filter_only(rows: &[Stored], notice: &str, cities: &[&str], name: &str, score_min: Option<i32>) -> Vec<Stored> {
    crate::filter::visible_rows(
        rows.to_vec(),
        &Filters {
            notice: notice.to_string(),
            locations: cities.iter().map(|city| (*city).to_string()).collect(),
            name: name.to_string(),
            score_min,
            ..Filters::default()
        },
    )
}
