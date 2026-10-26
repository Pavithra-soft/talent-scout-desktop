use serde::{Deserialize, Serialize};

use crate::fields::find_skills;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weights {
    pub must: i32,
    pub nice: i32,
    pub experience: i32,
    pub location: i32,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            must: 60,
            nice: 20,
            experience: 10,
            location: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobNeeds {
    pub must_have: Vec<String>,
    pub must_aliases: Vec<Vec<String>>,
    pub nice_to_have: Vec<String>,
    pub nice_aliases: Vec<Vec<String>>,
    pub min_years: Option<i32>,
    pub max_years: Option<i32>,
    pub locations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Score {
    pub score: i32,
    pub matching: Vec<String>,
    pub missing: Vec<String>,
    pub rationale: String,
}

pub fn parse_job(text: &str) -> JobNeeds {
    let lower = text.to_lowercase();
    let (must_text, nice_text) = split_sections(&lower, text);
    let must_have = if must_text.is_empty() && nice_text.is_empty() {
        skill_terms(text)
    } else {
        skill_terms(&must_text)
    };
    let mut nice_to_have = skill_terms(&nice_text);
    nice_to_have.retain(|skill| !must_have.contains(skill));
    let must_aliases = must_have.iter().map(|skill| vec![skill.clone()]).collect();
    let nice_aliases = nice_to_have.iter().map(|skill| vec![skill.clone()]).collect();
    let (min_years, max_years) = experience_range(text);
    let locations = crate::fields::find_location(text);
    let locations = if locations.is_empty() {
        Vec::new()
    } else {
        vec![locations]
    };
    JobNeeds {
        must_have,
        must_aliases,
        nice_to_have,
        nice_aliases,
        min_years,
        max_years,
        locations,
    }
}

pub fn score_profile(
    skills: &str,
    years: Option<i32>,
    location: &str,
    job: &JobNeeds,
    weights: &Weights,
) -> Score {
    let profile_skills = find_skills(skills);
    let has = |skill: &str| profile_skills.iter().any(|item| item == skill) || term_in(skills, skill);
    let matched_must = matched_groups(&job.must_have, &job.must_aliases, &has);
    let matched_nice = matched_groups(&job.nice_to_have, &job.nice_aliases, &has);
    let mut missing = Vec::new();
    for skill in job.must_have.iter().chain(job.nice_to_have.iter()) {
        if !matched_must.iter().any(|item| item == skill || item.starts_with(&format!("{skill} (")))
            && !matched_nice.iter().any(|item| item == skill || item.starts_with(&format!("{skill} (")))
        {
            missing.push(skill.clone());
        }
    }
    let listed_skills = !job.must_have.is_empty() || !job.nice_to_have.is_empty();
    let listed_experience = job.min_years.is_some() || job.max_years.is_some();
    let listed_location = !job.locations.is_empty();
    if !listed_skills && !listed_experience && !listed_location {
        return Score {
            score: 0,
            matching: Vec::new(),
            missing: Vec::new(),
            rationale: "This job did not list skills, experience, or a city, so no score was given. Add those details and rank again.".to_string(),
        };
    }
    let must_ratio = if job.must_have.is_empty() { 0.0 } else { ratio(matched_must.len(), job.must_have.len()) };
    let nice_ratio = if job.nice_to_have.is_empty() { 0.0 } else { ratio(matched_nice.len(), job.nice_to_have.len()) };
    // Secondary skills raise the score after primary skills match. A full secondary list cannot outrank a primary match.
    let nice_credit = if job.must_have.is_empty() { nice_ratio } else { nice_ratio * (0.2 + 0.8 * must_ratio) };
    let experience_ratio = experience_fit(years, job.min_years, job.max_years);
    let location_ratio = location_fit(location, &job.locations);
    let weight_must = if job.must_have.is_empty() { 0 } else { weights.must };
    let weight_nice = if job.nice_to_have.is_empty() { 0 } else { weights.nice };
    let weight_experience = if listed_experience { weights.experience } else { 0 };
    let weight_location = if listed_location { weights.location } else { 0 };
    let total = (weight_must + weight_nice + weight_experience + weight_location).max(1) as f32;
    let points = weight_must as f32 * must_ratio
        + weight_nice as f32 * nice_credit
        + weight_experience as f32 * experience_ratio
        + weight_location as f32 * location_ratio;
    let score = ((100.0 * points / total).round() as i32).clamp(0, 100);
    let mut matching = matched_must.clone();
    matching.extend(matched_nice.iter().cloned());
    let experience_note = if listed_experience {
        experience_line(experience_ratio, years)
    } else {
        "Experience was not requested."
    };
    let location_note = if listed_location {
        location_line(location_ratio, location)
    } else {
        "A city was not requested."
    };
    let rationale = if !listed_skills {
        format!("This job did not list skills, so skills were not scored. {experience_note} {location_note}")
    } else {
        format!(
            "Matches {} of {} primary skills{}. {experience_note} {location_note}",
            matched_must.len(),
            job.must_have.len(),
            if job.nice_to_have.is_empty() {
                String::new()
            } else {
                format!(" and {} of {} secondary skills", matched_nice.len(), job.nice_to_have.len())
            }
        )
    };
    Score {
        score,
        matching,
        missing,
        rationale,
    }
}

fn matched_groups(names: &[String], aliases: &[Vec<String>], has: &dyn Fn(&str) -> bool) -> Vec<String> {
    names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            let group = aliases.get(index).map(|items| items.as_slice()).unwrap_or(&[]);
            let hit = group.iter().find(|term| has(term)).or(if has(name) { Some(name) } else { None });
            hit.map(|term| if term == name { name.clone() } else { format!("{name} ({term})") })
        })
        .collect()
}

