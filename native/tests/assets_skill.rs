//! Shipped skills and mappings: the embedded copies, installing them, and the
//! precedence of installed files. Ported from `tests/skill.test.ts` and the
//! installed-mapping cases of `tests/mapping-cli.test.ts`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use athleticstandard::assets::reference::{command_paths, render_cli_reference};
use athleticstandard::assets::skill::{
    install_mappings, install_skill, install_skill_and_mappings, read_frontmatter, render_skill_status,
    shipped_skill_version, skill_names, skill_present, skill_status, skill_status_report, skill_version,
    stale_skill_warning, FrontmatterValue, SkillState, AGENT_DIRS, ALWAYS_DIR, SKILL_NAME, UNVERSIONED,
};
use athleticstandard::assets::{
    mapping_files, resolve_calculation, resolve_mapping, shipped_calculation, shipped_mapping, skill_files, AssetOrigin,
};

fn asset_root(name: &str) -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let beside = manifest.join(name);
    if beside.is_dir() {
        beside
    } else {
        manifest.join("..").join(name)
    }
}

/// Every non-hidden file under `root`, by `/`-separated relative path.
fn repository_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).unwrap();
                let key = relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                out.insert(key, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Every file under `root` on disk, by `/`-separated relative path.
fn installed_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    repository_files(root)
}

fn tempdir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("ath-skill-").tempdir().unwrap()
}

fn dir_with_agents(agents: &[&str]) -> tempfile::TempDir {
    let dir = tempdir();
    for agent in agents {
        fs::create_dir(dir.path().join(agent)).unwrap();
    }
    dir
}

fn skill_file(dir: &Path, agent: &str, file: &str) -> PathBuf {
    dir.join(agent).join("skills").join(SKILL_NAME).join(file)
}

/// Rewrite one installed copy's frontmatter so it reads as from an older ath.
fn age_copy(dir: &Path, agent: &str, version: &str) {
    let file = skill_file(dir, agent, "SKILL.md");
    let text = fs::read_to_string(&file).unwrap();
    let start = text.find("version: \"").unwrap() + "version: \"".len();
    let end = start + text[start..].find('"').unwrap();
    fs::write(&file, format!("{}{}{}", &text[..start], version, &text[end..])).unwrap();
}

fn embedded(files: &[athleticstandard::assets::EmbeddedFile]) -> BTreeMap<String, Vec<u8>> {
    files.iter().map(|file| (file.path.to_string(), file.bytes.to_vec())).collect()
}

fn skill_text() -> &'static str {
    shipped_calculation(SKILL_NAME).unwrap()
}

#[test]
fn embedded_assets_match_the_repository_byte_for_byte() {
    assert_eq!(embedded(skill_files()), repository_files(&asset_root("skills")));
    assert_eq!(embedded(mapping_files()), repository_files(&asset_root("mappings")));
    assert!(skill_files().len() > 20);
    assert!(mapping_files().len() >= 2);
}

const PROBE: &str = "ATH_ASSET_PROBE";

/// A copy of this test executable, run from an empty directory far from the
/// repository, installs the skill and mappings from its own embedded bytes.
#[test]
fn a_copied_executable_installs_skills_from_its_embedded_bytes() {
    if std::env::var_os(PROBE).is_some() {
        let cwd = std::env::current_dir().unwrap();
        let result = install_skill_and_mappings(&cwd).unwrap();
        assert!(!result.skill_installed.is_empty());
        assert_eq!(resolve_mapping("junction", &cwd).unwrap().origin, AssetOrigin::Installed);
        return;
    }
    let place = tempdir();
    let work = place.path().join("empty folder ü");
    fs::create_dir(&work).unwrap();
    let copy = place.path().join(format!("copied-ath{}", std::env::consts::EXE_SUFFIX));
    fs::copy(std::env::current_exe().unwrap(), &copy).unwrap();
    let output = Command::new(&copy)
        .args(["--exact", "a_copied_executable_installs_skills_from_its_embedded_bytes", "--test-threads=1"])
        .env(PROBE, "1")
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let skills = installed_files(&work.join(ALWAYS_DIR).join("skills"));
    assert_eq!(skills, repository_files(&asset_root("skills")));
    let mappings = installed_files(&work.join(ALWAYS_DIR).join("mappings"));
    assert_eq!(mappings, repository_files(&asset_root("mappings")));
}

