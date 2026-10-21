use chrono::{Datelike, NaiveDate};
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub location: String,
    pub skills: String,
    pub company: String,
    pub years: Option<i32>,
    pub notice_raw: String,
    pub notice_days: Option<i32>,
}

impl Profile {
    pub fn blank() -> Self {
        Self {
            name: String::new(),
            email: String::new(),
            phone: String::new(),
            location: String::new(),
            skills: String::new(),
            company: String::new(),
            years: None,
            notice_raw: String::new(),
            notice_days: None,
        }
    }
}

pub fn profile_from_text(text: &str) -> Profile {
    let text = text.replace('|', ":");
    let mut profile = Profile::blank();
    profile.email = find_email(&text);
    profile.phone = find_phone(&text);
    profile.location = find_location(&text);
    profile.skills = find_skills(&text).join(", ");
    profile.company = find_company(&text);
    profile.years = find_years(&text);
    let (notice_raw, notice_days) = find_notice(&text);
    profile.notice_days = notice_days;
    profile.notice_raw = notice_label(&notice_raw, notice_days);
    profile.name = find_name(&text);
    profile
}

pub fn notice_label(raw: &str, days: Option<i32>) -> String {
    match days {
        Some(days) => format!("{days} days"),
        None if raw.to_lowercase().contains("serving") => "Serving notice".to_string(),
        None => String::new(),
    }
}

pub fn normalize_phone(value: &str) -> String {
    let digits: String = value.chars().filter(|ch| ch.is_ascii_digit()).collect();
    let digits = if digits.len() == 12 && digits.starts_with("91") {
        digits[2..].to_string()
    } else if digits.len() == 11 && digits.starts_with('0') {
        digits[1..].to_string()
    } else {
        digits
    };
    if digits.len() == 10 && matches!(digits.as_bytes().first(), Some(b'6' | b'7' | b'8' | b'9')) {
        format!("+91{digits}")
    } else {
        String::new()
    }
}

pub fn normalize_notice(value: &str) -> (String, Option<i32>) {
    let raw = value.trim();
    if raw.is_empty() {
        return (String::new(), None);
    }
    let lower = raw.to_lowercase();
    if lower.contains("serving notice") {
        return ("serving notice".to_string(), None);
    }
    if lower.contains("immediate") {
        return ("immediate".to_string(), Some(0));
    }
    let pattern = regex().notice_amount();
    if let Some(caps) = pattern.captures(&lower) {
        let count: i32 = caps[1].parse().unwrap_or(0);
        let days = match &caps[2] {
            "day" | "days" => count,
            "week" | "weeks" => count * 7,
            "month" | "months" => count * 30,
            _ => count,
        };
        return (raw.to_string(), Some(days));
    }
    (raw.to_string(), None)
}

fn find_email(text: &str) -> String {
    regex()
        .email()
        .find(text)
        .map(|item| item.as_str().to_lowercase())
        .unwrap_or_default()
}

