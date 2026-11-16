//! Flat CSV tables from WHOOP and Oura exports.
//!
//! Both vendors write daily tables with readable headers whose wording changes
//! between export versions. A column is found by its normalized name, so
//! "Max HR (bpm)" and "max_hr_bpm" are the same column. A column that is not
//! named is never guessed at.
//!
//! This is the reader that ran in 2.1.0 (`crates/ath-core/src/whoop.rs`):
//!
//! - Surrounding whitespace is trimmed from the text, then it is split into
//!   lines. A line ending in CR LF loses the CR.
//! - The header decides the delimiter: `;` when it has more semicolons than
//!   commas, otherwise `,`.
//! - Quotes group a cell within one line. A quoted cell cannot span lines.
//! - Blank lines are skipped. When two headers normalize to one name, the
//!   later column wins.

use std::collections::BTreeMap;

/// One data row, keyed by normalized header.
pub type Row = BTreeMap<String, String>;

/// Read a CSV text into rows.
pub fn parse_csv(text: &str) -> Vec<Row> {
    let mut lines = text.trim().lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let delimiter = if header.matches(';').count() > header.matches(',').count() { ';' } else { ',' };
    let headers: Vec<String> = split_line(header, delimiter).iter().map(|name| normalize_header(name)).collect();
    lines
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let cells = split_line(line, delimiter);
            let mut row = Row::new();
            for (index, header) in headers.iter().enumerate() {
                if let Some(value) = cells.get(index) {
                    row.insert(header.clone(), value.clone());
                }
            }
            if row.is_empty() {
                None
            } else {
                Some(row)
            }
        })
        .collect()
}

/// Split one line into trimmed cells.
pub fn split_line(line: &str, delimiter: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '"' {
            if quoted && chars.peek() == Some(&'"') {
                current.push('"');
                chars.next();
            } else if quoted {
                quoted = false;
            } else if current.is_empty() {
                quoted = true;
            } else {
                current.push('"');
            }
        } else if character == delimiter && !quoted {
            out.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(character);
        }
    }
    out.push(current.trim().to_string());
    out
}

/// Lowercase the header, and turn each run of other characters into one `_`.
/// `(`, `)`, and `%` count as separators. Leading and trailing `_` are removed.
pub fn normalize_header(header: &str) -> String {
    let lower = header.to_lowercase().replace(['(', ')', '%'], " ");
    let mut out = String::new();
    let mut underscore = false;
    for character in lower.chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character);
            underscore = false;
        } else if !underscore && !out.is_empty() {
            out.push('_');
            underscore = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// The first non-blank value among candidate column names, trimmed.
pub fn cell(row: &Row, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(value) = row.get(&normalize_header(name)) {
            if !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

/// A finite number from a cell. Commas are removed first, so `1,234` is 1234.
/// The text is read as Rust reads a float, which accepts `1e3` and a leading `+`.
pub fn number(row: &Row, names: &[&str]) -> Option<f64> {
    let raw = cell(row, names)?;
    raw.replace(',', "").parse::<f64>().ok().filter(|value| value.is_finite())
}

/// A number above zero, for measurements where zero means "not recorded".
pub fn positive(row: &Row, names: &[&str]) -> Option<f64> {
    number(row, names).filter(|value| *value > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_normalize_to_one_spelling() {
        assert_eq!(normalize_header("Max HR (bpm)"), "max_hr_bpm");
        assert_eq!(normalize_header("Recovery score %"), "recovery_score");
        assert_eq!(normalize_header("\u{feff}Cycle start time"), "cycle_start_time");
        assert_eq!(normalize_header("Deep (SWS) duration (min)"), "deep_sws_duration_min");
    }

    #[test]
    fn quoted_cells_and_semicolons() {
        let rows = parse_csv("a;b;c\r\n\"x;y\";\"say \"\"hi\"\"\";3\r\n\r\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["a"], "x;y");
        assert_eq!(rows[0]["b"], "say \"hi\"");
        assert_eq!(number(&rows[0], &["c"]), Some(3.0));
    }

    #[test]
    fn later_duplicate_header_wins() {
        let rows = parse_csv("HRV,hrv\n1,2\n");
        assert_eq!(cell(&rows[0], &["hrv"]).as_deref(), Some("2"));
    }

    #[test]
    fn numbers_drop_commas_and_refuse_non_finite_values() {
        let rows = parse_csv("a,b,c,d\n\"1,234.5\",inf,abc,-2\n");
        assert_eq!(number(&rows[0], &["a"]), Some(1234.5));
        assert_eq!(number(&rows[0], &["b"]), None);
        assert_eq!(number(&rows[0], &["c"]), None);
        assert_eq!(positive(&rows[0], &["d"]), None);
        assert_eq!(number(&rows[0], &["d"]), Some(-2.0));
    }
}
