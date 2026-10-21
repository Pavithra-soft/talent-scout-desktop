use serde::Serialize;

use crate::fields::find_skills;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Project {
    pub title: String,
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Dossier {
    pub qualification: Vec<String>,
    pub projects: Vec<Project>,
    pub lines: Vec<String>,
}

pub fn from_text(text: &str) -> Dossier {
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect();
    let qualification = qualification_lines(&lines);
    let projects = project_blocks(&lines);
    let lines = profile_lines(&lines);
    Dossier { qualification, projects, lines }
}

fn profile_lines(lines: &[String]) -> Vec<String> {
    let mut kept = Vec::new();
    let mut chars = 0usize;
    for line in lines {
        let clean = clean_line(line);
        if clean.is_empty() {
            continue;
        }
        if kept.len() == 220 || chars + clean.len() > 12_000 {
            kept.push("The rest of the resume is in the downloaded file.".to_string());
            break;
        }
        chars += clean.len();
        kept.push(clean);
    }
    kept
}

fn qualification_lines(lines: &[String]) -> Vec<String> {
    let mut from_section = Vec::new();
    if let Some(start) = lines.iter().position(|line| is_education_heading(line)) {
        for line in lines.iter().skip(start + 1) {
            if is_section_heading(line) || looks_like_job(line) || is_project_title(line) || is_short_heading(line) {
                break;
            }
            if looks_like_degree(line) || line.len() > 12 {
                from_section.push(clean_line(line));
            }
            if from_section.len() == 6 {
                break;
            }
        }
    }
    if !from_section.is_empty() {
        return from_section;
    }
    lines
        .iter()
        .filter(|line| looks_like_degree(line))
        .take(6)
        .map(|line| clean_line(line))
        .collect()
}

fn project_blocks(lines: &[String]) -> Vec<Project> {
    let titles: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| is_project_title(line))
        .map(|(index, _)| index)
        .take(12)
        .collect();
    let mut projects = Vec::new();
    for (nth, start) in titles.iter().enumerate() {
        let next_title = titles.get(nth + 1).copied().unwrap_or(lines.len());
        let next_heading = lines[start + 1..next_title]
            .iter()
            .position(|line| is_section_heading(line) || is_education_heading(line))
            .map(|offset| start + 1 + offset)
            .unwrap_or(next_title);
        let end = next_heading.min(next_title);
        let title = clean_line(&lines[*start]);
        let body = lines[start + 1..end].join(" ");
        let mut skills = find_skills(&format!("{title} {body}"));
        skills.truncate(8);
        projects.push(Project { title, skills });
    }
    projects
}

fn heading_key(line: &str) -> String {
    line.trim().trim_end_matches(':').trim().to_lowercase()
}

fn is_education_heading(line: &str) -> bool {
    let lower = heading_key(line);
    matches!(
        lower.as_str(),
        "education" | "academics" | "academic" | "qualification" | "qualifications" | "academic qualification" | "academic qualifications"
    ) || lower.starts_with("education ")
        || lower.starts_with("academic ")
}

fn is_section_heading(line: &str) -> bool {
    let lower = heading_key(line);
    matches!(
        lower.as_str(),
        "skills" | "technical skills" | "experience" | "work experience" | "summary" | "professional summary" | "certifications" | "projects" | "professional organizations"
    )
}

fn is_short_heading(line: &str) -> bool {
    let words = line.split_whitespace().count();
    words <= 4
        && !looks_like_degree(line)
        && !line.to_lowercase().contains("university")
        && !line.to_lowercase().contains("college")
        && !line.to_lowercase().contains("project")
}

fn looks_like_degree(line: &str) -> bool {
    let lower = format!(" {} ", line.to_lowercase());
    [
        " bachelor ",
        " master ",
        " b.tech ",
        " b.e ",
        " b.e. ",
        " m.tech ",
        " mba ",
        " bsc ",
        " msc ",
        " bca ",
        " mca ",
        " phd ",
        " diploma ",
        " degree ",
    ]
    .iter()
    .any(|term| lower.contains(term))
}

