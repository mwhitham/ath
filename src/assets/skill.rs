//! Installing the tool, calculation, and coaching skills (D79, D81, D228).
//!
//! Each shipped skill folder is copied into `.agents/skills/`, which Claude
//! Code, Cursor, Codex, and Gemini CLI read. It is also copied into each
//! harness folder that is already present. A missing file is added.
//!
//! - The tool skill and calculation skills are installed by default. A
//!   coaching skill is installed only when it is named (D229).
//! - An older copy of the tool skill or a coaching skill is replaced, because
//!   an agent would otherwise keep reading old instructions.
//! - An installed calculation skill is replaced only when its `SKILL.md` is
//!   byte for byte a file an earlier release shipped, so the person never
//!   edited it (D231). An edited copy is left alone and reported as edited.
//!
//! Shipped mapping files are copied into `.agents/mappings/` only when that
//! copy is missing. An installed mapping file is never replaced.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{EmbeddedFile, MAPPING_FILES, SKILL_FILES};
use crate::error::{Error, Result};

/// The folder name the tool skill installs under, inside each skills directory.
pub const SKILL_NAME: &str = "athletic-standard";

/// Always written. The cross-client location every major harness reads.
pub const ALWAYS_DIR: &str = ".agents";

/// Written too, when the folder is already in the working directory.
pub const AGENT_DIRS: [&str; 5] = [".claude", ".cursor", ".codex", ".gemini", ".github"];

/// What a copy reads as when its `SKILL.md` has no version. Only an `ath`
/// older than 0.4.0 wrote such a copy.
pub const UNVERSIONED: &str = "unversioned";

/// Whether a skill folder has a `SKILL.md`. A coaching skill that is not
/// installed is `available`: it is installed only when named (D229).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillState {
    Installed,
    Missing,
    Available,
}

/// What a shipped skill is for (D228).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillKind {
    /// Reads and writes the record: `athletic-standard`.
    Tool,
    /// Produces one reproducible result.
    Calculation,
    /// Assesses, plans, adapts, and reviews training for one sport.
    Coaching,
}

/// The SHA-256 of every calculation `SKILL.md` an earlier release shipped
/// (D231). A copy with one of these hashes was never edited, so it is
/// replaced by the current file. The 2.0.0, 2.1.0, and 2.1.1 files are the
/// same; `tests/fixtures/calculations-2-1-1` keeps them.
pub const PREVIOUSLY_SHIPPED_CALCULATIONS: &[(&str, &str)] = &[
    ("baseline", "b89283be4e217b785b17b96c711dce4c17e660cdcff8a58838f9ad7e829f9e9f"),
    ("compare-sources", "3977b56322124b91e95bf870deb8d2b6d3f2411b8545cd1336471787f5e55dff"),
    ("correlation", "de7400002cdb901fa0b8a80af23e9e34d5ab21dab4c725cc15f99451d42c6b63"),
    ("coverage", "7ad67f0d10864e7e433f10c585907a7b32b8fcabac255f3bed4826fb8f8b183a"),
    ("heart-rate-recovery", "57041f7e55e3b4a8eaf2a0ec6b8db428cd9627b45c18c33cc9e95c289eb91821"),
    ("heart-rate-zones", "bed7403c07bf94744209ef34d8bea1ad0014b356c20f90f0018898ba24f94b3a"),
    ("hrv-change", "be5aab44fa83749598f95769eb7c3c17ed8f623b479c7cff843b52333e97754f"),
    ("provider-period-summary", "8b53ee9f7aca520571440c3343d7ad39650865cf73f5e93f7b6fb4850048bc73"),
    ("resting-heart-rate-change", "24ed1e2e9e1713d3bba8a3aaf9a22e457a8210af82cad1af4016a57673e450bc"),
    ("resting-heart-rate", "7627646242280399a9cbf30a5613b046929465be62d66b43a83b0699374ff11c"),
    ("sleep-change", "aa1c5c13ca166b26ad381b289b1924c0c197bfe7b3a13f9df9348a7da2372785"),
    ("sleep-duration", "b011a83053e5e4ce9d364032969ad8b2939ba020a784a91ef1da25f4901f13fe"),
    ("sleep-efficiency", "dbcfc7c38d4ae05fd7aa359def167cab37c2f1b0b758c35fd3a3796efe7f7a88"),
    ("sleep-interruptions", "8b773b3993ee1e0f1aeaecf34b58173154a8a651832cfe8a1ffd79ebe12f481f"),
    ("sleep-regularity", "e024d60251ec056a5950792d694d2ddc2a6bd2940f380013cf9e5d1bc30f14d7"),
    ("sleep-stages", "87a3b4f9665e0e57b2789f0408591146db2b70ce298dcb7b9d18a85a1b122d2b"),
    ("sleep-timing", "647226abf89181b4b06cf26df1b70440867ae7a4bf17c167426bec1708538853"),
    ("strength-volume", "c06ff53d223b495d397cc50f09833f32653224a17e1f73297aea14165d53042b"),
    ("summary", "282c7bc312aa36f469530788b7a502136c2afa1d9690741397079fbd3570744b"),
    ("training-load-effort", "e99189a3a0fe5a095166cba9fbf9abf76e616b7f5ebaa080aa212b5ea3918cff"),
    ("training-load-heart-rate", "7268d3a3ff249fa1a499cc355e867bff5dc9d00bbeeb85cfab7c44f3d87900a6"),
    ("trend", "71f7027c3330336e2f18228d10e81ae0bb33919b00b540752d5e9562fdacf065"),
    ("workout-drift", "0d5abf11d1ac2f50ae0566b0816a573581da97f4159b23e5c94584736450dfbf"),
];

