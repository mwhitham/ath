//! Installing the three kinds of skill in 2.1.2 (D229, D231): unedited
//! calculation copies are updated, edited ones are kept and reported, and
//! coaching skills are installed only when named.

mod calculation_support;

use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::assets::skill::{
    install_named_skills, install_skill, shipped_kind, skill_names, skill_status, SkillKind, SkillState,
    PREVIOUSLY_SHIPPED_CALCULATIONS,
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

#[test]
fn coaching_skills_are_installed_only_when_named() {
    let coaching: Vec<&str> =
        skill_names().into_iter().filter(|name| shipped_kind(name) == SkillKind::Coaching).collect();
    assert_eq!(coaching, ["crossfit", "hyrox"]);
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".claude")).unwrap();
    install_skill(dir.path()).unwrap();
    for name in &coaching {
        assert!(!installed(dir.path(), name).exists(), "{name} is not installed by default");
    }
    let status = skill_status(dir.path()).unwrap();
    for copy in status.iter().filter(|copy| copy.kind == SkillKind::Coaching) {
        assert_eq!(copy.state, SkillState::Available, "{}", copy.path.display());
    }

    let written = install_named_skills(dir.path(), &["hyrox".into()]).unwrap();
    assert_eq!(written.len(), 2, "into .agents and .claude: {written:?}");
    assert!(installed(dir.path(), "hyrox").exists());
    assert!(dir.path().join(".claude/skills/hyrox/SKILL.md").exists());
    assert!(!installed(dir.path(), "crossfit").exists());

    // An older installed coaching skill is refreshed by a default install.
    let file = installed(dir.path(), "hyrox");
    let older = fs::read_to_string(&file).unwrap().replace("version: \"2.1.2\"", "version: \"2.1.1\"");
    fs::write(&file, older).unwrap();
    install_skill(dir.path()).unwrap();
    assert!(fs::read_to_string(&file).unwrap().contains("version: \"2.1.2\""));
    assert!(!installed(dir.path(), "crossfit").exists(), "still not installed by default");
}

#[test]
fn applying_a_coaching_skill_says_what_it_is() {
    let dir = tempfile::tempdir().unwrap();
    install_named_skills(dir.path(), &["crossfit".into()]).unwrap();
    let mut lab = calculation_support::Lab::new();
    lab.write_one(&serde_json::json!({ "id": "night", "record_type": "sleep_session",
        "start": "2026-10-01T23:00:00.000Z", "end": "2026-10-02T07:00:00.000Z" }));
    let id = lab.id("night");
    let error = athleticstandard::calculate::apply(&lab.store, "crossfit", &id, dir.path()).unwrap_err();
    assert!(error.to_string().starts_with("crossfit is a coaching skill, not a calculation."), "{error}");
}