fn ratio(matched: usize, total: usize) -> f32 {
    if total == 0 {
        1.0
    } else {
        matched as f32 / total as f32
    }
}

fn experience_fit(years: Option<i32>, min_years: Option<i32>, max_years: Option<i32>) -> f32 {
    let Some(years) = years else {
        return 0.0;
    };
    let below = min_years.unwrap_or(0);
    let above = max_years.unwrap_or(years);
    if years >= below && years <= above {
        1.0
    } else if (years - below).abs() <= 2 || (years - above).abs() <= 2 {
        0.5
    } else {
        0.0
    }
}

fn location_fit(location: &str, wanted: &[String]) -> f32 {
    if wanted.is_empty() {
        return 1.0;
    }
    if location.is_empty() {
        return 0.0;
    }
    if wanted.iter().any(|city| city.eq_ignore_ascii_case(location)) {
        1.0
    } else {
        0.0
    }
}

fn experience_line(ratio: f32, years: Option<i32>) -> &'static str {
    if years.is_none() {
        "Experience was not found."
    } else if ratio >= 1.0 {
        "Experience fits."
    } else if ratio > 0.0 {
        "Experience is close."
    } else {
        "Experience is outside the range."
    }
}

fn location_line(ratio: f32, location: &str) -> &'static str {
    if location.is_empty() {
        "Location was not found."
    } else if ratio >= 1.0 {
        "Location matches."
    } else {
        "Location does not match."
    }
}

fn section_at(lower: &str, labels: &[&str]) -> Option<usize> {
    labels.iter().filter_map(|label| lower.find(label)).min()
}

fn skill_terms(section: &str) -> Vec<String> {
    let mut terms = find_skills(section);
    let lowered = section.to_lowercase();
    let listed = lowered.contains(',') || lowered.contains(';') || lowered.contains('\n') || lowered.contains('•');
    for part in lowered.split(|ch: char| matches!(ch, ',' | ';' | '|' | '\n' | '/' | '•' | '·')) {
        for piece in part.split(" and ") {
            let term = clean_term(piece);
            if term.is_empty() || terms.iter().any(|have| have == &term) || is_noise(&term) || !find_skills(&term).is_empty() {
                continue;
            }
            let words = term.split_whitespace().count();
            let single_tool = words == 1 && term.len() <= 24 && term.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '+' || ch == '#');
            if single_tool || (listed && words <= 3) {
                terms.push(term);
            }
        }
    }
    terms
}

fn clean_term(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    for prefix in [
        "primary skills:",
        "primary skill:",
        "secondary skills:",
        "secondary skill:",
        "primary skills",
        "primary skill",
        "secondary skills",
        "secondary skill",
        "must-have:",
        "must-have",
        "must have:",
        "must have",
        "nice-to-have:",
        "nice-to-have",
        "nice to have:",
        "nice to have",
        "required skills:",
        "required skills",
        "preferred skills:",
        "preferred skills",
        "core skills:",
        "core skills",
        "key skills:",
        "key skills",
        "good to have:",
        "good to have",
        "optional skills:",
        "optional skills",
        "additional skills:",
        "additional skills",
        "essential skills:",
        "essential skills",
        "mandatory skills:",
        "mandatory skills",
        "preferably",
        "preferred:",
        "preferred",
        "primary:",
        "secondary:",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim().to_string();
        }
    }
    text.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '+' && ch != '#' && ch != '.')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_noise(term: &str) -> bool {
    let words: Vec<&str> = term.split_whitespace().collect();
    if words.is_empty() || words.len() > 4 || term.len() < 2 || term.len() > 40 {
        return true;
    }
    if term.chars().any(|ch| ch.is_ascii_digit()) {
        return true;
    }
    const NOISE: &[&str] = &[
        "skill", "skills", "experience", "location", "years", "year", "required", "preferred", "primary", "secondary",
        "knowledge", "strong", "good", "hands", "with", "and", "the", "role", "candidate", "must", "have", "nice",
        "core", "key", "optional", "additional", "essential", "mandatory",
    ];
    words.iter().all(|word| NOISE.contains(word))
}

