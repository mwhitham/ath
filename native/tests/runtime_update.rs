//! `ath --update`. Ported from `tests/update.test.ts`, with the launcher
//! handoff, Cargo installs, release archives, and source checkouts added.
//! The registry is a closure or a local server; no package manager runs.
#![cfg_attr(not(unix), allow(unused))]

mod support;

use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::assets::skill::{install_skill, skill_version, ALWAYS_DIR, SKILL_NAME};
use athleticstandard::runtime::update::{
    candidate_paths, choose_install, classify_install_path, compare_versions, fetch_latest_version, run_update,
    skill_update_line, write_plan, InstallContext, InstallKind, PackageManager, Stream, UpdateOutcome, UpdatePhase,
    UpdatePlan, UpdateRequest, BUILD_TARGET, HANDOFF_VARIABLE, INSTALL_PATHS_VARIABLE, PHASE_VARIABLE,
};
use support::{Reply, Server};

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const NPM_PATH: &str = "/usr/local/lib/node_modules/athleticstandard";

fn tempdir() -> tempfile::TempDir {
    let base = fs::canonicalize(std::env::temp_dir()).unwrap();
    tempfile::Builder::new().prefix("ath-update-").tempdir_in(base).unwrap()
}

fn skill_file(dir: &Path, agent: &str) -> PathBuf {
    dir.join(agent).join("skills").join(SKILL_NAME).join("SKILL.md")
}

fn age_copy(dir: &Path, agent: &str, version: &str) {
    let file = skill_file(dir, agent);
    let text = fs::read_to_string(&file).unwrap();
    let start = text.find("version: \"").unwrap() + "version: \"".len();
    let end = start + text[start..].find('"').unwrap();
    fs::write(&file, format!("{}{}{}", &text[..start], version, &text[end..])).unwrap();
}

fn installed_version(dir: &Path, agent: &str) -> Option<String> {
    skill_version(&dir.join(agent).join("skills").join(SKILL_NAME)).unwrap()
}

fn launched_from(path: &str) -> InstallContext {
    InstallContext { launcher_paths: vec![PathBuf::from(path)], ..Default::default() }
}

fn update(context: &InstallContext, current: &str, cwd: &Path, latest: Result<&str, &str>) -> UpdateOutcome {
    let calls = Cell::new(0);
    let fetch = || {
        calls.set(calls.get() + 1);
        latest.map(str::to_string).map_err(str::to_string)
    };
    let outcome =
        run_update(&UpdateRequest { context, current_version: current, cwd, latest_version: &fetch }).unwrap();
    assert!(calls.get() <= 1);
    outcome
}

fn no_registry() -> Result<&'static str, &'static str> {
    Err("the registry must not be asked")
}

// Classifying install paths.

#[test]
fn recognises_a_global_npm_pnpm_or_bun_install() {
    assert_eq!(classify_install_path(NPM_PATH), InstallKind::Npm);
    assert_eq!(
        classify_install_path("/home/user/.nvm/versions/node/v22.0.0/lib/node_modules/athleticstandard"),
        InstallKind::Npm
    );
    assert_eq!(
        classify_install_path("/home/user/.local/share/pnpm/global/5/node_modules/athleticstandard"),
        InstallKind::Pnpm
    );
    assert_eq!(
        classify_install_path(
            "/Users/me/Library/pnpm/global/5/.pnpm/athleticstandard@0.4.1/node_modules/athleticstandard"
        ),
        InstallKind::Pnpm
    );
    assert_eq!(classify_install_path("/home/user/.bun/install/global/node_modules/athleticstandard"), InstallKind::Bun);
}

#[test]
fn recognises_the_same_layouts_with_backslashes() {
    assert_eq!(
        classify_install_path("C:\\Users\\me\\AppData\\Roaming\\npm\\node_modules\\athleticstandard"),
        InstallKind::Npm
    );
    assert_eq!(
        classify_install_path("C:\\Users\\me\\.bun\\install\\global\\node_modules\\athleticstandard"),
        InstallKind::Bun
    );
}

