//! Installing the agent skill and the calculation skills (D79, D81).
//!
//! Each shipped skill folder is copied into `.agents/skills/`, which Claude
//! Code, Cursor, Codex, and Gemini CLI read. It is also copied into each
//! harness folder that is already present. A missing file is added. A
//! calculation skill that is already installed is left alone, because the
//! person may have edited it. An older copy of the tool skill is replaced,
//! because an agent would otherwise keep reading old instructions.
//!
//! Shipped mapping files are copied into `.agents/mappings/` only when that
//! copy is missing. An installed mapping file is never replaced.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{EmbeddedFile, MAPPING_FILES, SKILL_FILES};
use crate::error::Result;

/// The folder name the tool skill installs under, inside each skills directory.
pub const SKILL_NAME: &str = "athletic-standard";

/// Always written. The cross-client location every major harness reads.
pub const ALWAYS_DIR: &str = ".agents";

/// Written too, when the folder is already in the working directory.
pub const AGENT_DIRS: [&str; 5] = [".claude", ".cursor", ".codex", ".gemini", ".github"];

/// What a copy reads as when its `SKILL.md` has no version. Only an `ath`
/// older than 0.4.0 wrote such a copy.
pub const UNVERSIONED: &str = "unversioned";

/// Whether a skill folder has a `SKILL.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillState {
    Installed,
    Missing,
}

/// One place a skill could be installed. `ath skill --json` lists these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillCopy {
    /// The installed folder, absolute when `cwd` was absolute.
    pub path: PathBuf,
    /// The version in the copy's frontmatter, or null when there is no copy.
    pub installed: Option<String>,
    /// The tool skill version this `ath` ships. Every copy reports the tool
    /// skill's version here, including calculation skills, as 2.1.0 did.
    pub shipped: String,
    pub state: SkillState,
}

/// `ath skill --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillStatusReport {
    pub shipped: String,
    pub copies: Vec<SkillCopy>,
}

/// `ath skill install --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillInstallResult {
    pub shipped: String,
    pub skill_installed: Vec<PathBuf>,
    pub mappings_installed: Vec<PathBuf>,
}

/// A frontmatter value: text, or one nested level of `key: value` pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontmatterValue {
    Text(String),
    Map(BTreeMap<String, String>),
}

/// Shipped skill folder names that contain a `SKILL.md`, sorted.
pub fn skill_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = SKILL_FILES
        .iter()
        .filter_map(|file| file.path.strip_suffix("/SKILL.md"))
        .filter(|name| !name.contains('/'))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

fn files_of(skill: &str) -> impl Iterator<Item = (&'static str, &'static EmbeddedFile)> + '_ {
    SKILL_FILES.iter().filter_map(move |file| {
        file.path.strip_prefix(skill).and_then(|rest| rest.strip_prefix('/')).map(|relative| (relative, file))
    })
}

/// The frontmatter of a `SKILL.md`: top-level `key: value` pairs and one
/// nested level. Text without a leading `---` block has none.
pub fn read_frontmatter(text: &str) -> BTreeMap<String, FrontmatterValue> {
    let mut out = BTreeMap::new();
    let Some(block) = frontmatter_block(text) else {
        return out;
    };
    let mut nested: Option<String> = None;
    for raw in split_lines(block) {
        if raw.trim().is_empty() {
            continue;
        }
        let indented = raw.starts_with(char::is_whitespace);
        let Some((key, value)) = key_value(raw) else {
            continue;
        };
        match (&nested, indented) {
            (Some(parent), true) => {
                if let Some(FrontmatterValue::Map(map)) = out.get_mut(parent) {
                    map.insert(key.to_string(), unquote(value));
                }
            }
            _ if value.is_empty() => {
                out.insert(key.to_string(), FrontmatterValue::Map(BTreeMap::new()));
                nested = Some(key.to_string());
            }
            _ => {
                nested = None;
                out.insert(key.to_string(), FrontmatterValue::Text(unquote(value)));
            }
        }
    }
    out
}