fn term_in(text: &str, skill: &str) -> bool {
    let folded = text
        .to_lowercase()
        .replace(|ch: char| matches!(ch, '/' | ',' | ';' | '|' | '\n' | '\r' | '.' | '(' | ')' | ':' | '[' | ']' | '•' | '-' | '–' | '—'), " ");
    let collapsed = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    format!(" {collapsed} ").contains(&format!(" {} ", skill.to_lowercase()))
}

fn split_sections(lower: &str, original: &str) -> (String, String) {
    let must_at = section_at(
        lower,
        &[
            "primary skills",
            "primary skill",
            "primary:",
            "core skills",
            "key skills",
            "must-have",
            "must have",
            "required skills",
            "mandatory skills",
            "essential skills",
        ],
    );
    let nice_at = section_at(
        lower,
        &[
            "secondary skills",
            "secondary skill",
            "secondary:",
            "nice-to-have",
            "nice to have",
            "preferably",
            "preferred",
            "good to have",
            "optional skills",
            "additional skills",
        ],
    );
    match (must_at, nice_at) {
        (Some(must), Some(nice)) if nice > must => (
            original[must..nice].to_string(),
            original[nice..].to_string(),
        ),
        (Some(must), Some(nice)) if must > nice => (
            original[must..].to_string(),
            original[nice..must].to_string(),
        ),
        (Some(must), None) => (original[must..].to_string(), String::new()),
        (None, Some(nice)) => (original[..nice].to_string(), original[nice..].to_string()),
        _ => (String::new(), String::new()),
    }
}

fn bounded_years<'a>(text: &'a str, pattern: &'a regex::Regex) -> Option<regex::Captures<'a>> {
    pattern.captures_iter(text).find(|caps| {
        let start = caps.get(0).map(|item| item.start()).unwrap_or(0);
        let previous = text[..start].chars().next_back();
        !previous.is_some_and(|ch| ch.is_ascii_digit() || ch == '.')
    })
}