#[test]
fn a_fetched_copy_is_not_a_global_install() {
    assert_eq!(classify_install_path("/home/user/.npm/_npx/abc/node_modules/athleticstandard"), InstallKind::Npx);
    assert_eq!(
        classify_install_path("/home/user/Library/Caches/pnpm/dlx/abc/node_modules/athleticstandard"),
        InstallKind::PnpmDlx
    );
    assert_eq!(
        classify_install_path("/home/user/.bun/install/cache/abc/node_modules/athleticstandard"),
        InstallKind::Bunx
    );
}

#[test]
fn a_project_copy_is_left_alone() {
    assert_eq!(classify_install_path("/home/user/proj/node_modules/athleticstandard"), InstallKind::Local);
    assert_eq!(
        classify_install_path(
            "/home/user/proj/node_modules/.pnpm/athleticstandard@0.4.1/node_modules/athleticstandard"
        ),
        InstallKind::Local
    );
}

#[test]
fn npm_pnpm_and_bun_are_asked_for_the_latest_package() {
    assert_eq!(PackageManager::Npm.program(), "npm");
    assert_eq!(PackageManager::Npm.args(), ["install", "-g", "athleticstandard@latest"]);
    assert_eq!(PackageManager::Pnpm.program(), "pnpm");
    assert_eq!(PackageManager::Pnpm.args(), ["add", "-g", "athleticstandard@latest"]);
    assert_eq!(PackageManager::Bun.program(), "bun");
    assert_eq!(PackageManager::Bun.args(), ["add", "-g", "athleticstandard@latest"]);
    assert_eq!(InstallKind::Pnpm.manager(), Some(PackageManager::Pnpm));
    assert_eq!(InstallKind::Npx.manager(), None);
}

#[test]
fn prefers_the_global_path_when_the_files_live_in_a_store() {
    let store =
        "/home/user/.local/share/pnpm/store/v10/links/athleticstandard/node_modules/athleticstandard".to_string();
    let global = "/home/user/.local/share/pnpm/global/5/node_modules/athleticstandard".to_string();
    assert_eq!(choose_install(std::slice::from_ref(&store)), InstallKind::Local);
    assert_eq!(choose_install(&[store, global]), InstallKind::Pnpm);
}

#[cfg(unix)]
#[test]
fn follows_a_bin_symlink_to_the_global_package() {
    let dir = tempdir();
    let package = dir.path().join("lib/node_modules/athleticstandard");
    fs::create_dir_all(package.join("dist")).unwrap();
    fs::write(package.join("package.json"), r#"{"name":"athleticstandard"}"#).unwrap();
    fs::write(package.join("dist/cli.js"), "").unwrap();
    fs::create_dir(dir.path().join("bin")).unwrap();
    std::os::unix::fs::symlink(package.join("dist/cli.js"), dir.path().join("bin/ath")).unwrap();
    let paths = candidate_paths(&[dir.path().join("bin/ath")]);
    assert_eq!(choose_install(&paths), InstallKind::Npm);
}

#[test]
fn orders_dotted_versions() {
    assert_eq!(compare_versions("0.4.1", "0.4.2"), -1);
    assert_eq!(compare_versions("0.4.2", "0.4.2"), 0);
    assert_eq!(compare_versions("0.4.3", "0.4.2"), 1);
    assert_eq!(compare_versions("2.10.0", "2.9.9"), 1);
}

#[test]
fn names_the_skill_directories_written() {
    let cwd = Path::new("/w");
    let written = [
        PathBuf::from("/w/.agents/skills/athletic-standard"),
        PathBuf::from("/w/.claude/skills/athletic-standard"),
        PathBuf::from("/w/.cursor/skills/athletic-standard"),
    ];
    assert_eq!(skill_update_line(cwd, &written[..1]), "updated the skill in .agents/skills");
    assert_eq!(
        skill_update_line(cwd, &written),
        "updated the skill in .agents/skills, .claude/skills and .cursor/skills"
    );
    assert_eq!(skill_update_line(cwd, &[]), "");
}

// Running the update.

#[test]
fn does_not_install_when_this_copy_is_not_global_and_does_not_write_a_skill() {
    let dir = tempdir();
    install_skill(dir.path()).unwrap();
    age_copy(dir.path(), ALWAYS_DIR, "0.0.1");
    let outcome =
        update(&launched_from("/home/user/proj/node_modules/athleticstandard"), CURRENT, dir.path(), no_registry());
    assert_eq!(outcome.exit_code, 1);
    assert_eq!(outcome.install, InstallKind::Local);
    assert_eq!(outcome.lines(Stream::Stderr), ["ath: This ath was not installed globally, so nothing was changed."]);
    assert_eq!(installed_version(dir.path(), ALWAYS_DIR).as_deref(), Some("0.0.1"));
}

#[test]
fn says_when_this_ath_is_the_published_version_and_plans_no_install() {
    let dir = tempdir();
    let outcome = update(&launched_from(NPM_PATH), CURRENT, dir.path(), Ok(CURRENT));
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.lines(Stream::Stdout), [format!("ath {CURRENT} is the current version.")]);
    assert_eq!(outcome.plan, None);
    assert!(!dir.path().join(ALWAYS_DIR).exists());
}

