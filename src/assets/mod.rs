//! Shipped skills and mapping files.
//!
//! The files under `skills/` and `mappings/` are compiled into the library by
//! `build.rs`. An executable copied into an empty directory can still install
//! the agent skill and read its shipped mappings. Nothing here reads the
//! repository at run time.
//!
//! Installed copies in a working directory win over the shipped copies where
//! the existing rules say they win: `ath import --mapping <name>` and
//! `ath apply <calculation>` read `.agents/` first. `ath calculate` reads only
//! the shipped calculation file.

pub mod reference;
pub mod skill;

use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

/// One file compiled into the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedFile {
    /// The path below `skills/` or `mappings/`, with `/` separators.
    pub path: &'static str,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

/// Every shipped file under `skills/`, sorted by path.
pub fn skill_files() -> &'static [EmbeddedFile] {
    SKILL_FILES
}

/// Every shipped file under `mappings/`, sorted by path.
pub fn mapping_files() -> &'static [EmbeddedFile] {
    MAPPING_FILES
}

/// The shipped text of `mappings/<name>.md`. Installed copies are not read.
pub fn shipped_mapping(name: &str) -> Option<&'static str> {
    let wanted = format!("{name}.md");
    find_text(MAPPING_FILES, &wanted)
}

/// The shipped text of `skills/<name>/SKILL.md`. Installed copies are not read.
pub fn shipped_calculation(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let wanted = format!("{name}/SKILL.md");
    find_text(SKILL_FILES, &wanted)
}

fn find_text(files: &'static [EmbeddedFile], path: &str) -> Option<&'static str> {
    files
        .binary_search_by(|file| file.path.cmp(path))
        .ok()
        .and_then(|index| std::str::from_utf8(files[index].bytes).ok())
}

/// Where a mapping or calculation file came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetOrigin {
    /// A path the person named.
    Path,
    /// A copy installed in the working directory's `.agents/` folder.
    Installed,
    /// The copy compiled into this program.
    Shipped,
}

/// A mapping or calculation file chosen by the precedence rules, with its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAsset {
    pub origin: AssetOrigin,
    /// The file read, or the shipped path such as `mappings/junction.md`.
    pub path: PathBuf,
    pub text: Cow<'static, str>,
}

/// True when `--mapping` names a file path rather than a mapping name.
pub fn is_mapping_path(name: &str) -> bool {
    let bytes = name.as_bytes();
    name.contains('/')
        || name.contains('\\')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

/// The mapping file for `ath import --mapping` (D178, D179).
///
/// A path is used as given, resolved against `cwd`. A name reads the installed
/// `.agents/mappings/<name>.md` first, then the shipped `mappings/<name>.md`.
pub fn resolve_mapping(name: &str, cwd: &Path) -> Result<ResolvedAsset> {
    if is_mapping_path(name) {
        let path = resolve_lexically(cwd, Path::new(name));
        if !path.exists() {
            return Err(Error::not_found(format!("no mapping file at {}", path.display())));
        }
        let text = std::fs::read_to_string(&path)?;
        return Ok(ResolvedAsset { origin: AssetOrigin::Path, path, text: Cow::Owned(text) });
    }
    let installed = cwd.join(skill::ALWAYS_DIR).join("mappings").join(format!("{name}.md"));
    if installed.exists() {
        let text = std::fs::read_to_string(&installed)?;
        return Ok(ResolvedAsset { origin: AssetOrigin::Installed, path: installed, text: Cow::Owned(text) });
    }
    if let Some(text) = shipped_mapping(name) {
        return Ok(ResolvedAsset {
            origin: AssetOrigin::Shipped,
            path: PathBuf::from(format!("mappings/{name}.md")),
            text: Cow::Borrowed(text),
        });
    }
    Err(Error::not_found(format!("no mapping file named {name}")))
}

/// The calculation file for `ath apply`: the installed
/// `.agents/skills/<name>/SKILL.md`, or the shipped copy.
///
/// A trailing `.md` and any leading folders are dropped from `name` first.
pub fn resolve_calculation(name: &str, cwd: &Path) -> Result<ResolvedAsset> {
    let skill_name = calculation_name(name);
    let installed = cwd.join(skill::ALWAYS_DIR).join("skills").join(skill_name).join("SKILL.md");
    if installed.exists() {
        let text = std::fs::read_to_string(&installed)?;
        return Ok(ResolvedAsset { origin: AssetOrigin::Installed, path: installed, text: Cow::Owned(text) });
    }
    match shipped_calculation(skill_name) {
        Some(text) => Ok(ResolvedAsset {
            origin: AssetOrigin::Shipped,
            path: PathBuf::from(format!("skills/{skill_name}/SKILL.md")),
            text: Cow::Borrowed(text),
        }),
        None => Err(Error::not_found(format!("no calculation skill named {skill_name}"))),
    }
}

/// `name.replace(/\.md$/, "").split(/[\\/]/).pop()`.
fn calculation_name(name: &str) -> &str {
    let name = name.strip_suffix(".md").unwrap_or(name);
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

/// Node's `path.resolve(cwd, name)`: join, then remove `.` and `..` without
/// reading the file system.
pub(crate) fn resolve_lexically(cwd: &Path, name: &Path) -> PathBuf {
    let joined = if name.is_absolute() { name.to_path_buf() } else { cwd.join(name) };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_sorted_for_lookup() {
        for files in [SKILL_FILES, MAPPING_FILES] {
            assert!(files.windows(2).all(|pair| pair[0].path < pair[1].path));
        }
    }

    #[test]
    fn calculation_names_drop_folders_and_suffix() {
        assert_eq!(calculation_name("heart-rate-zones"), "heart-rate-zones");
        assert_eq!(calculation_name("skills/heart-rate-zones/SKILL"), "SKILL");
        assert_eq!(calculation_name("a\\b.md"), "b");
        assert_eq!(calculation_name("baseline.md"), "baseline");
    }

    #[test]
    fn mapping_paths_are_told_apart_from_names() {
        assert!(is_mapping_path("./x.md"));
        assert!(is_mapping_path("dir\\x.md"));
        assert!(is_mapping_path("C:x.md"));
        assert!(!is_mapping_path("junction"));
        assert!(!is_mapping_path("1:x"));
    }
}
