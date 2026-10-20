use crate::fields::find_skills;
use crate::rank::{self, JobNeeds};

#[derive(Clone, Debug)]
pub struct Person {
    pub name: String,
    pub skills: String,
    pub matching: String,
    pub years: Option<i32>,
    pub location: String,
    pub company: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Letter {
    pub subject: String,
    pub body: String,
}

pub fn write_letter(jd: &str, instructions: &str, subject: &str, person: &Person) -> Letter {
    let job = rank::parse_job(jd);
    let role = role_name(jd, &job);
    let skills = overlap(person, &job);
    let first = first_name(&person.name);
    let subject = chosen_subject(subject, &role);
    let mut paragraphs = vec![greeting(&first), opening(&first, &role, &skills, person.years)];
    if let Some(line) = company_line(&person.company) {
        paragraphs.push(line);
    }
    if let Some(line) = fit_line(&job, person) {
        paragraphs.push(line);
    }
    if let Some(line) = instruction_line(instructions) {
        paragraphs.push(line);
    }
    paragraphs.push("If this sounds relevant, I would like a short call to share the role and hear what you are looking for. Would you be open to that?".to_string());
    paragraphs.push("Thank you".to_string());
    Letter {
        subject,
        body: paragraphs.join("\n\n"),
    }
}

fn greeting(first: &str) -> String {
    if first.is_empty() {
        "Hello,".to_string()
    } else {
        format!("Hello {first},")
    }
}

fn opening(_first: &str, role: &str, skills: &[String], years: Option<i32>) -> String {
    let role_bit = if role.starts_with("a ") {
        format!("I am hiring for {role}")
    } else {
        format!("I am hiring for a {role} role")
    };
    let skill_bit = match skills {
        [] => "and your profile stood out.".to_string(),
        [one] => format!("and your {one} experience stood out."),
        [one, two] => format!("and your experience with {one} and {two} stood out."),
        many => {
            let last = many.last().unwrap();
            let rest = many[..many.len() - 1].join(", ");
            format!("and your experience with {rest}, and {last} stood out.")
        }
    };
    let years_bit = match years {
        Some(1) => " You have about 1 year of experience.".to_string(),
        Some(years) => format!(" You have about {years} years of experience."),
        None => String::new(),
    };
    format!("{role_bit}, {skill_bit}{years_bit}")
}

fn company_line(company: &str) -> Option<String> {
    let company = company.trim();
    if company.is_empty() {
        return None;
    }
    Some(format!("Your work at {company} is relevant to this search."))
}

fn fit_line(job: &JobNeeds, person: &Person) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(min) = job.min_years {
        if person.years.is_some_and(|years| years >= min) {
            lines.push(format!("This role looks for {min}+ years, and your experience fits that."));
        }
    }
    if let Some(city) = job.locations.first() {
        if same_place(&person.location, city) {
            lines.push(format!("You are based in {city}, and this role is there as well."));
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" "))
    }
}

fn instruction_line(note: &str) -> Option<String> {
    let note = note.trim();
    if note.is_empty() {
        return None;
    }
    let lower = note.to_lowercase();
    if lower.contains("keep it short") || lower.contains("be brief") || lower.contains("short mail") || lower.contains("don't mention") || lower.contains("do not mention") {
        return None;
    }
    let prefixes = [
        "please mention that ",
        "please mention ",
        "mention that ",
        "mention ",
        "please include that ",
        "please include ",
        "include that ",
        "include ",
        "please say that ",
        "say that ",
        "please note that ",
        "note that ",
        "please add that ",
        "add that ",
        "tell them that ",
        "tell them ",
        "please tell them ",
    ];
    if let Some(rest) = strip_prefix(&lower, note, &prefixes) {
        return Some(sentence(&rest));
    }
    if let Some(rest) = strip_prefix(&lower, note, &["please ask them to ", "ask them to ", "please ask for ", "ask for ", "please ask about ", "ask about ", "please ask ", "ask "]) {
        return Some(format!("Could you share {}?", rest.trim().trim_end_matches(['.', '?'])));
    }
    Some(sentence(note))
}

fn strip_prefix<'a>(lower: &str, original: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    for prefix in prefixes {
        if lower.starts_with(prefix) {
            return Some(original[prefix.len()..].trim());
        }
    }
    None
}

fn sentence(value: &str) -> String {
    let value = value.trim().trim_end_matches('.').trim();
    if value.is_empty() {
        return String::new();
    }
    let mut chars = value.chars();
    let first = chars.next().unwrap().to_uppercase().to_string();
    format!("{first}{}.", chars.as_str())
}