#[test]
fn hands_the_install_to_the_launcher_with_the_tool_that_installed_this_copy() {
    let dir = tempdir();
    install_skill(dir.path()).unwrap();
    age_copy(dir.path(), ALWAYS_DIR, "0.0.1");
    let handoff = dir.path().join("handoff").join("plan.json");
    fs::create_dir(handoff.parent().unwrap()).unwrap();
    let context = InstallContext {
        handoff: Some(handoff.clone()),
        ..launched_from("/home/user/.bun/install/global/node_modules/athleticstandard")
    };
    let outcome = update(&context, "0.4.1", dir.path(), Ok("9.9.9"));
    assert_eq!(outcome.exit_code, 0);
    assert!(outcome.messages.is_empty(), "{:?}", outcome.messages);
    let plan = outcome.plan.clone().unwrap();
    assert_eq!(plan.program, "bun");
    assert_eq!(plan.args, ["add", "-g", "athleticstandard@latest"]);
    assert_eq!(plan.success_message, "updated ath from 0.4.1 to 9.9.9.");
    assert_eq!(plan.failure_message, "ath: the install failed.");
    assert_eq!(plan.refresh_args, ["--update"]);
    assert_eq!(plan.refresh_environment.get(PHASE_VARIABLE).map(String::as_str), Some("refresh-skills"));

    let written = fs::read_to_string(&handoff).unwrap();
    assert_eq!(written, plan.to_json());
    assert_eq!(UpdatePlan::from_json(&written).unwrap(), plan);
    // The skill waits for the newly installed program.
    assert_eq!(installed_version(dir.path(), ALWAYS_DIR).as_deref(), Some("0.0.1"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&handoff).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn the_plan_json_has_a_fixed_form() {
    let plan = UpdatePlan::new(PackageManager::Npm, "2.1.1", "2.1.2");
    assert_eq!(
        plan.to_json(),
        r#"{
  "ath_update_plan": 1,
  "manager": "npm",
  "program": "npm",
  "args": [
    "install",
    "-g",
    "athleticstandard@latest"
  ],
  "current_version": "2.1.1",
  "latest_version": "2.1.2",
  "success_message": "updated ath from 2.1.1 to 2.1.2.",
  "failure_message": "ath: the install failed.",
  "refresh_args": [
    "--update"
  ],
  "refresh_environment": {
    "ATH_UPDATE_PHASE": "refresh-skills"
  }
}
"#
    );
}

#[test]
fn a_plan_with_another_program_or_arguments_is_refused() {
    let good = UpdatePlan::new(PackageManager::Pnpm, "2.1.1", "2.1.2");
    assert!(good.check().is_ok());
    for tamper in [
        |plan: &mut UpdatePlan| plan.program = "sh".into(),
        |plan: &mut UpdatePlan| plan.args.push("--ignore-scripts=false".into()),
        |plan: &mut UpdatePlan| plan.args[2] = "athleticstandard@file:../evil".into(),
        |plan: &mut UpdatePlan| plan.ath_update_plan = 2,
        |plan: &mut UpdatePlan| plan.latest_version = "2.1.2; rm -rf /".into(),
        |plan: &mut UpdatePlan| plan.refresh_args = vec!["init".into()],
    ] {
        let mut plan = good.clone();
        tamper(&mut plan);
        assert!(plan.check().is_err(), "{plan:?}");
        assert!(UpdatePlan::from_json(&plan.to_json()).is_err());
    }
    let extra = good.to_json().replacen('{', "{\n  \"shell\": true,", 1);
    assert!(UpdatePlan::from_json(&extra).is_err());
}

#[test]
fn an_existing_plan_file_is_not_overwritten() {
    let dir = tempdir();
    let path = dir.path().join("plan.json");
    fs::write(&path, "theirs").unwrap();
    assert!(write_plan(&path, &UpdatePlan::new(PackageManager::Npm, "2.1.1", "2.1.2")).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "theirs");
}

#[test]
fn without_the_launcher_a_global_install_gets_the_command_instead() {
    let dir = tempdir();
    install_skill(dir.path()).unwrap();
    age_copy(dir.path(), ALWAYS_DIR, "0.0.1");
    let outcome = update(&launched_from(NPM_PATH), "0.4.1", dir.path(), Ok("9.9.9"));
    assert_eq!(outcome.exit_code, 1);
    assert_eq!(outcome.plan, None);
    let message = outcome.lines(Stream::Stderr).join("\n");
    assert!(message.starts_with("ath: This ath was started without its npm launcher, so nothing was changed."));
    assert!(message.contains("npm install -g athleticstandard@latest"));
    assert_eq!(installed_version(dir.path(), ALWAYS_DIR).as_deref(), Some("0.0.1"));
}

#[test]
fn stops_when_the_registry_cannot_be_reached_or_returns_no_version() {
    let dir = tempdir();
    install_skill(dir.path()).unwrap();
    age_copy(dir.path(), ALWAYS_DIR, "0.0.1");
    let offline = update(&launched_from(NPM_PATH), CURRENT, dir.path(), Err("offline"));
    assert_eq!(offline.exit_code, 1);
    assert_eq!(offline.lines(Stream::Stderr), ["ath: the npm registry could not be reached, so nothing was changed."]);
    let odd = update(&launched_from(NPM_PATH), CURRENT, dir.path(), Ok("latest"));
    assert_eq!(odd.lines(Stream::Stderr), ["ath: the npm registry did not return a version, so nothing was changed."]);
    assert_eq!(installed_version(dir.path(), ALWAYS_DIR).as_deref(), Some("0.0.1"));
}

#[test]
fn says_when_this_ath_is_ahead_of_the_published_version() {
    let dir = tempdir();
    let outcome = update(&launched_from(NPM_PATH), "0.4.3", dir.path(), Ok("0.4.2"));
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.lines(Stream::Stdout), ["This ath is 0.4.3, which is ahead of the published 0.4.2."]);
}

