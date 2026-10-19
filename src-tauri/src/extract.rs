use std::fs;
use std::io::Read;
use std::path::Path;

use crate::fields::{profile_from_text, Profile};

pub fn profile_from_path(path: &Path) -> Result<Profile, String> {
    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    let name = path
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("resume");
    profile_from_bytes(name, &bytes)
}

pub fn profile_from_bytes(name: &str, bytes: &[u8]) -> Result<Profile, String> {
    if bytes.len() > 15 * 1024 * 1024 {
        return Err("This file is too large to read. Split it or send a smaller copy.".to_string());
    }
    let text = text_from_bytes(name, bytes)?;
    if text.trim().is_empty() {
        return Err("This file is empty. Remove it or add a resume with text.".to_string());
    }
    let profile = profile_from_text(&text);
    if name.to_lowercase().ends_with(".doc") && profile.name.is_empty() && profile.email.is_empty() {
        return Err("This older Word file could not be read. Save it as .docx and add it again.".to_string());
    }
    Ok(profile)
}

fn text_from_bytes(name: &str, bytes: &[u8]) -> Result<String, String> {
    let lower = name.to_lowercase();
    if lower.ends_with(".txt") {
        return Ok(String::from_utf8_lossy(bytes).to_string());
    }
    if lower.ends_with(".rtf") {
        return Ok(strip_rtf(&String::from_utf8_lossy(bytes)));
    }
    if lower.ends_with(".docx") {
        return docx_text(bytes);
    }
    if lower.ends_with(".pdf") {
        return pdf_extract::extract_text_from_mem(bytes)
            .map_err(|_| "This PDF could not be read. Ask for an unlocked copy.".to_string());
    }
    if lower.ends_with(".doc") {
        return Ok(doc_text(bytes));
    }
    if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        return Err("This is a photo, so it was skipped. The other resumes will still be read.".to_string());
    }
    Err("This file type cannot be read. Use PDF, Word, or text.".to_string())
}

fn docx_text(bytes: &[u8]) -> Result<String, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|err| err.to_string())?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| "This Word file has no readable text.".to_string())?
        .read_to_string(&mut xml)
        .map_err(|err| err.to_string())?;
    Ok(strip_xml(&xml))
}

fn strip_xml(xml: &str) -> String {
    let with_breaks = xml.replace("</w:p>", "\n");
    let mut out = String::new();
    let mut inside = false;
    for ch in with_breaks.chars() {
        match ch {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(ch),
            _ => {}
        }
    }
    out
}

fn strip_rtf(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let mut word = String::new();
            while let Some(next) = chars.peek() {
                if next.is_ascii_alphabetic() {
                    word.push(*next);
                    chars.next();
                } else {
                    break;
                }
            }
            if word == "u" {
                let mut number = String::new();
                while let Some(digit) = chars.peek() {
                    if digit.is_ascii_digit() || *digit == '-' {
                        number.push(*digit);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Ok(code) = number.parse::<u32>() {
                    if let Some(decoded) = char::from_u32(code) {
                        out.push(decoded);
                    }
                }
            } else if word == "par" || word == "line" {
                out.push('\n');
            }
            if matches!(chars.peek(), Some(' ')) {
                chars.next();
            }
            continue;
        }
        if ch != '{' && ch != '}' {
            out.push(ch);
        }
    }
    out
}

fn doc_text(bytes: &[u8]) -> String {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        let code = u16::from_le_bytes([bytes[index], bytes[index + 1]]);
        if (32..127).contains(&code) {
            current.push(char::from(code as u8));
            index += 2;
            continue;
        }
        if current.split_whitespace().count() >= 2 {
            words.push(current.clone());
        }
        current.clear();
        index += 2;
    }
    if current.split_whitespace().count() >= 2 {
        words.push(current);
    }
    words.join("\n")
}

pub fn allowed_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    [".pdf", ".docx", ".doc", ".rtf", ".txt", ".png", ".jpg", ".jpeg", ".zip"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn reads_a_docx_resume() {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            writer
                .start_file("word/document.xml", SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(
                    br#"<w:document><w:p><w:r><w:t>Ananya Rao</w:t></w:r></w:p><w:p><w:r><w:t>ananya.rao@example.com Bengaluru java</w:t></w:r></w:p></w:document>"#,
                )
                .unwrap();
            writer.finish().unwrap();
        }
        let profile = profile_from_bytes("ananya.docx", cursor.get_ref()).unwrap();
        assert_eq!(profile.name, "Ananya Rao");
        assert_eq!(profile.email, "ananya.rao@example.com");
        assert_eq!(profile.location, "Bengaluru");
    }

    #[test]
    fn rtf_keeps_email_and_company_on_their_own_lines() {
        let rtf = "{\\rtf1\\ansi Ananya Rao\\par Email: ananya.rao@example.com\\par Company: Infosys\\par }";
        let profile = profile_from_bytes("ananya.rtf", rtf.as_bytes()).unwrap();
        assert_eq!(profile.email, "ananya.rao@example.com");
        assert_eq!(profile.company, "Infosys");
        assert_eq!(profile.name, "Ananya Rao");
    }

    #[test]
    fn locked_pdf_and_huge_file_use_plain_sentences() {
        let locked = profile_from_bytes("locked.pdf", b"%PDF-1.4\n1 0 obj<< /Encrypt 2 0 R >>endobj\n%%EOF");
        assert_eq!(
            locked.unwrap_err(),
            "This PDF could not be read. Ask for an unlocked copy."
        );
        let huge = vec![b'a'; 15 * 1024 * 1024 + 1];
        let too_big = profile_from_bytes("huge.txt", &huge);
        assert_eq!(
            too_big.unwrap_err(),
            "This file is too large to read. Split it or send a smaller copy."
        );
    }
}