// The skill itself.

#[test]
fn the_skill_names_the_version_it_ships() {
    let version = shipped_skill_version();
    assert_ne!(version, UNVERSIONED);
    assert!(skill_text().contains(&version));
}

#[test]
#[ignore = "skills/ still ship 2.1.0 until the 2.1.1 cutover regenerates them"]
fn the_skill_names_this_program_version() {
    assert!(skill_text().contains(athleticstandard::model::package::ATHLETIC_STANDARD_VERSION));
    assert_eq!(shipped_skill_version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn the_skill_keeps_its_detail_in_references() {
    for file in ["cli.md", "format.md", "logging.md", "predicting.md", "grading.md"] {
        assert!(skill_files().iter().any(|f| f.path == format!("{SKILL_NAME}/references/{file}")), "{file}");
        assert!(skill_text().contains(&format!("(references/{file})")), "SKILL.md should point at {file}");
    }
    assert!(skill_text().split('\n').count() < 500);
}

#[test]
fn the_skill_carries_the_workout_naming_procedure_and_leaves_rules_to_the_tools() {
    let logging = skill_files().iter().find(|f| f.path == format!("{SKILL_NAME}/references/logging.md")).unwrap();
    let logging = String::from_utf8_lossy(logging.bytes).to_string();
    assert!(logging.contains("--benchmark"));
    assert!(logging.contains("near variation"));
    assert!(logging.contains("fran"));
    assert!(logging.contains("murph"));
    assert!(logging.to_lowercase().contains("hyrox"));
    assert!(skill_text().contains("The database enforces the source"));
    assert!(skill_text().contains("never share a calculation"));
}

#[test]
fn the_frontmatter_follows_the_agent_skills_shape() {
    let meta = read_frontmatter(skill_text());
    let text = |key: &str| match meta.get(key) {
        Some(FrontmatterValue::Text(value)) => value.clone(),
        other => panic!("{key}: {other:?}"),
    };
    assert_eq!(text("name"), SKILL_NAME);
    let description = text("description");
    assert!(description.encode_utf16().count() < 1024);
    assert!(!description.starts_with("Use ") && description != "Use");
    for trigger in
        ["workout", "HRV", "sleep", "soreness", "Fran", "backtest", "Apple Health", "WHOOP", "Oura", ".ath.sqlite"]
    {
        assert!(description.contains(trigger), "{trigger}");
    }
    assert_eq!(text("license"), "MIT");
    assert!(text("compatibility").contains("ath"));
    let Some(FrontmatterValue::Map(metadata)) = meta.get("metadata") else { panic!("metadata") };
    assert_eq!(metadata.get("version"), Some(&shipped_skill_version()));
}

// Installing (D79).

#[test]
fn install_always_writes_to_agents_skills_even_with_no_agent_folder() {
    let dir = dir_with_agents(&[]);
    let written = install_skill(dir.path()).unwrap();
    assert!(skill_file(dir.path(), ALWAYS_DIR, "SKILL.md").exists());
    assert!(skill_file(dir.path(), ALWAYS_DIR, "references/cli.md").exists());
    assert!(written.contains(&dir.path().join(ALWAYS_DIR).join("skills").join(SKILL_NAME)));
    for agent in AGENT_DIRS {
        assert!(!dir.path().join(agent).exists(), "{agent}");
    }
}

#[test]
fn install_also_copies_into_every_harness_folder_it_finds() {
    let dir = dir_with_agents(&AGENT_DIRS);
    let written = install_skill(dir.path()).unwrap();
    for agent in std::iter::once(ALWAYS_DIR).chain(AGENT_DIRS) {
        assert!(skill_file(dir.path(), agent, "SKILL.md").exists(), "{agent}");
        assert!(skill_file(dir.path(), agent, "references/grading.md").exists(), "{agent}");
        assert!(written.contains(&dir.path().join(agent).join("skills").join(SKILL_NAME)), "{agent}");
    }
}

#[test]
fn install_reports_every_folder_including_calculation_skills() {
    let dir = dir_with_agents(&[".claude"]);
    let result = install_skill_and_mappings(dir.path()).unwrap();
    for expected in [
        dir.path().join(ALWAYS_DIR).join("skills").join(SKILL_NAME),
        dir.path().join(".claude").join("skills").join(SKILL_NAME),
        dir.path().join(ALWAYS_DIR).join("skills").join("heart-rate-zones"),
    ] {
        assert!(result.skill_installed.contains(&expected), "{}", expected.display());
    }
    assert_eq!(result.skill_installed.len(), skill_names().len() * 2);
    assert_eq!(result.shipped, shipped_skill_version());
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(
        json.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["shipped", "skill_installed", "mappings_installed"]
    );

    let again = install_skill_and_mappings(dir.path()).unwrap();
    assert!(again.skill_installed.is_empty());
    assert!(again.mappings_installed.is_empty());
}

#[test]
fn status_reports_each_copy_as_installed_or_missing() {
    let dir = dir_with_agents(&[".codex", ".gemini"]);
    install_skill(dir.path()).unwrap();
    age_copy(dir.path(), ".codex", "0.3.0");
    fs::write(skill_file(dir.path(), ".gemini", "SKILL.md"), "").unwrap();
    fs::create_dir(dir.path().join(".github")).unwrap();

    let copies = skill_status(dir.path()).unwrap();
    let tool: BTreeMap<String, _> = copies
        .iter()
        .filter(|copy| copy.path.ends_with(SKILL_NAME))
        .map(|copy| {
            let agent = copy.path.strip_prefix(dir.path()).unwrap().components().next().unwrap();
            (agent.as_os_str().to_string_lossy().into_owned(), copy.clone())
        })
        .collect();
    assert_eq!(tool[ALWAYS_DIR].state, SkillState::Installed);
    assert_eq!((tool[".codex"].state, tool[".codex"].installed.as_deref()), (SkillState::Installed, Some("0.3.0")));
    assert_eq!(
        (tool[".gemini"].state, tool[".gemini"].installed.as_deref()),
        (SkillState::Installed, Some(UNVERSIONED))
    );
    assert_eq!((tool[".github"].state, tool[".github"].installed.as_deref()), (SkillState::Missing, None));

    let text = render_skill_status(dir.path(), &copies);
    assert!(text.contains("installed (0.3.0)"));
    assert!(text.contains("missing"));
    assert!(!text.contains("stale"));
    assert!(text.starts_with(&format!("agent skill, version {} shipped with this ath", shipped_skill_version())));

    let report = serde_json::to_value(skill_status_report(dir.path()).unwrap()).unwrap();
    assert_eq!(report["shipped"], shipped_skill_version());
    let missing = report["copies"].as_array().unwrap().iter().find(|copy| copy["state"] == "missing").unwrap();
    assert!(missing["installed"].is_null());
    assert!(dir.path().join(ALWAYS_DIR).join("skills/heart-rate-zones/SKILL.md").exists());
}

#[test]
fn install_replaces_an_older_tool_skill_and_leaves_an_edited_current_one() {
    let dir = dir_with_agents(&[".codex"]);
    install_skill(dir.path()).unwrap();
    fs::write(skill_file(dir.path(), ".codex", "left-over.md"), "kept\n").unwrap();
    let edited = format!("---\nmetadata:\n  version: \"{}\"\n---\nedited\n", shipped_skill_version());
    fs::write(skill_file(dir.path(), ".codex", "SKILL.md"), &edited).unwrap();
    age_copy(dir.path(), ALWAYS_DIR, "0.3.0");

    let written = install_skill(dir.path()).unwrap();
    assert_eq!(written, [dir.path().join(ALWAYS_DIR).join("skills").join(SKILL_NAME)]);
    assert_eq!(fs::read_to_string(skill_file(dir.path(), ".codex", "SKILL.md")).unwrap(), edited);
    assert_eq!(
        fs::read(skill_file(dir.path(), ALWAYS_DIR, "SKILL.md")).unwrap(),
        skill_text().as_bytes(),
        "the older copy is the shipped file again"
    );
    assert_eq!(fs::read_to_string(skill_file(dir.path(), ".codex", "left-over.md")).unwrap(), "kept\n");
}

#[test]
fn install_keeps_an_edited_calculation_skill_even_when_older() {
    let dir = dir_with_agents(&[]);
    install_skill(dir.path()).unwrap();
    let calculation = dir.path().join(ALWAYS_DIR).join("skills/heart-rate-zones/SKILL.md");
    let edited = "---\nmetadata:\n  version: \"0.0.1\"\n---\nmy zones\n";
    fs::write(&calculation, edited).unwrap();
    let written = install_skill(dir.path()).unwrap();
    assert!(written.is_empty());
    assert_eq!(fs::read_to_string(&calculation).unwrap(), edited);
    assert_eq!(skill_version(calculation.parent().unwrap()).unwrap().as_deref(), Some("0.0.1"));
}

#[test]
fn install_adds_a_missing_file_to_an_existing_copy() {
    let dir = dir_with_agents(&[]);
    install_skill(dir.path()).unwrap();
    let grading = skill_file(dir.path(), ALWAYS_DIR, "references/grading.md");
    fs::remove_file(&grading).unwrap();
    let written = install_skill(dir.path()).unwrap();
    assert_eq!(written, [dir.path().join(ALWAYS_DIR).join("skills").join(SKILL_NAME)]);
    assert!(grading.exists());
}

#[test]
fn the_stale_warning_is_never_shown() {
    let none = dir_with_agents(&[]);
    assert_eq!(stale_skill_warning(none.path()), None);
    assert!(!skill_present(none.path()).unwrap());

    let edited = dir_with_agents(&[".cursor"]);
    install_skill(edited.path()).unwrap();
    age_copy(edited.path(), ".cursor", "0.3.0");
    assert_eq!(stale_skill_warning(edited.path()), None);
    assert!(skill_present(edited.path()).unwrap());
}

// Mapping files (D178, D179).

#[test]
fn installed_mappings_are_copied_once_and_never_replaced() {
    let dir = tempdir();
    let written = install_mappings(dir.path()).unwrap();
    let names: Vec<String> =
        written.iter().map(|path| path.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["junction.md", "open-wearables.md"]);
    let junction = dir.path().join(ALWAYS_DIR).join("mappings/junction.md");
    fs::write(&junction, "edited\n").unwrap();
    fs::remove_file(dir.path().join(ALWAYS_DIR).join("mappings/open-wearables.md")).unwrap();
    let again = install_mappings(dir.path()).unwrap();
    assert_eq!(again, [dir.path().join(ALWAYS_DIR).join("mappings/open-wearables.md")]);
    assert_eq!(fs::read_to_string(&junction).unwrap(), "edited\n");
}

#[test]
fn the_installed_mapping_file_is_the_one_that_runs() {
    let dir = tempdir();
    let shipped = resolve_mapping("junction", dir.path()).unwrap();
    assert_eq!(shipped.origin, AssetOrigin::Shipped);
    assert_eq!(shipped.text, shipped_mapping("junction").unwrap());

    let installed = dir.path().join(ALWAYS_DIR).join("mappings");
    fs::create_dir_all(&installed).unwrap();
    fs::write(installed.join("junction.md"), "edited junction\r\n").unwrap();
    fs::write(installed.join("my-source.md"), "a person's own mapping\n").unwrap();
    let chosen = resolve_mapping("junction", dir.path()).unwrap();
    assert_eq!((chosen.origin, chosen.text.as_ref()), (AssetOrigin::Installed, "edited junction\r\n"));
    assert_eq!(chosen.path, installed.join("junction.md"));
    assert_eq!(resolve_mapping("my-source", dir.path()).unwrap().text, "a person's own mapping\n");

    fs::write(dir.path().join("elsewhere.md"), "by path\n").unwrap();
    let by_path = resolve_mapping("./sub/../elsewhere.md", dir.path()).unwrap();
    assert_eq!((by_path.origin, by_path.path.clone()), (AssetOrigin::Path, dir.path().join("elsewhere.md")));

    let missing = resolve_mapping("nosuch", dir.path()).unwrap_err().to_string();
    assert_eq!(missing, "no mapping file named nosuch");
    let missing_path = resolve_mapping("./nosuch.md", dir.path()).unwrap_err().to_string();
    assert_eq!(missing_path, format!("no mapping file at {}", dir.path().join("nosuch.md").display()));
}

#[test]
fn an_installed_calculation_file_wins_for_apply_but_not_for_the_shipped_lookup() {
    let dir = tempdir();
    let shipped = resolve_calculation("heart-rate-zones.md", dir.path()).unwrap();
    assert_eq!(shipped.origin, AssetOrigin::Shipped);
    assert_eq!(shipped.text, shipped_calculation("heart-rate-zones").unwrap());

    let installed = dir.path().join(ALWAYS_DIR).join("skills/heart-rate-zones");
    fs::create_dir_all(&installed).unwrap();
    fs::write(installed.join("SKILL.md"), "edited zones\n").unwrap();
    let chosen = resolve_calculation("skills/heart-rate-zones", dir.path()).unwrap();
    assert_eq!((chosen.origin, chosen.text.as_ref()), (AssetOrigin::Installed, "edited zones\n"));
    assert_ne!(shipped_calculation("heart-rate-zones").unwrap(), "edited zones\n");

    assert_eq!(resolve_calculation("nosuch", dir.path()).unwrap_err().to_string(), "no calculation skill named nosuch");
    assert_eq!(shipped_calculation("../skills/baseline"), None);
    assert_eq!(shipped_mapping("../skills/baseline/SKILL"), None);
}

// The generated command reference (D80).

fn sample_program() -> clap::Command {
    clap::Command::new("ath")
        .about("Athletic Standard")
        .after_help("Root footer that the reference leaves out.")
        .subcommand(
            clap::Command::new("key")
                .about("save or show a gateway key")
                .after_help("Examples:\n  $ ath key")
                .subcommand(clap::Command::new("set").about("save a key").arg(clap::Arg::new("gateway").required(true)))
                .subcommand(clap::Command::new("clear").about("delete a key")),
        )
        .subcommand(clap::Command::new("skill").about("where the skill is").subcommand(clap::Command::new("install")))
}

#[test]
fn the_reference_covers_every_command_and_subcommand() {
    let program = sample_program();
    let paths = command_paths(&program);
    assert_eq!(paths, ["ath key", "ath key set", "ath key clear", "ath skill", "ath skill install"]);
    let reference = render_cli_reference(&program);
    assert!(reference.starts_with("# ath command reference\n"));
    assert!(reference.ends_with("```\n"));
    for path in &paths {
        assert!(reference.contains(&format!("\n## {path}\n")), "{path}");
    }
    assert!(reference.contains("- [ath key set](#ath-key-set) — save a key"));
    assert!(reference.contains("Examples:\n  $ ath key"));
    assert!(reference.contains("Usage: ath key set <gateway>"));
    assert!(!reference.contains("Root footer"));
    assert_eq!(reference, render_cli_reference(&program), "the reference is deterministic");
}