#[test]
fn replaces_an_older_tool_skill_when_this_copy_is_current() {
    let dir = tempdir();
    fs::create_dir(dir.path().join(".cursor")).unwrap();
    install_skill(dir.path()).unwrap();
    age_copy(dir.path(), ALWAYS_DIR, "0.0.1");
    age_copy(dir.path(), ".cursor", "0.0.1");
    let shipped = athleticstandard::assets::skill::shipped_skill_version();
    let outcome = update(&launched_from(NPM_PATH), CURRENT, dir.path(), Ok(CURRENT));
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(installed_version(dir.path(), ALWAYS_DIR), Some(shipped.clone()));
    assert_eq!(installed_version(dir.path(), ".cursor"), Some(shipped));
    let stdout = outcome.lines(Stream::Stdout);
    assert_eq!(stdout[0], format!("ath {CURRENT} is the current version."));
    assert_eq!(stdout[1], "updated the skill in .agents/skills and .cursor/skills");
    assert!(stdout[2].starts_with("added mapping file "), "missing mapping files are added too");
}

#[test]
fn reports_unchanged_skills_and_added_mapping_files() {
    let dir = tempdir();
    install_skill(dir.path()).unwrap();
    let outcome = update(&launched_from(NPM_PATH), CURRENT, dir.path(), Ok(CURRENT));
    let stdout = outcome.lines(Stream::Stdout);
    assert_eq!(stdout[1], "left installed skill files unchanged");
    assert_eq!(
        stdout[2],
        format!("added mapping file {}", dir.path().join(ALWAYS_DIR).join("mappings").join("junction.md").display())
    );
    assert_eq!(outcome.mappings_installed.len(), 2);
}

