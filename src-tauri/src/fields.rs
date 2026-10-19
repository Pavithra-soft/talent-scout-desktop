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
    for line in text.lines() {
        let line = line.trim();
        if looks_like_name(line) && !line.to_lowercase().starts_with("must") {
            return line.to_string();
        }
    }
    leading_name(text)
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
        })
        && !value.contains('@')
}

fn find_company(text: &str) -> String {
    if let Some(caps) = regex().company_label().captures(text) {
        return trim_company(caps[1].trim());
    }
    String::new()
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
    regex().years().captures(text).and_then(|caps| {
        let years: i32 = caps[1].parse().ok()?;
        if (0..=50).contains(&years) {
            Some(years)
        } else {
            None
        }
    })
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
    if lower.contains("immediate") {
        return ("immediate".to_string(), Some(0));
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
    let folded = text.to_lowercase().replace(['\n', '\r', ',', ';', ':', '/', '|'], " ");
    let lower = format!(" {folded} ");
    for (alias, city) in CITIES {
        if lower.contains(&format!(" {alias} ")) {
            return (*city).to_string();
        }
    }
    String::new()
}

pub fn find_skills(text: &str) -> Vec<String> {
    let lower = format!(" {} ", text.to_lowercase().replace(['/', ',', ';', '|', '\n'], " "));
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
];

const SKILLS: &[(&str, &[&str])] = &[
    ("spring boot", &["spring boot", "springboot"]),
    ("javascript", &["javascript", "js"]),
    ("typescript", &["typescript", "ts"]),
    ("kubernetes", &["kubernetes", "k8s"]),
    ("postgresql", &["postgresql", "postgres"]),
    ("node", &["node", "node.js", "nodejs"]),
    ("react", &["react", "reactjs"]),
    ("java", &["java"]),
    ("kafka", &["kafka"]),
    ("python", &["python"]),
    ("django", &["django"]),
    ("docker", &["docker"]),
    ("aws", &["aws"]),
    ("sql", &["sql"]),
    ("html", &["html"]),
    ("css", &["css"]),
    ("angular", &["angular"]),
    ("hibernate", &["hibernate"]),
    ("microservices", &["microservices"]),
    ("redis", &["redis"]),
    ("mongodb", &["mongodb"]),
    ("jenkins", &["jenkins"]),
    ("git", &["git"]),
    ("linux", &["linux"]),
    ("selenium", &["selenium"]),
    ("azure", &["azure"]),
];

struct Patterns {
    email: Regex,
    notice_amount: Regex,
    name_label: Regex,
    company_label: Regex,
    years: Regex,
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
    fn years(&self) -> &Regex {
        &self.years
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
        email: Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(),
        notice_amount: Regex::new(r"(\d+)\s*(day|days|week|weeks|month|months)").unwrap(),
        name_label: Regex::new(r"(?i)name\s*[:\-]\s*([^\n]{2,80})").unwrap(),
        company_label: Regex::new(
            r"(?i)(?:current company|previous company|company|employer)\s*[:\-]\s*([A-Za-z0-9][A-Za-z0-9 .&-]{1,60})",
        )
        .unwrap(),
        years: Regex::new(r"(?i)(\d{1,2})\s*\+?\s*(?:years|yrs)\b").unwrap(),
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
}