fn experience_range(text: &str) -> (Option<i32>, Option<i32>) {
    let range = regex::Regex::new(r"(?i)(\d{1,2}(?:\.\d+)?)\s*(?:to|-|–)\s*(\d{1,2}(?:\.\d+)?)\s*(?:years|yrs)\b").unwrap();
    if let Some(caps) = bounded_years(text, &range) {
        return (crate::fields::round_years(&caps[1]), crate::fields::round_years(&caps[2]));
    }
    let plus = regex::Regex::new(r"(?i)(\d{1,2}(?:\.\d+)?)\s*\+\s*(?:years|yrs)\b").unwrap();
    if let Some(caps) = bounded_years(text, &plus) {
        return (crate::fields::round_years(&caps[1]), None);
    }
    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const JD: &str = "Backend engineer, Bengaluru\n\nMust-have: Java, Spring Boot, Kafka\nNice-to-have: JavaScript, Kubernetes\nExperience: 5-9 years\nLocation: Bengaluru\n";

    #[test]
    fn a_different_job_ranks_the_people_who_fit_that_job() {
        let java = parse_job("Java developer with Spring Boot");
        let idmc = parse_job("Informatica IDMC developer with PowerCenter");
        let aparna = "informatica, idmc, powercenter, etl, sql";
        let rahul = "java, spring boot, sql";
        let weights = Weights::default();
        let aparna_idmc = score_profile(aparna, Some(7), "Bengaluru", &idmc, &weights);
        let rahul_idmc = score_profile(rahul, Some(8), "Bengaluru", &idmc, &weights);
        let aparna_java = score_profile(aparna, Some(7), "Bengaluru", &java, &weights);
        assert!(aparna_idmc.score > rahul_idmc.score);
        assert!(aparna_idmc.matching.iter().any(|skill| skill == "idmc"));
        assert!(aparna_java.score < score_profile(rahul, Some(8), "Bengaluru", &java, &weights).score);
    }

    #[test]
    fn java_profile_scores_above_python() {
        let job = parse_job(JD);
        assert!(job.must_have.contains(&"java".to_string()));
        assert!(job.nice_to_have.contains(&"javascript".to_string()));
        assert_eq!(job.min_years, Some(5));
        assert_eq!(job.locations, vec!["Bengaluru".to_string()]);
        let strong = score_profile("Java, Spring Boot, Kafka, JS", Some(8), "Bengaluru", &job, &Weights::default());
        let weak = score_profile("Python, Django", Some(4), "Pune", &job, &Weights::default());
        assert!(strong.matching.iter().any(|skill| skill == "javascript"));
        assert!(strong.score > weak.score);
        assert!(weak.missing.iter().any(|skill| skill == "kafka"));
        assert!(!strong.rationale.is_empty());
    }

    #[test]
    fn a_job_without_skills_does_not_award_skill_points() {
        let empty = parse_job("Hiring now");
        let blank = score_profile("", None, "", &empty, &Weights::default());
        assert_eq!(blank.score, 0);
        assert!(blank.rationale.contains("did not list skills"));
        let city_only = parse_job("Need someone in Bengaluru soon");
        let elsewhere = score_profile("Python", None, "Pune", &city_only, &Weights::default());
        assert_eq!(elsewhere.score, 0);
        assert!(elsewhere.rationale.contains("skills were not scored"));
        assert!(elsewhere.rationale.contains("A city was not requested.") == false);
        assert!(elsewhere.rationale.contains("Experience was not requested."));
    }

    #[test]
    fn preferably_is_extra_and_unrequested_experience_is_not_a_fit() {
        let job = parse_job("Strong hands-on experience in Core Java.\nGood experience with Spring Boot, Spring MVC and REST APIs.\nExperience in microservices architecture.\nGood knowledge of SQL, preferably MySQL/Oracle.\n");
        assert!(job.must_have.contains(&"java".to_string()));
        assert!(job.must_have.contains(&"spring mvc".to_string()));
        assert!(job.must_have.contains(&"rest".to_string()));
        assert!(job.nice_to_have.contains(&"mysql".to_string()));
        assert!(job.nice_to_have.contains(&"oracle".to_string()));
        assert!(job.min_years.is_none());
        let full = score_profile("java, spring boot, spring mvc, rest, microservices, sql, mysql", Some(15), "Mumbai", &job, &Weights::default());
        let plain = score_profile("java, spring boot, spring mvc, rest, microservices, sql", Some(1), "Pune", &job, &Weights::default());
        assert!(full.score > plain.score);
        assert!(full.rationale.contains("Experience was not requested."));
        assert!(full.rationale.contains("A city was not requested."));
        let plus = parse_job("Overall 8+ years of Java");
        assert_eq!(plus.min_years, Some(8));
    }

    #[test]
    fn primary_skills_outrank_a_secondary_only_profile() {
        let job = parse_job("ETL developer\nPrimary skills: Informatica, IDMC, PowerCenter\nSecondary skills: SQL, Python, AWS\n");
        assert!(job.must_have.contains(&"informatica".to_string()));
        assert!(job.must_have.contains(&"idmc".to_string()));
        assert!(job.nice_to_have.contains(&"sql".to_string()));
        assert!(job.nice_to_have.contains(&"python".to_string()));
        assert!(!job.must_have.contains(&"sql".to_string()));
        let primary = score_profile("Informatica, IDMC, PowerCenter", Some(7), "Hyderabad", &job, &Weights::default());
        let secondary = score_profile("SQL, Python, AWS", Some(9), "Hyderabad", &job, &Weights::default());
        assert!(primary.score > secondary.score);
        assert!(primary.score >= 70);
        assert!(secondary.score < 40);
        assert!(primary.rationale.contains("3 of 3 primary"));
        assert!(primary.rationale.contains("0 of 3 secondary"));
    }

    #[test]
    fn a_listed_skill_outside_the_dictionary_still_counts() {
        let job = parse_job("Primary skills: Snowflake, dbt\nSecondary skills: Airflow\n");
        assert!(job.must_have.iter().any(|skill| skill == "snowflake"));
        assert!(job.must_have.iter().any(|skill| skill == "dbt"));
        assert!(job.nice_to_have.iter().any(|skill| skill == "airflow"));
        let fit = score_profile("Built Snowflake models with dbt for the warehouse team.", Some(6), "", &job, &Weights::default());
        let other = score_profile("Java and Spring Boot developer", Some(8), "", &job, &Weights::default());
        assert!(fit.matching.iter().any(|skill| skill == "snowflake"));
        assert!(fit.score > other.score);
    }
}
