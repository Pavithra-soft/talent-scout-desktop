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
    let mut profile = Profile::blank();
    profile.email = find_email(text);
    profile.phone = find_phone(text);
    profile.location = find_location(text);
    profile.skills = find_skills(text).join(", ");
    profile.company = find_company(text);
    profile.years = find_years(text);
    let (notice_raw, notice_days) = find_notice(text);
    profile.notice_raw = notice_raw;
    profile.notice_days = notice_days;
    profile.name = find_name(text);
    profile
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
        let name = caps[1].trim();
        if looks_like_name(name) {
            return name.to_string();
        }
    }
    for line in text.lines() {
        let line = line.trim();
        if looks_like_name(line) && !line.to_lowercase().starts_with("must") {
            return line.to_string();
        }
    }
    String::new()
}

fn looks_like_name(value: &str) -> bool {
    let words: Vec<&str> = value.split_whitespace().collect();
    (2..=4).contains(&words.len())
        && words.iter().all(|word| {
            word.chars()
                .all(|ch| ch.is_ascii_alphabetic() || ch == '.' || ch == '\'')
        })
        && !value.contains('@')
}

fn find_company(text: &str) -> String {
    if let Some(caps) = regex().company_label().captures(text) {
        return caps[1].trim().to_string();
    }
    String::new()
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
    let lower = text.to_lowercase();
    if let Some(index) = lower.find("notice") {
        let slice = &text[index..];
        let end = slice.find('\n').unwrap_or(slice.len().min(80));
        return normalize_notice(&slice[..end]);
    }
    if lower.contains("immediate joiner") || lower.contains("immediate") {
        return ("immediate".to_string(), Some(0));
    }
    (String::new(), None)
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
}

fn regex() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        email: Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(),
        notice_amount: Regex::new(r"(\d+)\s*(day|days|week|weeks|month|months)").unwrap(),
        name_label: Regex::new(r"(?i)name\s*[:\-]\s*([A-Za-z][A-Za-z .']{2,60})").unwrap(),
        company_label: Regex::new(
            r"(?i)(?:current company|previous company|company|employer)\s*[:\-]\s*([A-Za-z0-9][A-Za-z0-9 .&-]{1,60})",
        )
        .unwrap(),
        years: Regex::new(r"(?i)(\d{1,2})\s*\+?\s*(?:years|yrs)\b").unwrap(),
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
    fn does_not_invent_a_blank_phone_or_name() {
        let profile = profile_from_text("Skills: Python\nLocation: Pune\n");
        assert_eq!(profile.phone, "");
        assert_eq!(profile.name, "");
        assert_eq!(profile.location, "Pune");
        assert_eq!(profile.skills, "python");
    }
}
