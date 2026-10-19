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
    pub nice_to_have: Vec<String>,
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
        find_skills(text)
    } else {
        find_skills(&must_text)
    };
    let mut nice_to_have = find_skills(&nice_text);
    nice_to_have.retain(|skill| !must_have.contains(skill));
    let (min_years, max_years) = experience_range(text);
    let locations = crate::fields::find_location(text);
    let locations = if locations.is_empty() {
        Vec::new()
    } else {
        vec![locations]
    };
    JobNeeds {
        must_have,
        nice_to_have,
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
    let matched_must: Vec<String> = job
        .must_have
        .iter()
        .filter(|skill| profile_skills.iter().any(|item| item == *skill))
        .cloned()
        .collect();
    let matched_nice: Vec<String> = job
        .nice_to_have
        .iter()
        .filter(|skill| profile_skills.iter().any(|item| item == *skill))
        .cloned()
        .collect();
    let mut missing = Vec::new();
    for skill in job.must_have.iter().chain(job.nice_to_have.iter()) {
        if !matched_must.contains(skill) && !matched_nice.contains(skill) {
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
    let experience_ratio = experience_fit(years, job.min_years, job.max_years);
    let location_ratio = location_fit(location, &job.locations);
    let weight_must = if job.must_have.is_empty() { 0 } else { weights.must };
    let weight_nice = if job.nice_to_have.is_empty() { 0 } else { weights.nice };
    let weight_experience = if listed_experience { weights.experience } else { 0 };
    let weight_location = if listed_location { weights.location } else { 0 };
    let total = (weight_must + weight_nice + weight_experience + weight_location).max(1) as f32;
    let points = weight_must as f32 * must_ratio
        + weight_nice as f32 * nice_ratio
        + weight_experience as f32 * experience_ratio
        + weight_location as f32 * location_ratio;
    let score = ((100.0 * points / total).round() as i32).clamp(0, 100);
    let mut matching = matched_must.clone();
    matching.extend(matched_nice.iter().cloned());
    let rationale = if !listed_skills {
        format!(
            "This job did not list skills, so skills were not scored. {} {}",
            experience_line(experience_ratio, years),
            location_line(location_ratio, location)
        )
    } else {
        format!(
            "Matches {} of {} required skills. {} {}",
            matched_must.len(),
            job.must_have.len(),
            experience_line(experience_ratio, years),
            location_line(location_ratio, location)
        )
    };
    Score {
        score,
        matching,
        missing,
        rationale,
    }
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

fn split_sections(lower: &str, original: &str) -> (String, String) {
    let must_at = ["must-have", "must have", "required"]
        .iter()
        .find_map(|label| lower.find(label));
    let nice_at = ["nice-to-have", "nice to have", "preferred"]
        .iter()
        .find_map(|label| lower.find(label));
    match (must_at, nice_at) {
        (Some(must), Some(nice)) if nice > must => (
            original[must..nice].to_string(),
            original[nice..].to_string(),
        ),
        (Some(must), None) => (original[must..].to_string(), String::new()),
        _ => (String::new(), String::new()),
    }
}

fn experience_range(text: &str) -> (Option<i32>, Option<i32>) {
    let pattern = regex::Regex::new(r"(?i)(\d{1,2})\s*(?:to|-)\s*(\d{1,2})\s*(?:years|yrs)").unwrap();
    if let Some(caps) = pattern.captures(text) {
        return (caps[1].parse().ok(), caps[2].parse().ok());
    }
    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const JD: &str = "Backend engineer, Bengaluru\n\nMust-have: Java, Spring Boot, Kafka\nNice-to-have: JavaScript, Kubernetes\nExperience: 5-9 years\nLocation: Bengaluru\n";

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
    }
}