/// `/^---\r?\n([\s\S]*?)\r?\n---/`: the text between the opening `---` line
/// and the first later line that starts with `---`.
fn frontmatter_block(text: &str) -> Option<&str> {
    let start = if text.starts_with("---\r\n") {
        5
    } else if text.starts_with("---\n") {
        4
    } else {
        return None;
    };
    let rest = &text[start..];
    let mut index = 0;
    while index <= rest.len() {
        let tail = &rest[index..];
        if tail.starts_with("\r\n---") || tail.starts_with("\n---") {
            return Some(&rest[..index]);
        }
        index += tail.chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// `text.split(/\r?\n/)`.
fn split_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line))
}

/// `/^\s*([A-Za-z_][\w-]*):\s*(.*)$/`.
fn key_value(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    let mut chars = trimmed.char_indices();
    let (_, first) = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut end = first.len_utf8();
    for (index, character) in chars {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            end = index + character.len_utf8();
        } else {
            break;
        }
    }
    let rest = trimmed[end..].strip_prefix(':')?;
    let value = rest.trim_start();
    if value.contains('\r') || value.contains('\u{2028}') || value.contains('\u{2029}') {
        return None;
    }
    Some((&trimmed[..end], value))
}

/// Trim, then remove one pair of matching surrounding quotes.
fn unquote(value: &str) -> String {
    let value = value.trim();
    let quoted = value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\'')));
    if quoted {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn version_in(text: &str) -> String {
    match read_frontmatter(text).get("metadata") {
        Some(FrontmatterValue::Map(map)) => match map.get("version") {
            Some(version) if !version.is_empty() => version.clone(),
            _ => UNVERSIONED.to_string(),
        },
        _ => UNVERSIONED.to_string(),
    }
}

/// The version in an installed skill folder's `SKILL.md`. None when there is
/// no `SKILL.md`; [`UNVERSIONED`] when it has no version.
pub fn skill_version(skill_dir: &Path) -> io::Result<Option<String>> {
    let file = skill_dir.join("SKILL.md");
    if !file.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&file)?;
    Ok(Some(version_in(&String::from_utf8_lossy(&bytes))))
}

fn shipped_version_of(skill: &str) -> Option<String> {
    let wanted = format!("{skill}/SKILL.md");
    SKILL_FILES.iter().find(|file| file.path == wanted).map(|file| version_in(&String::from_utf8_lossy(file.bytes)))
}

/// The tool skill version this `ath` ships.
pub fn shipped_skill_version() -> String {
    shipped_version_of(SKILL_NAME).unwrap_or_else(|| UNVERSIONED.to_string())
}

/// Every folder one skill would be written to in `cwd`.
pub fn skill_targets(cwd: &Path, name: &str) -> Vec<PathBuf> {
    let mut targets = vec![cwd.join(ALWAYS_DIR).join("skills").join(name)];
    for agent in AGENT_DIRS {
        if cwd.join(agent).exists() {
            targets.push(cwd.join(agent).join("skills").join(name));
        }
    }
    targets
}

/// `Number(part) || 0` for each dotted part.
fn version_rank(version: &str) -> Vec<f64> {
    version
        .split('.')
        .map(|part| {
            let value = js_number(part);
            if value.is_nan() || value == 0.0 {
                0.0
            } else {
                value
            }
        })
        .collect()
}

/// JavaScript's `Number(text)` for the forms a version part can take.
pub(crate) fn js_number(text: &str) -> f64 {
    let text = text.trim_matches(|character: char| character.is_whitespace() || character == '\u{feff}');
    if text.is_empty() {
        return 0.0;
    }
    let lower = text.to_ascii_lowercase();
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(digits) = lower.strip_prefix(prefix) {
            return match u128::from_str_radix(digits, radix) {
                Ok(value) if !digits.is_empty() => value as f64,
                _ => f64::NAN,
            };
        }
    }
    match text {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let decimal = text.chars().all(|character| character.is_ascii_digit() || "+-.eE".contains(character));
    if !decimal {
        return f64::NAN;
    }
    text.parse::<f64>().unwrap_or(f64::NAN)
}

