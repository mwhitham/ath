//! Installing the three kinds of skill in 2.1.2 (D229, D231): unedited
//! calculation copies are updated, edited ones are kept and reported, and
//! coaching skills are installed only when named.

use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::assets::skill::{
    install_named_skills, install_skill, skill_status, SkillKind, PREVIOUSLY_SHIPPED_CALCULATIONS,
};
use sha2::{Digest, Sha256};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/calculations-2-1-1")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn installed(dir: &Path, name: &str) -> PathBuf {
    dir.join(".agents/skills").join(name).join("SKILL.md")
}

fn put_2_1_1(dir: &Path, name: &str) -> String {
    let text = fs::read_to_string(fixtures().join(name).join("SKILL.md")).unwrap();
    fs::create_dir_all(installed(dir, name).parent().unwrap()).unwrap();
    fs::write(installed(dir, name), &text).unwrap();
    text
}

#[test]
fn the_known_hashes_are_the_2_1_1_files() {
    let mut expected: Vec<(String, String)> = fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            (name, sha256_hex(&fs::read(path.join("SKILL.md")).unwrap()))
        })
        .collect();
    expected.sort();
    let mut actual: Vec<(String, String)> =
        PREVIOUSLY_SHIPPED_CALCULATIONS.iter().map(|(name, hash)| (name.to_string(), hash.to_string())).collect();
    actual.sort();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 23, "every 2.1.1 calculation skill");
}

#[test]
fn an_unedited_calculation_copy_is_updated_and_an_edited_one_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    put_2_1_1(dir.path(), "hrv-change");
    let edited = put_2_1_1(dir.path(), "sleep-regularity").replace("minimum_nights: 7", "minimum_nights: 5");
    fs::write(installed(dir.path(), "sleep-regularity"), &edited).unwrap();

    let before = skill_status(dir.path()).unwrap();
    let copy = |copies: &[athleticstandard::assets::skill::SkillCopy], name: &str| {
        copies.iter().find(|copy| copy.path.ends_with(format!(".agents/skills/{name}"))).unwrap().clone()
    };
    assert!(!copy(&before, "hrv-change").edited, "an unedited 2.1.1 copy is not edited");
    assert!(copy(&before, "sleep-regularity").edited, "a changed copy is edited");
    assert_eq!(copy(&before, "sleep-regularity").kind, SkillKind::Calculation);

    install_skill(dir.path()).unwrap();
    let shipped =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills/hrv-change/SKILL.md")).unwrap();
    assert_eq!(fs::read_to_string(installed(dir.path(), "hrv-change")).unwrap(), shipped, "updated");
    assert_eq!(fs::read_to_string(installed(dir.path(), "sleep-regularity")).unwrap(), edited, "kept");
    let after = skill_status(dir.path()).unwrap();
    assert!(!copy(&after, "hrv-change").edited);
    assert!(copy(&after, "sleep-regularity").edited);
}

#[test]
fn an_unknown_name_installs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let error = install_named_skills(dir.path(), &["hrv-change".into(), "no-such-skill".into()]).unwrap_err();
    assert!(error.to_string().starts_with("no shipped skill named no-such-skill. Shipped skills: "), "{error}");
    assert!(!dir.path().join(".agents").exists(), "nothing was written");
}