pub fn matches_skill_query(skills: &str, query: &str) -> bool {
    let terms: Vec<String> = query
        .split(|ch: char| ch == ',' || ch == ';')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(|term| term.to_lowercase())
        .collect();
    if terms.is_empty() {
        return true;
    }
    let collapsed = skills
        .to_lowercase()
        .replace(['/', ',', ';', '|', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let haystack = format!(" {collapsed} ");
    terms.iter().all(|term| contains_term(&haystack, term))
}

fn find_phone(text: &str) -> String {
    let pattern = Regex::new(r"\+?\d[\d\s\-()]{8,18}\d").unwrap();
    for found in pattern.find_iter(text) {
        let phone = normalize_phone(found.as_str());
        if !phone.is_empty() {
            return phone;
        }
    }
    String::new()
}

fn find_name(text: &str) -> String {
    if let Some(caps) = regex().name_label().captures(text) {
        let name = first_name(&caps[1]);
        if looks_like_name(&name) {
            return name;
        }
    }
    let mut checked = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        checked += 1;
        if checked > 15 {
            break;
        }
        let name = name_before_title(line);
        if !name.is_empty() {
            return name;
        }
    }
    leading_name(text)
}

pub fn name_from_filename(filename: &str) -> String {
    let stem = filename.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(filename);
    let stem = stem.replace(['_', '-'], " ");
    let mut words = Vec::new();
    for word in stem.split_whitespace() {
        let bare: String = word.chars().filter(|ch| ch.is_alphabetic() || *ch == '\'').collect();
        if bare.chars().count() < 2 {
            continue;
        }
        if is_heading_word(&bare) {
            if words.is_empty() {
                continue;
            }
            break;
        }
        for part in split_camel(&bare) {
            if is_heading_word(&part) {
                if words.len() >= 2 {
                    break;
                }
                continue;
            }
            words.push(part);
            if words.len() == 4 {
                break;
            }
        }
        if words.len() == 4 {
            break;
        }
    }
    let name = words.join(" ");
    if looks_like_name(&name) { name } else { String::new() }
}

fn split_camel(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut parts: Vec<String> = Vec::new();
    let mut start = 0;
    for index in 1..chars.len() {
        let prev = chars[index - 1];
        let current = chars[index];
        let next = chars.get(index + 1).copied();
        let boundary = prev.is_lowercase() && current.is_uppercase()
            || prev.is_uppercase() && current.is_uppercase() && next.is_some_and(|ch| ch.is_lowercase());
        if boundary {
            parts.push(chars[start..index].iter().collect());
            start = index;
        }
    }
    parts.push(chars[start..].iter().collect());
    parts.retain(|part| part.chars().count() >= 2);
    if parts.is_empty() { vec![word.to_string()] } else { parts }
}

fn name_before_title(line: &str) -> String {
    let mut words = Vec::new();
    for word in line.split_whitespace() {
        let bare: String = word
            .chars()
            .filter(|ch| ch.is_alphabetic() || *ch == '.' || *ch == '\'')
            .collect();
        if bare.is_empty() {
            continue;
        }
        if is_heading_word(&bare) || is_label_word(&bare) {
            break;
        }
        if bare.chars().count() < 2 {
            if bare.chars().all(|ch| ch.is_uppercase()) && !words.is_empty() && words.len() < 4 {
                words.push(bare);
                continue;
            }
            break;
        }
        words.push(bare);
        if words.len() == 4 {
            break;
        }
    }
    let name = words.join(" ");
    if looks_like_name(&name) { name } else { String::new() }
}

fn first_name(value: &str) -> String {
    value
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
}

fn leading_name(text: &str) -> String {
    let mut words = Vec::new();
    let mut saw_label = false;
    for word in text.split_whitespace().take(12) {
        let lower = word.trim_matches(|ch: char| !ch.is_alphanumeric()).to_lowercase();
        if [
            "email", "phone", "mobile", "location", "company", "skills", "notice", "experience", "must", "nice", "years", "yrs",
        ]
        .iter()
        .any(|label| lower.starts_with(label))
        {
            saw_label = true;
            break;
        }
        let bare: String = word
            .chars()
            .filter(|ch| ch.is_alphabetic() || *ch == '.' || *ch == '\'')
            .collect();
        if bare.chars().count() < 2 {
            return String::new();
        }
        words.push(bare);
        if words.len() == 4 {
            break;
        }
    }
    if !saw_label || !(2..=4).contains(&words.len()) {
        return String::new();
    }
    let name = words.join(" ");
    if looks_like_name(&name) {
        name
    } else {
        String::new()
    }
}

fn looks_like_name(value: &str) -> bool {
    let words: Vec<&str> = value.split_whitespace().collect();
    (2..=4).contains(&words.len())
        && words.iter().all(|word| {
            word.chars().count() <= 30
                && word.chars().all(|ch| ch.is_alphabetic() || ch == '.' || ch == '\'')
                && !is_heading_word(word)
        })
        && !value.contains('@')
}

fn is_label_word(word: &str) -> bool {
    let lower = word.trim_matches('.').to_lowercase();
    ["email", "phone", "mobile", "location", "company", "skills", "notice", "experience", "linkedin", "github"]
        .iter()
        .any(|label| lower.starts_with(label))
}

fn is_heading_word(word: &str) -> bool {
    let lower = word.trim_matches('.').to_lowercase();
    [
        "professional", "summary", "skills", "skill", "experience", "education", "resume", "curriculum", "vitae",
        "profile", "objective", "career", "technical", "project", "projects", "contact", "personal", "details",
        "developer", "engineer", "engineering", "backend", "frontend", "fullstack", "software", "consultant",
        "manager", "architect", "information", "about", "work", "history", "certification", "certifications",
        "achievements", "strengths", "languages", "declaration", "overview", "senior", "junior", "lead", "intern",
        "java", "python", "sql", "linkedin", "github", "portfolio", "seeking", "dba", "devops", "data", "etl",
        "specialist", "associate", "analyst", "executive", "university", "college", "bachelor", "master", "tools",
        "servers", "databases", "stack", "full", "naukri", "updated", "resume", "cv", "role", "responsibilities", "responsibility", "duties",
        "duration", "client", "description", "environment", "organization", "organisation", "designation", "month", "year",
    ]
    .iter()
    .any(|heading| lower == *heading)
}

fn find_company(text: &str) -> String {
    if let Some(caps) = regex().company_label().captures(text) {
        let company = clean_company(&trim_company(caps[1].trim()));
        if !company.is_empty() {
            return company;
        }
    }
    if let Some(company) = first_clean_company(text, regex().company_before_present()) {
        return company;
    }
    if let Some(company) = first_clean_company(text, regex().company_after_present()) {
        return company;
    }
    let latest = company_with_latest_year(text);
    if !latest.is_empty() {
        return latest;
    }
    String::new()
}

fn first_clean_company(text: &str, pattern: &Regex) -> Option<String> {
    for caps in pattern.captures_iter(text) {
        let company = clean_company(caps.get(1).map(|item| item.as_str()).unwrap_or("").trim());
        if !company.is_empty() {
            return Some(company);
        }
    }
    None
}

fn company_with_latest_year(text: &str) -> String {
    let mut best: Option<(i32, String)> = None;
    for caps in regex().company_in_city().captures_iter(text) {
        let company = clean_company(caps.get(1).map(|item| item.as_str()).unwrap_or("").trim());
        if company.is_empty() {
            continue;
        }
        let end = caps.get(0).map(|item| item.end()).unwrap_or(0);
        let window = text.get(end..).unwrap_or("");
        let window = &window[..window.find('\n').unwrap_or(window.len()).min(80)];
        let mut latest = 0;
        for found in regex().year().find_iter(window) {
            if let Ok(year) = found.as_str().parse::<i32>() {
                latest = latest.max(year);
            }
        }
        if best.as_ref().map(|(year, _)| latest >= *year).unwrap_or(true) {
            best = Some((latest, company));
        }
    }
    best.map(|(_, company)| company).unwrap_or_default()
}

fn clean_company(value: &str) -> String {
    let mut words: Vec<&str> = value.split_whitespace().collect();
    while words.first().is_some_and(|word| is_heading_word(word) || is_label_word(word)) {
        words.remove(0);
    }
    while words.last().is_some_and(|word| is_city_word(word) || word.chars().all(|ch| !ch.is_alphanumeric())) {
        words.pop();
    }
    let company = words.join(" ");
    let company = company.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '&').trim().to_string();
    if company.chars().count() < 2 || company.chars().count() > 60 || is_heading_word(&company) || is_city_word(&company) {
        String::new()
    } else {
        company
    }
}

