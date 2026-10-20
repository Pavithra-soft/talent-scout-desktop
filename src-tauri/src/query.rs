use serde::Serialize;

use crate::rank::parse_job;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PortalQuery {
    pub keywords: String,
    pub boolean_query: String,
    pub experience: String,
    pub location: String,
}

pub fn portal_query(jd_text: &str) -> Result<PortalQuery, String> {
    let trimmed = jd_text.trim();
    if trimmed.is_empty() {
        return Err("Paste the job description first.".to_string());
    }
    let job = parse_job(trimmed);
    if job.must_have.is_empty() && job.nice_to_have.is_empty() {
        return Err("This job did not list skills the search can use. Add skills such as Java or SQL, then try again.".to_string());
    }
    let must: Vec<String> = job.must_have.iter().map(|skill| display_skill(skill)).collect();
    let nice: Vec<String> = job.nice_to_have.iter().map(|skill| display_skill(skill)).collect();
    let mut keywords = must.clone();
    keywords.extend(nice.iter().cloned());
    let mut boolean = must.iter().map(|skill| quote_term(skill)).collect::<Vec<_>>().join(" AND ");
    if !nice.is_empty() {
        let extras = nice.iter().map(|skill| quote_term(skill)).collect::<Vec<_>>().join(" OR ");
        boolean = if boolean.is_empty() {
            format!("({extras})")
        } else {
            format!("{boolean} AND ({extras})")
        };
    }
    let experience = match (job.min_years, job.max_years) {
        (Some(min), Some(max)) => format!("{min} to {max} years"),
        (Some(min), None) => format!("{min}+ years"),
        (None, Some(max)) => format!("Up to {max} years"),
        (None, None) => "Any experience".to_string(),
    };
    let location = if job.locations.is_empty() {
        "Any city".to_string()
    } else {
        job.locations.join(", ")
    };
    Ok(PortalQuery {
        keywords: keywords.join(", "),
        boolean_query: boolean,
        experience,
        location,
    })
}

fn display_skill(skill: &str) -> String {
    match skill {
        "sql" => "SQL".to_string(),
        "mysql" => "MySQL".to_string(),
        "postgresql" => "PostgreSQL".to_string(),
        "mongodb" => "MongoDB".to_string(),
        "javascript" => "JavaScript".to_string(),
        "typescript" => "TypeScript".to_string(),
        "rest" => "REST".to_string(),
        "aws" => "AWS".to_string(),
        "html" => "HTML".to_string(),
        "css" => "CSS".to_string(),
        "j2ee" => "J2EE".to_string(),
        "jsf" => "JSF".to_string(),
        other => other
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().to_string() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn quote_term(skill: &str) -> String {
    if skill.contains(' ') {
        format!("\"{skill}\"")
    } else {
        skill.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_job_becomes_a_portal_boolean() {
        let query = portal_query(
            "Strong hands-on experience in Core Java.\nGood experience with Spring Boot, Spring MVC and REST APIs.\nExperience in microservices architecture.\nGood knowledge of SQL, preferably MySQL/Oracle.\nOverall 8+ years.\n",
        )
        .unwrap();
        assert!(query.boolean_query.contains("Java"));
        assert!(query.boolean_query.contains("\"Spring Boot\""));
        assert!(query.boolean_query.contains("SQL"));
        assert!(query.boolean_query.contains("MySQL OR Oracle") || query.boolean_query.contains("Oracle OR MySQL"));
        assert_eq!(query.experience, "8+ years");
        assert!(!query.keywords.is_empty());
    }
}
