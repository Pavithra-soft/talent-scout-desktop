use std::path::Path;

use rust_xlsxwriter::{Color, Format, Workbook};

use crate::db::Stored;

pub fn write_xlsx(path: &Path, rows: &[Stored]) -> Result<(), String> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Ranked profiles").map_err(|err| err.to_string())?;
    let header = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1F4B3A));
    let titles = [
        "Rank",
        "Name",
        "Email",
        "Phone",
        "Location",
        "Skills",
        "Matching skills",
        "Experience",
        "Company",
        "Notice",
        "Match score",
        "Note",
    ];
    for (col, title) in titles.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, *title, &header)
            .map_err(|err| err.to_string())?;
        sheet.set_column_width(col as u16, 22).ok();
    }
    for (index, row) in rows.iter().enumerate() {
        let line = (index + 1) as u32;
        let values = [
            (index + 1).to_string(),
            row.name.clone(),
            row.email.clone(),
            row.phone.clone(),
            row.location.clone(),
            row.skills.clone(),
            row.matching.clone(),
            match row.years {
                Some(1) => "1 year".to_string(),
                Some(years) => format!("{years} years"),
                None => String::new(),
            },
            row.company.clone(),
            row.notice_raw.clone(),
            row.score.map(|score| score.to_string()).unwrap_or_default(),
            row.note.clone(),
        ];
        for (col, value) in values.iter().enumerate() {
            sheet
                .write_string(line, col as u16, value)
                .map_err(|err| err.to_string())?;
        }
    }
    let last = rows.len() as u32;
    sheet.autofilter(0, 0, last, 11).map_err(|err| err.to_string())?;
    sheet.set_freeze_panes(1, 0).map_err(|err| err.to_string())?;
    workbook.save(path).map_err(|err| err.to_string())?;
    Ok(())
}