fn is_city_word(word: &str) -> bool {
    let lower = word.trim_matches(|ch: char| !ch.is_alphanumeric()).to_lowercase();
    CITIES.iter().any(|(alias, _)| *alias == lower)
}

fn trim_company(value: &str) -> String {
    let lower = value.to_lowercase();
    let mut end = value.len();
    for stop in [" years", " yrs", " notice", " skill", " email", " phone", " location", " experience"] {
        if let Some(index) = lower.find(stop) {
            end = end.min(index);
        }
    }
    let cut = value.get(..end).unwrap_or(value).trim();
    cut.trim_end_matches(|ch: char| ch.is_ascii_digit() || ch.is_whitespace())
        .trim()
        .to_string()
}

fn find_years(text: &str) -> Option<i32> {
    for caps in regex().years().captures_iter(text) {
        let start = caps.get(0).map(|item| item.start()).unwrap_or(0);
        let previous = text[..start].chars().next_back();
        if previous.is_some_and(|ch| ch.is_ascii_digit() || ch == '.') {
            continue;
        }
        if let Some(years) = round_years(&caps[1]) {
            return Some(years);
        }
    }
    None
}

pub fn round_years(value: &str) -> Option<i32> {
    let years: f32 = value.parse().ok()?;
    let years = years.round() as i32;
    if (0..=50).contains(&years) { Some(years) } else { None }
}

