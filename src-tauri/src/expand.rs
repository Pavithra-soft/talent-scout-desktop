use serde::Deserialize;

use crate::rank::JobNeeds;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPlan {
    pub primary: Vec<String>,
    pub secondary: Vec<String>,
    pub added_primary: Vec<String>,
    pub added_secondary: Vec<String>,
    pub note: String,
}

#[derive(Debug, Deserialize)]
struct Suggestion {
    #[serde(default)]
    primary: Vec<SkillIdea>,
    #[serde(default)]
    secondary: Vec<SkillIdea>,
}

#[derive(Debug, Deserialize)]
struct SkillIdea {
    #[serde(default)]
    skill: String,
    #[serde(default)]
    same: Vec<String>,
    #[serde(default)]
    with: Vec<String>,
}

pub fn merge(job: &mut JobNeeds, raw: &str) -> Result<SkillPlan, String> {
    let suggestion = parse_suggestion(raw)?;
    let original_primary = job.must_have.clone();
    let original_secondary = job.nice_to_have.clone();
    let mut added_primary = Vec::new();
    let mut added_secondary = Vec::new();
    apply_side(&mut job.must_have, &mut job.must_aliases, &suggestion.primary, &mut added_primary, &job.nice_to_have);
    let primary_names = job.must_have.clone();
    apply_side(&mut job.nice_to_have, &mut job.nice_aliases, &suggestion.secondary, &mut added_secondary, &primary_names);
    let note = search_note(&original_primary, &original_secondary, job, &added_primary, &added_secondary);
    Ok(SkillPlan {
        primary: display_groups(&job.must_have, &job.must_aliases),
        secondary: display_groups(&job.nice_to_have, &job.nice_aliases),
        added_primary,
        added_secondary,
        note,
    })
}

pub fn fetch_related(primary: &[String], secondary: &[String]) -> Result<String, String> {
    let key = gemini_key();
    if key.is_empty() {
        return Err("Add a Gemini key to bring related skills. The search used the skills in the job.".to_string());
    }
    let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string());
    let prompt = format!(
        "You help a recruiter expand skill names. Use only the skill names below. Do not invent people, employers, or cities.\n\
For each skill return same: other names a resume uses for that exact skill, maximum 3.\n\
Return with: at most one tool that is mandatory together with that skill. Use an empty list when there is none.\n\
Do not repeat a skill already listed. Do not add soft skills such as communication or teamwork.\n\
Reply JSON only: {{\"primary\":[{{\"skill\":\"...\",\"same\":[\"...\"],\"with\":[\"...\"]}}],\"secondary\":[...]}}\n\n\
Primary: {}\nSecondary: {}",
        primary.join(", "),
        secondary.join(", ")
    );
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model.trim(),
        key
    );
    let body = serde_json::json!({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {"temperature": 0.1}
    });
    let response = ureq::post(&url)
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(20))
        .send_string(&body.to_string())
        .map_err(|_| "Related skills could not be loaded. The search used the skills in the job.".to_string())?;
    let text = response
        .into_string()
        .map_err(|_| "Related skills could not be loaded. The search used the skills in the job.".to_string())?;
    reply_text(&text)
}

pub fn gemini_key() -> String {
    if let Ok(key) = std::env::var("GEMINI_API_KEY") {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(path) = key_file() {
        if let Ok(saved) = std::fs::read_to_string(path) {
            return saved.trim().to_string();
        }
    }
    String::new()
}

pub fn save_gemini_key(key: &str) -> Result<(), String> {
    let Some(path) = key_file() else {
        return Err("This computer could not store the Gemini key.".to_string());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "This computer could not store the Gemini key.".to_string())?;
    }
    let trimmed = key.trim();
    if trimmed.is_empty() {
        let _ = std::fs::remove_file(path);
        return Ok(());
    }
    std::fs::write(&path, trimmed).map_err(|_| "This computer could not store the Gemini key.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn key_file() -> Option<std::path::PathBuf> {
    crate::privacy::config_dir().map(|dir| dir.join("gemini.key"))
}

fn apply_side(names: &mut Vec<String>, aliases: &mut Vec<Vec<String>>, ideas: &[SkillIdea], added: &mut Vec<String>, other_side: &[String]) {
    let mut extras = 0;
    for idea in ideas {
        let skill = clean_skill(&idea.skill);
        let Some(index) = names.iter().position(|item| item == &skill) else {
            continue;
        };
        for alias in idea.same.iter().take(3) {
            let alias = clean_skill(alias);
            if !keep_term(&alias) || names.iter().any(|item| item == &alias) || other_side.iter().any(|item| item == &alias) {
                continue;
            }
            if let Some(group) = aliases.get_mut(index) {
                if !group.contains(&alias) && group.len() < 4 {
                    group.push(alias.clone());
                    push_unique(added, alias);
                }
            }
        }
        if extras >= 2 {
            continue;
        }
        if let Some(extra) = idea.with.first() {
            let extra = clean_skill(extra);
            if keep_term(&extra) && !names.iter().any(|item| item == &extra) && !other_side.iter().any(|item| item == &extra) {
                names.push(extra.clone());
                aliases.push(vec![extra.clone()]);
                push_unique(added, extra);
                extras += 1;
            }
        }
    }
}

fn parse_suggestion(raw: &str) -> Result<Suggestion, String> {
    let text = raw.trim();
    let text = text.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    serde_json::from_str(text).map_err(|_| "The related-skill reply could not be read. The search used the skills in the job.".to_string())
}

fn reply_text(raw: &str) -> Result<String, String> {
    let data: serde_json::Value = serde_json::from_str(raw).map_err(|_| "Related skills could not be loaded. The search used the skills in the job.".to_string())?;
    let parts = data
        .pointer("/candidates/0/content/parts")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "Related skills could not be loaded. The search used the skills in the job.".to_string())?;
    let text = parts.iter().filter_map(|part| part.get("text").and_then(|value| value.as_str())).collect::<String>();
    if text.trim().is_empty() {
        Err("Related skills could not be loaded. The search used the skills in the job.".to_string())
    } else {
        Ok(text)
    }
}

