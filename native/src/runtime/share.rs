//! `ath share` is not built. It names the latest backtest report so the person
//! knows where the later share payload lives (D77).
//!
//! The placeholder stays explicitly unimplemented in 2.1.1. Hosted sharing is
//! not part of the rebuild.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

use crate::error::Result;

/// `ath share --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShareStatus {
    /// Always false.
    pub built: bool,
    /// The newest report path, or null.
    pub latest_report: Option<PathBuf>,
}

/// A backtest report name. `ath backtest` writes `backtest-<date>T<HHMMSSmmm>Z.json`
/// (the creation time without `:` and `.`). The 2.1.0 lookup expected
/// `backtest-<date>T<HHMMSS>.json`, which no version writes, so it never found
/// a report. Both forms are accepted.
pub fn is_report_name(name: &str) -> bool {
    let Some(stamp) = name.strip_prefix("backtest-").and_then(|rest| rest.strip_suffix(".json")) else {
        return false;
    };
    let bytes = stamp.as_bytes();
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    let date = bytes.len() >= 17
        && digits(0..4)
        && bytes[4] == b'-'
        && digits(5..7)
        && bytes[7] == b'-'
        && digits(8..10)
        && bytes[10] == b'T';
    date && ((bytes.len() == 17 && digits(11..17)) || (bytes.len() == 21 && digits(11..20) && bytes[20] == b'Z'))
}

/// The newest report beside the athlete file or in `cwd`, by modification time.
pub fn latest_report(athlete_path: &Path, cwd: &Path) -> Result<Option<PathBuf>> {
    let beside = match athlete_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut found: Vec<(PathBuf, SystemTime)> = Vec::new();
    for directory in [beside, cwd.to_path_buf()] {
        if seen.contains(&directory) || !directory.exists() {
            continue;
        }
        seen.push(directory.clone());
        let mut names: Vec<String> = fs::read_dir(&directory)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .filter(|name| is_report_name(name))
            .collect();
        names.sort();
        for name in names {
            let path = directory.join(&name);
            let modified = fs::metadata(&path)?.modified()?;
            found.push((path, modified));
        }
    }
    found.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    Ok(found.into_iter().next().map(|(path, _)| path))
}

/// `ath share --json`.
pub fn share_status(athlete_path: &Path, cwd: &Path) -> Result<ShareStatus> {
    Ok(ShareStatus { built: false, latest_report: latest_report(athlete_path, cwd)? })
}

/// The human `ath share` text.
pub fn share_refusal(report: Option<&Path>) -> String {
    match report {
        None => {
            ["ath share is not built yet.", "Run `ath backtest` first. The shareable part of a report is its summary."]
                .join("\n")
        }
        Some(path) => format!(
            "ath share is not built yet.\nThe shareable part of a backtest is the summary in:\n  {}",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_names_follow_the_2_1_0_pattern() {
        assert!(is_report_name("backtest-2026-01-02T030405.json"));
        assert!(!is_report_name("backtest-2026-01-02T030405Z.json"));
        assert!(is_report_name("backtest-2026-01-02T030405123Z.json"));
        assert!(!is_report_name("backtest-2026-01-02.json"));
    }
}