fn find_notice(text: &str) -> (String, Option<i32>) {
    if let Some(days) = find_lwd_days(text) {
        return (format!("{days} days"), Some(days));
    }
    let lower = text.to_lowercase();
    if let Some(index) = lower.find("notice") {
        let slice = &text[index..];
        let end = slice.find('\n').unwrap_or(slice.len().min(80));
        let normalized = normalize_notice(&slice[..end]);
        if normalized.1.is_some() || !normalized.0.is_empty() {
            return normalized;
        }
    }
    for line in text.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("notice") || line_lower.contains("joining") || line_lower.contains("joiner") {
            let normalized = normalize_notice(line);
            if normalized.1.is_some() || !normalized.0.is_empty() {
                return normalized;
            }
        }
    }
    (String::new(), None)
}

fn find_lwd_days(text: &str) -> Option<i32> {
    let lower = text.to_lowercase();
    if !lower.contains("lwd") && !lower.contains("last working") {
        return None;
    }
    if let Some(caps) = regex().lwd_day_month().captures(text) {
        let day: u32 = caps[1].parse().ok()?;
        let month = month_number(&caps[2])?;
        let year = caps.get(3).and_then(|item| item.as_str().parse().ok());
        return days_until(day, month, year);
    }
    if let Some(caps) = regex().lwd_month_day().captures(text) {
        let month = month_number(&caps[1])?;
        let day: u32 = caps[2].parse().ok()?;
        let year = caps.get(3).and_then(|item| item.as_str().parse().ok());
        return days_until(day, month, year);
    }
    None
}

fn month_number(value: &str) -> Option<u32> {
    let value = value.to_lowercase();
    let months = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    months.iter().position(|month| value.starts_with(month)).map(|index| index as u32 + 1)
}

fn days_until(day: u32, month: u32, year: Option<i32>) -> Option<i32> {
    let today = chrono::Local::now().date_naive();
    let year = match year {
        Some(year) => year,
        None => {
            let candidate = NaiveDate::from_ymd_opt(today.year(), month, day)?;
            if candidate < today {
                today.year() + 1
            } else {
                today.year()
            }
        }
    };
    let lwd = NaiveDate::from_ymd_opt(year, month, day)?;
    let days = (lwd - today).num_days() as i32;
    Some(if days < 0 { 0 } else { days })
}

pub fn find_location(text: &str) -> String {
    if let Some(caps) = regex().location_label().captures(text) {
        let labeled = first_city(&caps[1]);
        if !labeled.is_empty() {
            return labeled;
        }
    }
    let header = header_contact_city(text);
    if !header.is_empty() {
        return header;
    }
    let latest = city_by_latest_year(text);
    if !latest.is_empty() {
        return latest;
    }
    first_city(text)
}

fn header_contact_city(text: &str) -> String {
    let mut count = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        count += 1;
        if count > 8 {
            break;
        }
        let city = first_city(line);
        if city.is_empty() {
            continue;
        }
        let lower = line.to_lowercase();
        let digits = line.chars().filter(|ch| ch.is_ascii_digit()).count();
        let contact = lower.contains('@') || lower.contains("+91") || digits >= 10;
        if contact {
            return city;
        }
    }
    String::new()
}

fn city_by_latest_year(text: &str) -> String {
    let mut best: Option<(i32, String)> = None;
    for line in text.lines() {
        let city = first_city(line);
        if city.is_empty() {
            continue;
        }
        let mut latest = 0;
        for found in regex().year().find_iter(line) {
            if let Ok(year) = found.as_str().parse::<i32>() {
                latest = latest.max(year);
            }
        }
        if latest == 0 {
            continue;
        }
        if best.as_ref().map(|(year, _)| latest >= *year).unwrap_or(true) {
            best = Some((latest, city));
        }
    }
    best.map(|(_, city)| city).unwrap_or_default()
}