fn looks_like_job(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("present") || year_span(line)
}

fn is_project_title(line: &str) -> bool {
    let lower = line.to_lowercase();
    if is_education_heading(line) || is_section_heading(line) || looks_like_degree(line) {
        return false;
    }
    if lower.starts_with("this ") || lower.starts_with("the ") {
        return false;
    }
    let dated = year_span(line) && (lower.contains(" - ") || lower.contains(" – ") || lower.contains("—"));
    let named = lower.contains("project") && (year_span(line) || lower.contains(" - ") || lower.contains(" – "));
    dated || named
}

fn year_span(line: &str) -> bool {
    let years: Vec<_> = regex_years().find_iter(line).collect();
    years.len() >= 2 || (years.len() == 1 && line.to_lowercase().contains("present"))
}

fn regex_years() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| regex::Regex::new(r"\b(?:19|20)\d{2}\b").unwrap())
}

fn clean_line(line: &str) -> String {
    line.trim().trim_matches(|ch: char| ch == '•' || ch == '-' || ch == '–').trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aparna_style_resume_splits_qualification_and_project_skills() {
        let text = "\
Aparna Kalamkuntla
Analytics projects
Education
Bachelor of Technology (ECE) from Sri Padmavathi Mahila Visvavidyalayam, Tirupati 2017.
Professional Organizations
Capgemini - Liberty Mutual Insurance Project Oct 2023 – July 2026 ETL developer
Migrating PowerCenter workflows to Informatica IDMC.
Environment: Informatica power center, IDMC, SQL
Capgemini - Honeywell internal project June 2023 - Oct 2023
Designed ETL solutions using Informatica PowerCenter.
DXC Technology - CRM Project Feb 2022 - May 2023
Skills: SQL, Oracle
Genpact - MOS (Material object suite) Aug 2018 - July 2021
Talend and PostgreSQL developer
Technical Skills:
Java, Spring Boot, and Kafka for a different section.
";
        let dossier = from_text(text);
        assert_eq!(dossier.qualification.len(), 1);
        assert!(dossier.qualification[0].contains("Bachelor of Technology"));
        assert!(dossier.projects.len() >= 2);
        assert!(dossier.projects[0].title.contains("Liberty Mutual"));
        assert!(dossier.projects[0].skills.iter().any(|skill| skill == "idmc" || skill == "informatica" || skill == "powercenter"));
        assert!(dossier.projects.iter().any(|project| project.skills.iter().any(|skill| skill == "sql")));
        assert!(dossier.projects.iter().any(|project| project.title.contains("Genpact") && project.skills.iter().any(|skill| skill == "talend")));
        assert!(dossier.projects.iter().all(|project| !project.skills.iter().any(|skill| skill == "java" || skill == "kafka")));
        assert!(dossier.lines.iter().any(|line| line.contains("Aparna Kalamkuntla")));
        assert!(dossier.lines.iter().any(|line| line.contains("Java, Spring Boot")));
    }

    #[test]
    fn a_long_resume_stops_before_the_profile_view_grows_without_limit() {
        let text = (0..400)
            .map(|index| format!("Line {index} of the resume with enough words to count toward the limit."))
            .collect::<Vec<_>>()
            .join("\n");
        let dossier = from_text(&text);
        assert!(dossier.lines.len() <= 221);
        assert!(dossier.lines.last().unwrap().contains("downloaded file"));
    }

    #[test]
    fn a_resume_without_projects_stays_empty_there() {
        let dossier = from_text("Rahul Kumar\nEducation\nMaster of Computer Applications, Pune University, 2016\nSkills: Java, Spring Boot\n");
        assert!(dossier.qualification[0].contains("Master of Computer Applications"));
        assert!(dossier.projects.is_empty());
    }
}