#[test]
fn an_npx_launch_leaves_a_folder_without_a_skill_alone() {
    let dir = tempdir();
    fs::create_dir(dir.path().join(".cursor")).unwrap();
    let outcome = update(
        &launched_from("/home/user/.npm/_npx/abc/node_modules/athleticstandard"),
        CURRENT,
        dir.path(),
        no_registry(),
    );
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.lines(Stream::Stdout), ["This ath was started with npx, which fetches the current version."]);
    assert!(!dir.path().join(ALWAYS_DIR).exists());
    assert!(!dir.path().join(".cursor/skills").exists());
}

#[test]
fn an_npx_launch_replaces_an_older_tool_skill() {
    let dir = tempdir();
    install_skill(dir.path()).unwrap();
    age_copy(dir.path(), ALWAYS_DIR, "0.0.1");
    let outcome = update(
        &launched_from("/home/user/.npm/_npx/abc/node_modules/athleticstandard"),
        CURRENT,
        dir.path(),
        no_registry(),
    );
    assert_eq!(outcome.exit_code, 0);
    assert_ne!(installed_version(dir.path(), ALWAYS_DIR).as_deref(), Some("0.0.1"));
    assert!(outcome.lines(Stream::Stdout).join("\n").contains("updated the skill"));
}

#[test]
fn the_refresh_run_after_an_install_only_refreshes_skills() {
    let dir = tempdir();
    install_skill(dir.path()).unwrap();
    age_copy(dir.path(), ALWAYS_DIR, "0.0.1");
    let context = InstallContext { phase: UpdatePhase::RefreshSkills, ..launched_from("/anywhere/ath") };
    let outcome = update(&context, CURRENT, dir.path(), no_registry());
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.lines(Stream::Stdout)[..1], ["updated the skill in .agents/skills"]);
    assert_ne!(installed_version(dir.path(), ALWAYS_DIR).as_deref(), Some("0.0.1"));

    let empty = tempdir();
    let nothing = update(&context, CURRENT, empty.path(), no_registry());
    assert!(nothing.messages.is_empty());
    assert!(!empty.path().join(ALWAYS_DIR).exists());
}

// Native installs.

fn executable_at(path: &Path) -> InstallContext {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "binary").unwrap();
    InstallContext { executable: Some(path.to_path_buf()), ..Default::default() }
}

#[test]
fn a_source_checkout_refuses_and_never_builds() {
    let dir = tempdir();
    fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"athleticstandard\"\n").unwrap();
    let context = executable_at(&dir.path().join("target/debug/ath"));
    let outcome = update(&context, CURRENT, dir.path(), no_registry());
    assert_eq!((outcome.exit_code, outcome.install), (1, InstallKind::Local));
    assert_eq!(outcome.lines(Stream::Stderr), ["ath: This ath was not installed globally, so nothing was changed."]);
}

#[test]
fn a_cargo_install_gets_the_cargo_command_for_its_source() {
    let dir = tempdir();
    let root = dir.path().join("cargo home");
    let context = executable_at(&root.join("bin/ath"));
    fs::write(
        root.join(".crates2.json"),
        r#"{"installs":{
            "other 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)":{"bins":["ath"]},
            "athleticstandard 2.1.1 (git+https://github.com/mwhitham/ath?branch=main#0123456789abcdef)":{"bins":["ath"]}
        }}"#,
    )
    .unwrap();
    let outcome = update(&context, CURRENT, dir.path(), no_registry());
    assert_eq!((outcome.exit_code, outcome.install), (1, InstallKind::Cargo));
    assert_eq!(
        outcome.lines(Stream::Stderr),
        ["ath: This ath was installed with Cargo, so nothing was changed.\nTo update it, run:\n  \
          cargo install --locked --git https://github.com/mwhitham/ath --branch main athleticstandard\n\
          Then run `ath skill install` in each folder that has the agent skill."]
    );

    let path_source = r#"{"installs":{"athleticstandard 2.1.1 (path+file:///src/my%20ath/native)":{"bins":["ath"]}}}"#;
    fs::write(root.join(".crates2.json"), path_source).unwrap();
    let from_path = update(&context, CURRENT, dir.path(), no_registry());
    let text = from_path.lines(Stream::Stderr).join("\n");
    assert!(text.contains(
        "update the source at /src/my ath/native, then run:\n  cargo install --locked --path /src/my ath/native"
    ));
}