fn first_city(text: &str) -> String {
    let folded = text.to_lowercase().replace(['\n', '\r', ',', ';', ':', '/', '|', '.'], " ");
    let padded = format!(" {folded} ");
    let mut best: Option<(usize, &str)> = None;
    for (alias, city) in CITIES {
        if let Some(index) = padded.find(&format!(" {alias} ")) {
            if best.map(|(at, _)| index < at).unwrap_or(true) {
                best = Some((index, *city));
            }
        }
    }
    best.map(|(_, city)| city.to_string()).unwrap_or_default()
}

pub fn find_skills(text: &str) -> Vec<String> {
    let lower = format!(
        " {} ",
        text.to_lowercase()
            .replace(|ch: char| matches!(ch, '/' | ',' | ';' | '|' | '\n' | '\r' | '.' | '(' | ')' | ':' | '[' | ']' | '•' | '-' | '–' | '—'), " ")
    );
    let mut found = Vec::new();
    for (canonical, aliases) in SKILLS {
        if aliases.iter().any(|alias| contains_term(&lower, alias)) && !found.contains(&(*canonical).to_string())
        {
            found.push((*canonical).to_string());
        }
    }
    let snapshot = found.clone();
    found.retain(|skill| {
        !snapshot
            .iter()
            .any(|other| other != skill && other.starts_with(&format!("{skill} ")))
    });
    found
}

fn contains_term(haystack: &str, term: &str) -> bool {
    haystack.contains(&format!(" {term} "))
}

const CITIES: &[(&str, &str)] = &[
    ("bengaluru", "Bengaluru"),
    ("bangalore", "Bengaluru"),
    ("hyderabad", "Hyderabad"),
    ("pune", "Pune"),
    ("chennai", "Chennai"),
    ("mumbai", "Mumbai"),
    ("delhi", "Delhi"),
    ("noida", "Noida"),
    ("gurugram", "Gurugram"),
    ("gurgaon", "Gurugram"),
    ("kolkata", "Kolkata"),
    ("ahmedabad", "Ahmedabad"),
    ("jaipur", "Jaipur"),
    ("kochi", "Kochi"),
    ("indore", "Indore"),
    ("chandigarh", "Chandigarh"),
    ("coimbatore", "Coimbatore"),
    ("mysuru", "Mysuru"),
    ("mysore", "Mysuru"),
    ("visakhapatnam", "Visakhapatnam"),
    ("vizag", "Visakhapatnam"),
];

const SKILLS: &[(&str, &[&str])] = &[
    ("spring boot", &["spring boot", "springboot"]),
    ("spring mvc", &["spring mvc", "springmvc"]),
    ("javascript", &["javascript", "js"]),
    ("typescript", &["typescript", "ts"]),
    ("kubernetes", &["kubernetes", "k8s"]),
    ("postgresql", &["postgresql", "postgres"]),
    ("node", &["node", "node.js", "nodejs"]),
    ("react", &["react", "reactjs"]),
    ("spring", &["spring"]),
    ("java", &["java"]),
    ("kafka", &["kafka"]),
    ("python", &["python"]),
    ("django", &["django"]),
    ("docker", &["docker"]),
    ("aws", &["aws"]),
    ("mysql", &["mysql", "my sql"]),
    ("oracle", &["oracle"]),
    ("sql", &["sql", "mysql", "my sql", "oracle", "postgresql", "postgres"]),
    ("html", &["html"]),
    ("css", &["css"]),
    ("angular", &["angular"]),
    ("hibernate", &["hibernate"]),
    ("microservices", &["microservices", "microservice"]),
    ("rest", &["rest", "restful", "rest api", "rest apis"]),
    ("redis", &["redis"]),
    ("mongodb", &["mongodb"]),
    ("jenkins", &["jenkins"]),
    ("git", &["git"]),
    ("linux", &["linux"]),
    ("selenium", &["selenium"]),
    ("azure", &["azure"]),
    ("j2ee", &["j2ee", "java ee"]),
    ("jsf", &["jsf"]),
    ("tomcat", &["tomcat"]),
    ("informatica", &["informatica"]),
    ("idmc", &["idmc", "informatica data management cloud"]),
    ("powercenter", &["powercenter", "power center"]),
    ("iics", &["iics", "informatica cloud"]),
    ("talend", &["talend"]),
    ("etl", &["etl"]),
    ("ssis", &["ssis"]),
    ("plsql", &["plsql", "pl sql"]),
];