fn version_is_older(installed: &str, shipped: &str) -> bool {
    let left = version_rank(installed);
    let right = version_rank(shipped);
    for index in 0..3 {
        let diff = left.get(index).copied().unwrap_or(0.0) - right.get(index).copied().unwrap_or(0.0);
        if diff != 0.0 && !diff.is_nan() {
            return diff < 0.0;
        }
    }
    false
}

fn write_file(destination: &Path, relative: &str, bytes: &[u8]) -> io::Result<()> {
    let target = relative.split('/').fold(destination.to_path_buf(), |path, part| path.join(part));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, bytes)
}

/// Replace the tool skill when an older copy is installed. A calculation
/// skill is never passed here.
fn replace_older_tool_skill(name: &str, destination: &Path) -> io::Result<bool> {
    let installed = skill_version(destination)?;
    let shipped = shipped_version_of(name);
    let (Some(installed), Some(shipped)) = (installed, shipped) else {
        return Ok(false);
    };
    if !version_is_older(&installed, &shipped) {
        return Ok(false);
    }
    match fs::remove_dir_all(destination) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(destination)?;
    for (relative, file) in files_of(name) {
        write_file(destination, relative, file.bytes)?;
    }
    Ok(true)
}

/// Add the files of one shipped skill that are not in `destination` yet.
fn copy_missing_files(name: &str, destination: &Path) -> io::Result<usize> {
    let mut added = 0;
    for (relative, file) in files_of(name) {
        let target = relative.split('/').fold(destination.to_path_buf(), |path, part| path.join(part));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if target.exists() {
            continue;
        }
        fs::write(&target, file.bytes)?;
        added += 1;
    }
    Ok(added)
}

/// Copy skill files that are not already installed in `cwd`.
///
/// Returns the destination folders where at least one file was added or the
/// tool skill was replaced.
pub fn install_skill(cwd: &Path) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for name in skill_names() {
        for destination in skill_targets(cwd, name) {
            fs::create_dir_all(&destination)?;
            let replaced = name == SKILL_NAME && replace_older_tool_skill(name, &destination)?;
            if replaced || copy_missing_files(name, &destination)? > 0 {
                written.push(destination);
            }
        }
    }
    Ok(written)
}

/// Copy each shipped mapping file into `.agents/mappings` when that copy is
/// missing. Returns the files written.
pub fn install_mappings(cwd: &Path) -> Result<Vec<PathBuf>> {
    let destination = cwd.join(ALWAYS_DIR).join("mappings");
    let mut written = Vec::new();
    for file in MAPPING_FILES.iter().filter(|file| !file.path.contains('/') && file.path.ends_with(".md")) {
        let target = destination.join(file.path);
        if target.exists() {
            continue;
        }
        fs::create_dir_all(&destination)?;
        fs::write(&target, file.bytes)?;
        written.push(target);
    }
    Ok(written)
}

/// `ath skill install`: the skill folders and mapping files written.
pub fn install_skill_and_mappings(cwd: &Path) -> Result<SkillInstallResult> {
    let skill_installed = install_skill(cwd)?;
    let mappings_installed = install_mappings(cwd)?;
    Ok(SkillInstallResult { shipped: shipped_skill_version(), skill_installed, mappings_installed })
}

/// Each place a shipped skill could be in `cwd`, and whether a copy is there.
pub fn skill_status(cwd: &Path) -> Result<Vec<SkillCopy>> {
    let shipped = shipped_skill_version();
    let mut copies = Vec::new();
    for name in skill_names() {
        for path in skill_targets(cwd, name) {
            let installed = skill_version(&path)?;
            let state = if installed.is_some() { SkillState::Installed } else { SkillState::Missing };
            copies.push(SkillCopy { path, installed, shipped: shipped.clone(), state });
        }
    }
    Ok(copies)
}

/// `ath skill --json`.
pub fn skill_status_report(cwd: &Path) -> Result<SkillStatusReport> {
    Ok(SkillStatusReport { shipped: shipped_skill_version(), copies: skill_status(cwd)? })
}

/// True when any skill copy is installed in `cwd`. `ath --update` refreshes
/// skills only then.
pub fn skill_present(cwd: &Path) -> Result<bool> {
    Ok(skill_status(cwd)?.iter().any(|copy| copy.state != SkillState::Missing))
}