#[test]
fn a_release_archive_gets_download_instructions_and_is_not_mistaken_for_npm() {
    let dir = tempdir();
    let executable = dir.path().join("ath-2.1.1").join("ath");
    let outcome = update(&executable_at(&executable), CURRENT, dir.path(), no_registry());
    assert_eq!((outcome.exit_code, outcome.install), (1, InstallKind::NativeArchive));
    let text = outcome.lines(Stream::Stderr).join("\n");
    assert!(text.starts_with("ath: This ath was installed from a release archive, so nothing was changed."));
    assert!(text.contains(BUILD_TARGET));
    assert!(text.contains("https://github.com/mwhitham/ath/releases/latest"));
    assert!(text.contains(&executable.display().to_string()));
    assert!(!dir.path().join(ALWAYS_DIR).exists());
}

#[test]
fn the_executable_inside_a_global_npm_package_counts_as_npm() {
    let dir = tempdir();
    let executable = dir.path().join("lib/node_modules/athleticstandard/native/ath");
    let outcome = update(&executable_at(&executable), "0.4.1", dir.path(), Ok("9.9.9"));
    assert_eq!(outcome.install, InstallKind::Npm);
    assert!(outcome.lines(Stream::Stderr)[0].contains("npm install -g athleticstandard@latest"));

    let project = dir.path().join("project/node_modules/@athleticstandard/native/ath");
    let local = update(&executable_at(&project), "0.4.1", dir.path(), no_registry());
    assert_eq!(local.install, InstallKind::Local);
}

#[test]
fn the_launcher_variables_set_the_context() {
    let paths = std::env::join_paths([NPM_PATH, "/usr/local/bin/ath"]).unwrap();
    let env: HashMap<String, String> = HashMap::from([
        (INSTALL_PATHS_VARIABLE.to_string(), paths.to_string_lossy().into_owned()),
        (HANDOFF_VARIABLE.to_string(), "/private/handoff/plan.json".to_string()),
        (PHASE_VARIABLE.to_string(), "refresh-skills".to_string()),
    ]);
    let context = InstallContext::from_environment(&env);
    assert_eq!(context.launcher_paths, [PathBuf::from(NPM_PATH), PathBuf::from("/usr/local/bin/ath")]);
    assert_eq!(context.handoff, Some(PathBuf::from("/private/handoff/plan.json")));
    assert_eq!(context.phase, UpdatePhase::RefreshSkills);
    assert!(context.executable.is_some());

    let plain = InstallContext::from_environment(&HashMap::<String, String>::new());
    assert!(plain.launcher_paths.is_empty());
    assert_eq!((plain.handoff, plain.phase), (None, UpdatePhase::Check));
}

#[test]
fn the_latest_version_comes_from_the_registry_document() {
    let server = Server::start(vec![
        Reply::json(r#"{"name":"athleticstandard","version":"9.9.9"}"#),
        Reply::json(r#"{"name":"athleticstandard"}"#),
        Reply::status(503, "Service Unavailable", "{}"),
    ]);
    let url = format!("{}/athleticstandard/latest", server.origin());
    assert_eq!(fetch_latest_version(&url).unwrap(), "9.9.9");
    assert!(fetch_latest_version(&url).is_err());
    assert!(fetch_latest_version(&url).is_err());
    let received = server.finish();
    assert_eq!(received[0].path, "/athleticstandard/latest");
    assert_eq!(received[0].headers["accept"], "application/json");
}