struct Patterns {
    email: Regex,
    notice_amount: Regex,
    name_label: Regex,
    company_label: Regex,
    company_before_present: Regex,
    company_after_present: Regex,
    company_in_city: Regex,
    location_label: Regex,
    years: Regex,
    year: Regex,
    lwd_day_month: Regex,
    lwd_month_day: Regex,
}

impl Patterns {
    fn email(&self) -> &Regex {
        &self.email
    }
    fn notice_amount(&self) -> &Regex {
        &self.notice_amount
    }
    fn name_label(&self) -> &Regex {
        &self.name_label
    }
    fn company_label(&self) -> &Regex {
        &self.company_label
    }
    fn company_before_present(&self) -> &Regex {
        &self.company_before_present
    }
    fn company_after_present(&self) -> &Regex {
        &self.company_after_present
    }
    fn company_in_city(&self) -> &Regex {
        &self.company_in_city
    }
    fn location_label(&self) -> &Regex {
        &self.location_label
    }
    fn years(&self) -> &Regex {
        &self.years
    }
    fn year(&self) -> &Regex {
        &self.year
    }
    fn lwd_day_month(&self) -> &Regex {
        &self.lwd_day_month
    }
    fn lwd_month_day(&self) -> &Regex {
        &self.lwd_month_day
    }
}