/// One place a skill could be installed. `ath skill --json` lists these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillCopy {
    /// The installed folder, absolute when `cwd` was absolute.
    pub path: PathBuf,
    pub kind: SkillKind,
    /// The version in the copy's frontmatter, or null when there is no copy.
    pub installed: Option<String>,
    /// The tool skill version this `ath` ships. Every copy reports the tool
    /// skill's version here, including calculation skills, as 2.1.0 did.
    pub shipped: String,
    pub state: SkillState,
    /// A calculation copy that matches no shipped file: the person edited it,
    /// so it is kept and runs its own method (D231).
    pub edited: bool,
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

/// The heading of the section every shipped skill carries (D250).
pub const PREREQUISITE_HEADING: &str = "## Before the first ath command";

/// The section every shipped skill carries, for skills of `version` (D250).
///
/// A skill installed with the skills CLI arrives without `ath`, so each skill
/// says how to check for `ath`, where it really comes from, and to ask before
/// installing it. What `ath` can say for itself, such as a missing database,
/// stays in its own messages. `tests/generated.rs` writes this text into every
/// `SKILL.md` and fails when a copy differs.
pub fn prerequisite_section(version: &str) -> String {
    format!(
        "{PREREQUISITE_HEADING}

This skill runs `ath`, the Athletic Standard program, version {version} or later.
Check it once with `ath --version`.

- If `ath` is not found, it is not installed. It comes from the npm package
  `athleticstandard`. The npm package named `ath` is a different program. Install it
  with `npm install -g athleticstandard`, which needs Node.js 22 or later. Other ways
  to install it are at https://www.npmjs.com/package/athleticstandard.
- If the version is older than {version}, update it with `ath --update`.
- Install or update only after the person agrees. Then run `ath --version` again. If
  it is still older than {version}, say so and stop.

If the answer needs the athlete's records or a calculation and `ath` cannot run, stop.
Tell the person what is missing and how to fix it. Do not read the database another
way or do the calculation yourself: the calculations and their checks run only in
`ath`. For any other message from `ath`, follow what it says, and ask before anything
that creates or changes a file.
"
    )
}