fn display_groups(names: &[String], aliases: &[Vec<String>]) -> Vec<String> {
    let mut shown = Vec::new();
    for (index, name) in names.iter().enumerate() {
        push_unique(&mut shown, name.clone());
        if let Some(group) = aliases.get(index) {
            for alias in group {
                if alias != name {
                    push_unique(&mut shown, alias.clone());
                }
            }
        }
    }
    shown
}

fn search_note(original_primary: &[String], original_secondary: &[String], job: &JobNeeds, added_primary: &[String], added_secondary: &[String]) -> String {
    if added_primary.is_empty() && added_secondary.is_empty() {
        return "No related skills were added. The search used the skills in the job.".to_string();
    }
    let mut lines = Vec::new();
    explain_side("primary", original_primary, &job.must_have, &job.must_aliases, added_primary, &mut lines);
    explain_side("secondary", original_secondary, &job.nice_to_have, &job.nice_aliases, added_secondary, &mut lines);
    lines.join(" ")
}

fn explain_side(label: &str, original: &[String], names: &[String], aliases: &[Vec<String>], added: &[String], lines: &mut Vec<String>) {
    for (index, name) in names.iter().enumerate() {
        if original.iter().any(|item| item == name) {
            let extras: Vec<&str> = aliases
                .get(index)
                .map(|group| group.iter().filter(|alias| *alias != name && added.iter().any(|item| item == *alias)).map(|alias| alias.as_str()).collect())
                .unwrap_or_default();
            for extra in extras {
                lines.push(format!("{extra} counts as {name}."));
            }
        } else if added.iter().any(|item| item == name) {
            lines.push(format!("{name} was added to {label}."));
        }
    }
}

fn push_unique(skills: &mut Vec<String>, skill: String) {
    if !skill.is_empty() && !skills.iter().any(|item| item == &skill) {
        skills.push(skill);
    }
}

fn clean_skill(raw: &str) -> String {
    raw.to_lowercase()
        .replace(|ch: char| matches!(ch, '/' | ',' | ';' | '|' | '.' | '(' | ')' | ':' ), " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn keep_term(term: &str) -> bool {
    if term.len() < 2 || term.len() > 40 || term.split_whitespace().count() > 4 {
        return false;
    }
    if term.chars().any(|ch| ch.is_ascii_digit()) {
        return false;
    }
    const NOISE: &[&str] = &[
        "communication", "teamwork", "leadership", "attitude", "ownership", "stakeholder", "interpersonal",
        "presentation", "problem solving", "soft skills", "management", "skill", "skills",
    ];
    !NOISE.contains(&term)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rank::{parse_job, score_profile, Weights};

    #[test]
    fn related_skills_join_the_right_list_and_a_synonym_satisfies_the_primary_skill() {
        let mut job = parse_job("ETL developer\nPrimary skills: IDMC\nSecondary skills: SQL\n");
        let plan = merge(
            &mut job,
            r#"{"primary":[{"skill":"idmc","same":["IICS","Informatica Data Management Cloud","communication"],"with":["etl"]}],"secondary":[{"skill":"sql","same":["plsql","teamwork"],"with":["idmc"]}]}"#,
        )
        .unwrap();
        assert!(plan.added_primary.iter().any(|skill| skill == "iics"));
        assert!(plan.added_primary.iter().any(|skill| skill == "etl"));
        assert!(plan.added_primary.iter().all(|skill| skill != "communication"));
        assert!(plan.added_secondary.iter().any(|skill| skill == "plsql"));
        assert!(plan.added_secondary.iter().all(|skill| skill != "idmc"));
        assert!(plan.primary.iter().any(|skill| skill == "iics"));
        assert!(plan.secondary.iter().any(|skill| skill == "plsql"));
        let weights = Weights::default();
        let synonym = score_profile("IICS and ETL developer", Some(7), "Hyderabad", &job, &weights);
        let exact = score_profile("IDMC developer", Some(7), "Hyderabad", &job, &weights);
        let secondary_only = score_profile("SQL and PL/SQL developer", Some(9), "Hyderabad", &job, &weights);
        assert!(synonym.matching.iter().any(|skill| skill.starts_with("idmc")));
        assert!(synonym.score > exact.score);
        assert!(exact.score > secondary_only.score);
        assert!(secondary_only.score < 40);
        assert!(plan.note.contains("iics counts as idmc"));
        assert!(plan.note.contains("etl was added to primary"));
        assert!(plan.note.contains("plsql counts as sql"));
    }

    #[test]
    fn a_bad_reply_does_not_change_the_job() {
        let mut job = parse_job("Primary skills: Java, Spring Boot\nSecondary skills: Kafka\n");
        let before = job.must_have.clone();
        let error = merge(&mut job, "not json").unwrap_err();
        assert!(error.contains("could not be read"));
        assert_eq!(job.must_have, before);
    }

    #[test]
    fn the_reply_text_is_taken_from_the_model_payload() {
        let raw = r#"{"candidates":[{"content":{"parts":[{"text":"{\"primary\":[]}"}]}}]}"#;
        let text = reply_text(raw).unwrap();
        assert!(text.contains("primary"));
    }
}