/// The one-line notice a command would print on standard error when an
/// installed skill copy is older than the shipped one (D79).
///
/// Version 2.1.0 never prints it: an older tool skill is replaced by
/// `ath skill install` and `ath --update`, and an edited calculation skill is
/// the person's own file. This returns None for every directory. The command
/// layer still skips the check for `init`, `skill`, and `--json` runs.
pub fn stale_skill_warning(_cwd: &Path) -> Option<String> {
    None
}

/// The human `ath skill` status text.
pub fn render_skill_status(cwd: &Path, copies: &[SkillCopy]) -> String {
    let mut lines =
        vec![format!("agent skill, version {} shipped with this ath", shipped_skill_version()), String::new()];
    let cwd_text = cwd.to_string_lossy();
    for copy in copies {
        let path_text = copy.path.to_string_lossy();
        let relative = path_text.get(cwd_text.len()..).unwrap_or("").trim_start_matches(['/', '\\']);
        let place = if relative.is_empty() { path_text.as_ref() } else { relative };
        let state = match (&copy.state, &copy.installed) {
            (SkillState::Installed, Some(version)) => format!("installed ({version})"),
            (SkillState::Installed, None) => "installed (null)".to_string(),
            (SkillState::Missing, _) => "missing".to_string(),
        };
        let width = place.encode_utf16().count();
        let padding = " ".repeat(40usize.saturating_sub(width));
        lines.push(format!("  {place}{padding} {state}"));
    }
    lines.push(String::new());
    if copies.iter().any(|copy| copy.state == SkillState::Missing) {
        lines.push(
            "`ath skill install` adds skill files that are not already here, and replaces an older tool skill."
                .to_string(),
        );
    } else {
        lines.push(
            "An older tool skill is replaced by `ath skill install`. A calculation skill that is already installed is left as it is."
                .to_string(),
        );
    }
    lines.push(format!("{ALWAYS_DIR}/skills is read by Claude Code, Cursor, Codex, and Gemini CLI."));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_reads_top_level_and_one_nested_level() {
        let text = "---\nname: x\nmetadata:\n  version: \"2.1.0\"\n  other: 'q'\nlicense: MIT\n---\nbody\n";
        let meta = read_frontmatter(text);
        assert_eq!(meta.get("name"), Some(&FrontmatterValue::Text("x".into())));
        assert_eq!(meta.get("license"), Some(&FrontmatterValue::Text("MIT".into())));
        let Some(FrontmatterValue::Map(nested)) = meta.get("metadata") else { panic!("nested") };
        assert_eq!(nested.get("version").map(String::as_str), Some("2.1.0"));
        assert_eq!(nested.get("other").map(String::as_str), Some("q"));
    }

    #[test]
    fn frontmatter_accepts_crlf_and_needs_an_opening_line() {
        let meta = read_frontmatter("---\r\nmetadata:\r\n  version: 1.2.3\r\n---\r\n");
        assert_eq!(version_in("---\r\nmetadata:\r\n  version: 1.2.3\r\n---\r\n"), "1.2.3");
        assert!(meta.contains_key("metadata"));
        assert!(read_frontmatter("name: x\n---\n").is_empty());
        assert_eq!(version_in(""), UNVERSIONED);
        assert_eq!(version_in("---\nmetadata:\n---\n"), UNVERSIONED);
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(version_is_older("0.3.0", "2.1.0"));
        assert!(version_is_older(UNVERSIONED, "2.1.0"));
        assert!(!version_is_older("2.1.0", "2.1.0"));
        assert!(!version_is_older("2.10.0", "2.9.0"));
        assert!(version_is_older("2.1", "2.1.1"));
    }

    #[test]
    fn javascript_number_forms() {
        assert_eq!(js_number(""), 0.0);
        assert_eq!(js_number(" 7 "), 7.0);
        assert_eq!(js_number("0x10"), 16.0);
        assert!(js_number("1a").is_nan());
        assert_eq!(js_number("1e2"), 100.0);
    }
}