/// `text` with its prerequisite section replaced by [`prerequisite_section`].
/// The section runs from its heading to the next `## ` heading. `None` when
/// the text has no such heading.
pub fn with_prerequisite_section(text: &str, version: &str) -> Option<String> {
    let start = text.match_indices(PREREQUISITE_HEADING).find_map(|(index, _)| {
        let at_line_start = index == 0 || text[..index].ends_with('\n');
        let line_end = text[index + PREREQUISITE_HEADING.len()..].chars().next();
        (at_line_start && matches!(line_end, None | Some('\n' | '\r'))).then_some(index)
    })?;
    let body_start = start + PREREQUISITE_HEADING.len();
    let end = text[body_start..].find("\n## ").map_or(text.len(), |offset| body_start + offset + 1);
    Some(format!("{}{}\n{}", &text[..start], prerequisite_section(version), &text[end..]))
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

/// The kind of a shipped skill: the tool skill by name, a coaching skill by
/// `metadata.kind`, and otherwise a calculation.
pub fn shipped_kind(name: &str) -> SkillKind {
    if name == SKILL_NAME {
        return SkillKind::Tool;
    }
    let wanted = format!("{name}/SKILL.md");
    let kind = SKILL_FILES.iter().find(|file| file.path == wanted).and_then(|file| {
        match read_frontmatter(&String::from_utf8_lossy(file.bytes)).get("metadata") {
            Some(FrontmatterValue::Map(map)) => map.get("kind").cloned(),
            _ => None,
        }
    });
    match kind.as_deref() {
        Some("coaching") => SkillKind::Coaching,
        Some("tool") => SkillKind::Tool,
        _ => SkillKind::Calculation,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn shipped_bytes(name: &str) -> Option<&'static [u8]> {
    let wanted = format!("{name}/SKILL.md");
    SKILL_FILES.iter().find(|file| file.path == wanted).map(|file| file.bytes)
}

/// True when an installed calculation `SKILL.md` matches no file this or an
/// earlier release shipped.
fn calculation_edited(name: &str, installed: &[u8]) -> bool {
    let hash = sha256_hex(installed);
    shipped_bytes(name) != Some(installed)
        && !PREVIOUSLY_SHIPPED_CALCULATIONS.iter().any(|(skill, shipped)| *skill == name && *shipped == hash)
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

/// Replace the tool skill or a coaching skill when an older copy is
/// installed.
fn replace_older_versioned_skill(name: &str, destination: &Path) -> io::Result<bool> {
    let installed = skill_version(destination)?;
    let shipped = shipped_version_of(name);
    let (Some(installed), Some(shipped)) = (installed, shipped) else {
        return Ok(false);
    };
    if !version_is_older(&installed, &shipped) {
        return Ok(false);
    }
    replace_folder(name, destination)?;
    Ok(true)
}

/// Replace a calculation skill whose `SKILL.md` an earlier release shipped
/// unchanged (D231).
fn replace_unedited_calculation(name: &str, destination: &Path) -> io::Result<bool> {
    let file = destination.join("SKILL.md");
    if !file.exists() {
        return Ok(false);
    }
    let installed = fs::read(&file)?;
    let hash = sha256_hex(&installed);
    let unedited_old = shipped_bytes(name) != Some(installed.as_slice())
        && PREVIOUSLY_SHIPPED_CALCULATIONS.iter().any(|(skill, shipped)| *skill == name && *shipped == hash);
    if !unedited_old {
        return Ok(false);
    }
    replace_folder(name, destination)?;
    Ok(true)
}

/// Remove a skill folder and write the shipped files in its place.
fn replace_folder(name: &str, destination: &Path) -> io::Result<()> {
    match fs::remove_dir_all(destination) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(destination)?;
    for (relative, file) in files_of(name) {
        write_file(destination, relative, file.bytes)?;
    }
    Ok(())
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

/// Install one shipped skill into one folder by its kind's rules. True when a
/// file was added or the folder was replaced.
fn install_one(name: &str, destination: &Path) -> io::Result<bool> {
    fs::create_dir_all(destination)?;
    let replaced = match shipped_kind(name) {
        SkillKind::Tool | SkillKind::Coaching => replace_older_versioned_skill(name, destination)?,
        SkillKind::Calculation => replace_unedited_calculation(name, destination)?,
    };
    Ok(replaced || copy_missing_files(name, destination)? > 0)
}

/// Install the tool skill and the calculation skills, and refresh any coaching
/// skill that is already installed (D229). Returns the destination folders
/// where a file was added or a folder was replaced.
pub fn install_skill(cwd: &Path) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for name in skill_names() {
        let coaching = shipped_kind(name) == SkillKind::Coaching;
        for destination in skill_targets(cwd, name) {
            if coaching && !destination.join("SKILL.md").exists() {
                continue;
            }
            if install_one(name, &destination)? {
                written.push(destination);
            }
        }
    }
    Ok(written)
}

/// `ath skill install <name>...`: install the named shipped skills, of any
/// kind, by the same rules. An unknown name installs nothing.
pub fn install_named_skills(cwd: &Path, names: &[String]) -> Result<Vec<PathBuf>> {
    let shipped = skill_names();
    for name in names {
        if !shipped.contains(&name.as_str()) {
            return Err(Error::not_found(format!(
                "no shipped skill named {name}. Shipped skills: {}",
                shipped.join(", ")
            )));
        }
    }
    let mut written = Vec::new();
    for name in names {
        for destination in skill_targets(cwd, name) {
            if install_one(name, &destination)? && !written.contains(&destination) {
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

/// `ath skill install <name>...`: only the named skills; no mapping files.
pub fn install_named(cwd: &Path, names: &[String]) -> Result<SkillInstallResult> {
    let skill_installed = install_named_skills(cwd, names)?;
    Ok(SkillInstallResult { shipped: shipped_skill_version(), skill_installed, mappings_installed: Vec::new() })
}

/// Each place a shipped skill could be in `cwd`, and whether a copy is there.
pub fn skill_status(cwd: &Path) -> Result<Vec<SkillCopy>> {
    let shipped = shipped_skill_version();
    let mut copies = Vec::new();
    for name in skill_names() {
        let kind = shipped_kind(name);
        for path in skill_targets(cwd, name) {
            let installed = skill_version(&path)?;
            let state = match (&installed, kind) {
                (Some(_), _) => SkillState::Installed,
                (None, SkillKind::Coaching) => SkillState::Available,
                (None, _) => SkillState::Missing,
            };
            let edited = kind == SkillKind::Calculation
                && installed.is_some()
                && calculation_edited(name, &fs::read(path.join("SKILL.md"))?);
            copies.push(SkillCopy { path, kind, installed, shipped: shipped.clone(), state, edited });
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
    Ok(skill_status(cwd)?.iter().any(|copy| copy.state == SkillState::Installed))
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
            (SkillState::Installed, Some(version)) if copy.edited => {
                format!("installed ({version}), edited: kept, runs its own method")
            }
            (SkillState::Installed, Some(version)) => format!("installed ({version})"),
            (SkillState::Installed, None) => "installed (null)".to_string(),
            (SkillState::Missing, _) => "missing".to_string(),
            (SkillState::Available, _) => "available: coaching, installed only when named".to_string(),
        };
        let width = place.encode_utf16().count();
        let padding = " ".repeat(40usize.saturating_sub(width));
        lines.push(format!("  {place}{padding} {state}"));
    }
    lines.push(String::new());
    if copies.iter().any(|copy| copy.state == SkillState::Missing) {
        lines.push(
            "`ath skill install` adds skill files that are not already here, replaces an older tool or coaching skill, and updates an unedited calculation skill."
                .to_string(),
        );
    } else {
        lines.push(
            "An older tool or coaching skill is replaced by `ath skill install`. An unedited calculation skill is updated; an edited one is left as it is."
                .to_string(),
        );
    }
    let mut available: Vec<&str> = copies
        .iter()
        .filter(|copy| copy.state == SkillState::Available)
        .filter_map(|copy| copy.path.file_name().and_then(|name| name.to_str()))
        .collect();
    available.dedup();
    if !available.is_empty() {
        lines.push(format!(
            "Coaching skills are installed only when named, such as `ath skill install {}`.",
            available.join(" ")
        ));
    }
    lines.push(format!("{ALWAYS_DIR}/skills is read by Claude Code, Cursor, Codex, and Gemini CLI."));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prerequisite_section_replaces_only_its_own_text() {
        let text = format!("# Skill\n\nIntro.\n\n{PREREQUISITE_HEADING}\n\nold text\n\n## Run it\n\nbody\n");
        let written = with_prerequisite_section(&text, "9.9.9").unwrap();
        assert!(written.starts_with("# Skill\n\nIntro.\n\n## Before the first ath command\n\n"));
        assert!(written.ends_with(".\n\n## Run it\n\nbody\n"), "{written}");
        assert!(!written.contains("old text"));
        assert!(written.contains("version 9.9.9 or later"));
        assert_eq!(with_prerequisite_section(&written, "9.9.9").as_deref(), Some(written.as_str()));
        let last = format!("# Skill\n\n{PREREQUISITE_HEADING}\nold\n");
        assert!(with_prerequisite_section(&last, "9.9.9")
            .unwrap()
            .ends_with("ask before anything\nthat creates or changes a file.\n\n"));
        assert_eq!(with_prerequisite_section("# Skill\n\n## Before the first ath commands\n", "1"), None);
        assert_eq!(with_prerequisite_section("# Skill\n", "1"), None);
    }

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