fn regex() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        email: Regex::new(r"(?i)[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.(?:co\.in|com|org|net|edu|gov|info|io|dev|in|me|ai|uk|us|co)").unwrap(),
        notice_amount: Regex::new(r"(\d+)\s*(day|days|week|weeks|month|months)").unwrap(),
        name_label: Regex::new(r"(?i)name\s*[:\-]\s*([^\n]{2,80})").unwrap(),
        company_label: Regex::new(
            r"(?i)(?:current company|previous company|company|employer)\s*[:\-]\s*([A-Za-z0-9][A-Za-z0-9 .&-]{1,60})",
        )
        .unwrap(),
        company_before_present: Regex::new(
            r"(?i)([A-Za-z][A-Za-z0-9&.' ]{1,50}?)\s*(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?\s*\d{4}\s*[–—\-]\s*(?:present|till now|till date)",
        )
        .unwrap(),
        company_after_present: Regex::new(
            r"(?i)(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?\s+\d{4}\s*[–—\-]\s*present\s+([A-Za-z][A-Za-z0-9&.' ]{2,60})",
        )
        .unwrap(),
        company_in_city: Regex::new(
            r"(?i)\bin\s+([A-Z][A-Za-z0-9&.' ]{1,40}?),\s*(?:bengaluru|bangalore|hyderabad|pune|chennai|mumbai|delhi|noida|gurugram|gurgaon|kolkata)\b",
        )
        .unwrap(),
        location_label: Regex::new(r"(?i)(?:current location|location|city|based in)\s*[:\-]\s*([^\n]{2,60})").unwrap(),
        years: Regex::new(r"(?i)(\d{1,2}(?:\.\d+)?)\s*\+?\s*(?:years|yrs)\b").unwrap(),
        year: Regex::new(r"(?i)(?:19|20)\d{2}").unwrap(),
        lwd_day_month: Regex::new(r"(?i)(\d{1,2})\s*(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?\s*(\d{4})?").unwrap(),
        lwd_month_day: Regex::new(r"(?i)(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?\s+(\d{1,2})(?:\s*,?\s*(\d{4}))?").unwrap(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indian_phone_formats() {
        assert_eq!(normalize_phone("+91 98765 43210"), "+919876543210");
        assert_eq!(normalize_phone("98765-43210"), "+919876543210");
        assert_eq!(normalize_phone("09876543210"), "+919876543210");
        assert_eq!(normalize_phone("12345"), "");
    }

    #[test]
    fn notice_periods() {
        assert_eq!(normalize_notice("immediate").1, Some(0));
        assert_eq!(normalize_notice("30 days").1, Some(30));
        assert_eq!(normalize_notice("2 months").1, Some(60));
        let (raw, days) = normalize_notice("serving notice");
        assert_eq!(raw, "serving notice");
        assert_eq!(days, None);
    }

    #[test]
    fn last_working_day_and_unicode_name() {
        let profile = profile_from_text("José Alvarez\njose@example.com\nNotice period: LWD 15 Nov\n");
        assert_eq!(profile.name, "José Alvarez");
        assert_eq!(profile.notice_days, Some(days_until(15, 11, None).unwrap()));
        assert!(profile.notice_raw.ends_with(" days"));
        let table = profile_from_text("Name | Priya Sharma\nEmail | priya.sharma@example.com\nCompany: Infosys 6 years experience Notice period: 30 days\n");
        assert_eq!(table.name, "Priya Sharma");
        assert_eq!(table.company, "Infosys");
        assert_eq!(table.notice_days, Some(30));
        assert_eq!(table.notice_raw, "30 days");
    }

    #[test]
    fn does_not_invent_a_blank_phone_or_name() {
        let profile = profile_from_text("Skills: Python\nLocation: Pune\n");
        assert_eq!(profile.phone, "");
        assert_eq!(profile.name, "");
        assert_eq!(profile.location, "Pune");
        assert_eq!(profile.skills, "python");
    }

    #[test]
    fn names_emails_years_notice_and_city_follow_the_resume() {
        let heading = profile_from_text("Aparna Kalamkuntla Email ID: aparna.kalamkuntla08@gmail.com\nProfessional Summary\nSenior Consultant in Capgemini, Bangalore from June 2023 to July 2026\nGenpact, Hyderabad from August 2018 to July 2021\n");
        assert_eq!(heading.name, "Aparna Kalamkuntla");
        assert_eq!(heading.company, "Capgemini");
        assert_eq!(heading.location, "Bengaluru");
        let kiran = profile_from_text("GOWLA KIRAN Java Backend Developer\nkiran.g.dev28@gmail.com\nJava backend developer with 4.1 years of experience\nJava Developer – Tricon InfotechFeb 2026 – Present\n");
        assert_eq!(kiran.name, "GOWLA KIRAN");
        assert_eq!(kiran.years, Some(4));
        assert_eq!(kiran.company, "Tricon Infotech");
        let email = profile_from_text("Chandan Kumar\nchandankrs273@gmail.comph\nHaving 10+ years of experience\n");
        assert_eq!(email.email, "chandankrs273@gmail.com");
        assert_eq!(email.years, Some(10));
        let satish = profile_from_text("SATISH CHAURASIA\nMumbai, Maharashtra | +91 9769056122 | satish@example.com\n15+ years of experience\nUniversity of Pune\n");
        assert_eq!(satish.location, "Mumbai");
        assert_eq!(satish.years, Some(15));
        let shivani = profile_from_text("Shivani Waghe\nPune\nimplementing both immediate and long-term solutions.\n");
        assert_eq!(shivani.notice_days, None);
        assert!(shivani.notice_raw.is_empty());
        let joiner = profile_from_text("Anita Rao\nanita@example.com\nNotice period: immediate\n");
        assert_eq!(joiner.notice_days, Some(0));
        assert!(matches_skill_query("javascript, node", "Java") == false);
        assert!(matches_skill_query("java, spring boot", "Java"));
        let skills = find_skills("Spring Boot, Spring MVC and REST APIs. SQL, preferably MySQL and Oracle.");
        assert!(skills.contains(&"spring mvc".to_string()));
        assert!(skills.contains(&"rest".to_string()));
        assert!(skills.contains(&"mysql".to_string()));
        assert!(skills.contains(&"oracle".to_string()));
        let aparna = find_skills("ETL Tools: Informatica, Talend and IDMC. Environment: Informatica power center, IDMC.");
        assert!(aparna.contains(&"informatica".to_string()));
        assert!(aparna.contains(&"idmc".to_string()));
        assert!(aparna.contains(&"powercenter".to_string()));
        assert!(aparna.contains(&"talend".to_string()));
        assert!(aparna.contains(&"etl".to_string()));
        let saved = aparna.join(", ");
        assert!(matches_skill_query(&saved, "IDMC, Informatica"));
    }
}