fn chosen_subject(subject: &str, role: &str) -> String {
    let subject = subject.trim();
    if subject.is_empty() || subject.eq_ignore_ascii_case("your profile") {
        if role.starts_with("a ") {
            "A role that matches your experience".to_string()
        } else {
            format!("{role} opportunity")
        }
    } else {
        subject.replace(['\r', '\n'], " ")
    }
}

fn role_name(jd: &str, job: &JobNeeds) -> String {
    let first = jd.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or("");
    let title = first.split([',', '.', ':', '|']).next().unwrap_or("").trim();
    let lower = title.to_lowercase();
    let skip = lower.starts_with("must")
        || lower.starts_with("job description")
        || lower.starts_with("we ")
        || lower.starts_with("experience")
        || lower.starts_with("location")
        || lower.starts_with("skills")
        || lower.starts_with("nice");
    if (4..=48).contains(&title.len()) && !skip {
        return title.to_string();
    }
    let named: Vec<String> = job.must_have.iter().take(2).map(|skill| label_skill(skill)).collect();
    if named.is_empty() {
        "a role I am hiring for".to_string()
    } else if named.len() == 1 {
        named[0].clone()
    } else {
        format!("{} and {}", named[0], named[1])
    }
}

fn overlap(person: &Person, job: &JobNeeds) -> Vec<String> {
    let listed: Vec<String> = person
        .matching
        .split(',')
        .map(str::trim)
        .filter(|skill| !skill.is_empty())
        .map(label_skill)
        .take(3)
        .collect();
    if !listed.is_empty() {
        return listed;
    }
    let profile = find_skills(&person.skills);
    job.must_have
        .iter()
        .filter(|skill| profile.iter().any(|item| item == *skill))
        .take(3)
        .map(|skill| label_skill(skill))
        .collect()
}

fn first_name(name: &str) -> String {
    let word = name.split_whitespace().next().unwrap_or("").trim_matches(|ch: char| !ch.is_alphanumeric());
    if word.is_empty() {
        return String::new();
    }
    let mut chars = word.chars();
    let first = chars.next().unwrap().to_uppercase().to_string();
    format!("{first}{}", chars.as_str().to_lowercase())
}

fn same_place(profile: &str, city: &str) -> bool {
    let profile = profile.trim().to_lowercase();
    let city = city.trim().to_lowercase();
    !profile.is_empty() && (profile == city || profile.contains(&city) || city.contains(&profile))
}

fn label_skill(skill: &str) -> String {
    match skill.trim().to_lowercase().as_str() {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ananya() -> Person {
        Person {
            name: "Ananya Rao".into(),
            skills: "java, spring boot, sql".into(),
            matching: "spring boot, java".into(),
            years: Some(8),
            location: "Bengaluru".into(),
            company: "Infosys".into(),
        }
    }

    #[test]
    fn first_mail_uses_the_job_the_profile_and_the_recruiter_note() {
        let letter = write_letter(
            "Backend engineer, Bengaluru\nMust-have: Java, Spring Boot\nExperience: 5-9 years\nLocation: Bengaluru\n",
            "mention that the role is hybrid",
            "Your profile",
            &ananya(),
        );
        assert_eq!(letter.subject, "Backend engineer opportunity");
        assert!(letter.body.contains("Hello Ananya,"));
        assert!(letter.body.contains("Spring Boot"));
        assert!(letter.body.contains("8 years"));
        assert!(letter.body.contains("Infosys"));
        assert!(letter.body.contains("Bengaluru"));
        assert!(letter.body.contains("5+ years"));
        assert!(letter.body.contains("The role is hybrid."));
        assert!(letter.body.contains("Would you be open to that?"));
        assert!(!letter.body.to_lowercase().contains("status: sent"));
    }

    #[test]
    fn two_people_do_not_get_the_same_letter() {
        let jd = "Java developer\nMust-have: Java, Kafka\n";
        let java = write_letter(jd, "", "", &ananya());
        let mut meera = ananya();
        meera.name = "Meera Iyer".into();
        meera.matching = "kafka".into();
        meera.company = String::new();
        meera.years = None;
        meera.location = "Pune".into();
        let other = write_letter(jd, "ask about notice period", "Hello", &meera);
        assert!(java.body.contains("Ananya"));
        assert!(other.body.contains("Hello Meera,"));
        assert!(other.body.contains("Kafka"));
        assert!(!other.body.contains("Infosys"));
        assert!(other.body.contains("notice period"));
        assert_eq!(other.subject, "Hello");
    }
}
