//! `ath --update` (D81).
//!
//! The program and the skill are different things. The program is installed
//! once, with npm, pnpm, bun, Cargo, or a release archive. The skill is written
//! only into folders where `ath init` or `ath skill install` ran. This command
//! updates a global npm, pnpm, or bun install with the tool that installed it.
//! Then it replaces an older tool skill and adds missing skill files, but only
//! where a skill copy is already present.
//!
//! # How the install is found
//!
//! The npm launcher tells this program where it was started from. Each path is
//! classified with the directory markers npm, pnpm, and bun use. A path that
//! matches none of them is never treated as an npm install: guessing npm would
//! install a second copy. Without the launcher, the executable's own path is
//! classified. A Cargo install and a release archive get exact update
//! instructions. A source checkout never starts a build.
//!
//! # The launcher handoff
//!
//! The running executable must exit before a package manager can replace it on
//! Windows. The same handoff is used on every system, so there is one tested
//! route. When an install is needed, this program writes an [`UpdatePlan`] to
//! the file the launcher named and exits. The launcher then:
//!
//! 1. reads the plan and checks it: `ath_update_plan` is 1, `program` is the
//!    `manager`, and `args` is exactly that manager's argument list;
//! 2. runs `program` with `args`, without a shell command string, with the
//!    terminal attached;
//! 3. on failure or cancellation, prints `failure_message` on standard error
//!    and exits 1;
//! 4. on success, prints `success_message`, then runs the newly installed
//!    `ath` with `refresh_args` and `refresh_environment` added to its
//!    environment, and exits with that status.
//!
//! The refresh run uses the new program's own embedded skill, so the copied
//! skill matches the program that was just installed.
//!
//! # Environment variables
//!
//! - `ATH_INSTALL_PATHS`: set by the npm launcher. The launcher script path as
//!   started (`process.argv[1]`) and the launcher module's own file path,
//!   joined with the system path-list separator (`:`, or `;` on Windows).
//! - `ATH_UPDATE_HANDOFF`: set by the npm launcher. A file path in a private
//!   directory the launcher created. The file must not exist yet. The plan is
//!   written there when an install is needed; otherwise nothing is written.
//! - `ATH_UPDATE_PHASE`: set to `refresh-skills` by the launcher for the run
//!   after a successful install. That run only refreshes skills.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::Environment;
use crate::assets::skill;
use crate::error::{Error, Result};

/// The npm package name.
pub const PACKAGE_NAME: &str = "athleticstandard";

/// Where the npm registry reports the latest published version.
pub const REGISTRY_LATEST_URL: &str = "https://registry.npmjs.org/athleticstandard/latest";

/// Where release archives are published.
pub const RELEASES_URL: &str = "https://github.com/mwhitham/ath/releases/latest";

/// The source repository, for Cargo installs from Git.
pub const SOURCE_REPOSITORY: &str = "https://github.com/mwhitham/ath";

/// Launcher and executable paths to classify.
pub const INSTALL_PATHS_VARIABLE: &str = "ATH_INSTALL_PATHS";

/// The file the launcher reads an update plan from.
pub const HANDOFF_VARIABLE: &str = "ATH_UPDATE_HANDOFF";

/// Which part of the update this run performs.
pub const PHASE_VARIABLE: &str = "ATH_UPDATE_PHASE";

/// The `ATH_UPDATE_PHASE` value for the run after a successful install.
pub const REFRESH_PHASE: &str = "refresh-skills";

/// The update plan format this program writes.
pub const PLAN_FORMAT: u32 = 1;

/// The release target this program was built for, such as `aarch64-apple-darwin`.
pub const BUILD_TARGET: &str = env!("ATH_BUILD_TARGET");

const LOCAL_MESSAGE: &str = "This ath was not installed globally, so nothing was changed.";

/// A package manager that installs a global `ath`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Bun,
}

impl PackageManager {
    /// The program to run.
    pub fn program(self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Bun => "bun",
        }
    }

    /// The exact arguments that install the latest published `ath` globally.
    pub fn args(self) -> [&'static str; 3] {
        match self {
            PackageManager::Npm => ["install", "-g", "athleticstandard@latest"],
            PackageManager::Pnpm | PackageManager::Bun => ["add", "-g", "athleticstandard@latest"],
        }
    }
}

/// How this `ath` was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallKind {
    /// A global npm install.
    Npm,
    /// A global pnpm install.
    Pnpm,
    /// A global bun install.
    Bun,
    /// A temporary `npx` run, which fetches the current version.
    Npx,
    /// A temporary `pnpm dlx` run.
    PnpmDlx,
    /// A temporary `bunx` run.
    Bunx,
    /// A project copy, a source checkout, or an unknown location.
    Local,
    /// `cargo install`.
    Cargo,
    /// An executable extracted from a release archive.
    NativeArchive,
}

impl InstallKind {
    /// The package manager that updates this install, for global installs.
    pub fn manager(self) -> Option<PackageManager> {
        match self {
            InstallKind::Npm => Some(PackageManager::Npm),
            InstallKind::Pnpm => Some(PackageManager::Pnpm),
            InstallKind::Bun => Some(PackageManager::Bun),
            _ => None,
        }
    }

    /// The note for a temporary run.
    pub fn fetched_message(self) -> Option<&'static str> {
        match self {
            InstallKind::Npx => Some("This ath was started with npx, which fetches the current version."),
            InstallKind::PnpmDlx => Some("This ath was started with pnpm dlx, which fetches the current version."),
            InstallKind::Bunx => Some("This ath was started with bunx, which fetches the current version."),
            _ => None,
        }
    }

    /// A global install outranks a fetched copy, which outranks a project copy.
    fn rank(self) -> u8 {
        match self {
            InstallKind::Npm | InstallKind::Pnpm | InstallKind::Bun => 3,
            InstallKind::Npx | InstallKind::PnpmDlx | InstallKind::Bunx => 2,
            _ => 1,
        }
    }
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

/// Classify one path with the directories npm, pnpm, and bun use. A path that
/// matches none of them is [`InstallKind::Local`].
pub fn classify_install_path(package_path: &str) -> InstallKind {
    let path = normalize(package_path);
    if path.contains("/_npx/") {
        return InstallKind::Npx;
    }
    if path.contains("/pnpm/dlx/") || path.contains("/pnpm-dlx/") {
        return InstallKind::PnpmDlx;
    }
    if path.contains("/bunx/") || path.contains("/.bun/install/cache/") {
        return InstallKind::Bunx;
    }
    if path.contains("/pnpm/global/") {
        return InstallKind::Pnpm;
    }
    if path.contains("/.bun/install/global/") {
        return InstallKind::Bun;
    }
    if path.ends_with("/lib/node_modules/athleticstandard")
        || path.contains("/lib/node_modules/athleticstandard/")
        || path.ends_with("/npm/node_modules/athleticstandard")
        || path.contains("/npm/node_modules/athleticstandard/")
    {
        return InstallKind::Npm;
    }
    InstallKind::Local
}

/// Pick one classification from every path that might be this process.
///
/// pnpm and bun keep the real files in a store and leave a link where the
/// command was typed. The store path does not say "global"; the link does.
pub fn choose_install(paths: &[String]) -> InstallKind {
    let mut best = InstallKind::Local;
    for path in paths.iter().filter(|path| !path.is_empty()) {
        let found = classify_install_path(path);
        if found.rank() > best.rank() {
            best = found;
        }
    }
    best
}

/// Walk up to the `athleticstandard` package that holds `start`, if any.
pub fn find_package_root(start: &Path) -> Option<PathBuf> {
    let mut directory = start.to_path_buf();
    for _ in 0..12 {
        let manifest = directory.join("package.json");
        if manifest.exists() {
            let named = fs::read(&manifest)
                .ok()
                .and_then(|bytes| crate::json::parse_bytes(&bytes).ok())
                .is_some_and(|value| value.get("name").and_then(|name| name.as_str()) == Some(PACKAGE_NAME));
            if named {
                return Some(directory);
            }
        }
        let parent = directory.parent()?.to_path_buf();
        if parent == directory || parent.as_os_str().is_empty() {
            return None;
        }
        directory = parent;
    }
    None
}

/// Every path worth classifying, starting from the given ones: each path,
/// where it points if it is a link, its real path, and the package directory
/// that holds it. A bin link such as `/usr/local/bin/ath` does not say how it
/// was installed; its target does.
pub fn candidate_paths(starts: &[PathBuf]) -> Vec<String> {
    let mut out = Vec::new();
    let mut queue: std::collections::VecDeque<PathBuf> = std::collections::VecDeque::new();
    let push = |path: PathBuf, out: &mut Vec<String>, queue: &mut std::collections::VecDeque<PathBuf>| {
        if path.as_os_str().is_empty() {
            return;
        }
        let normal = normalize(&path.to_string_lossy());
        if out.contains(&normal) {
            return;
        }
        out.push(normal);
        queue.push_back(path);
    };
    for start in starts {
        push(start.clone(), &mut out, &mut queue);
    }
    while let Some(path) = queue.pop_front() {
        if let Ok(target) = fs::read_link(&path) {
            let base = path.parent().unwrap_or(Path::new(""));
            push(crate::assets::resolve_lexically(base, &target), &mut out, &mut queue);
        }
        if let Ok(real) = fs::canonicalize(&path) {
            push(real, &mut out, &mut queue);
        }
        if let Some(root) = find_package_root(&path) {
            push(root, &mut out, &mut queue);
        }
    }
    out
}

/// Compare dotted versions part by part.
pub fn compare_versions(left: &str, right: &str) -> i32 {
    let parse = |text: &str| -> Vec<f64> { text.split('.').map(skill::js_number).collect() };
    let a = parse(left);
    let b = parse(right);
    for index in 0..3 {
        let diff = a.get(index).copied().unwrap_or(0.0) - b.get(index).copied().unwrap_or(0.0);
        if diff != 0.0 {
            return if diff > 0.0 { 1 } else { -1 };
        }
    }
    0
}

/// One line naming the skill directories that were written.
pub fn skill_update_line(cwd: &Path, written: &[PathBuf]) -> String {
    let cwd_text = cwd.to_string_lossy();
    let directories: Vec<String> = written
        .iter()
        .map(|path| {
            let text = path.to_string_lossy();
            let relative = text.get(cwd_text.len()..).unwrap_or("").trim_start_matches(['/', '\\']);
            let agent = relative.split(['/', '\\']).next().unwrap_or(relative);
            Path::new(agent).join("skills").display().to_string()
        })
        .collect();
    match directories.as_slice() {
        [] => String::new(),
        [only] => format!("updated the skill in {only}"),
        [rest @ .., last] => format!("updated the skill in {} and {last}", rest.join(", ")),
    }
}

/// Which part of the update this run performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdatePhase {
    /// Classify the install, check the registry, and plan an install.
    #[default]
    Check,
    /// The run after a successful install: refresh skills only.
    RefreshSkills,
}

/// Where this process came from, as the launcher and the system report it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallContext {
    /// Paths from `ATH_INSTALL_PATHS`. Empty when no npm launcher started this run.
    pub launcher_paths: Vec<PathBuf>,
    /// This executable.
    pub executable: Option<PathBuf>,
    /// The plan file from `ATH_UPDATE_HANDOFF`.
    pub handoff: Option<PathBuf>,
    pub phase: UpdatePhase,
}

impl InstallContext {
    /// Read the launcher's variables and this executable's path.
    pub fn from_environment(env: &dyn Environment) -> Self {
        let launcher_paths = env
            .var(INSTALL_PATHS_VARIABLE)
            .map(|value| std::env::split_paths(&value).filter(|path| !path.as_os_str().is_empty()).collect())
            .unwrap_or_default();
        let handoff = env.var(HANDOFF_VARIABLE).filter(|value| !value.is_empty()).map(PathBuf::from);
        let phase = if env.var(PHASE_VARIABLE).as_deref() == Some(REFRESH_PHASE) {
            UpdatePhase::RefreshSkills
        } else {
            UpdatePhase::Check
        };
        InstallContext { launcher_paths, executable: std::env::current_exe().ok(), handoff, phase }
    }
}

/// Where a Cargo install came from, read from Cargo's install record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum CargoSource {
    Git { url: String, branch: Option<String>, tag: Option<String> },
    Path { path: PathBuf },
    Registry,
    Unknown,
}

/// The classified install, with what is needed to describe its update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    pub kind: InstallKind,
    pub executable: Option<PathBuf>,
    pub cargo_source: Option<CargoSource>,
}

/// Classify the install for `context`.
pub fn classify_installation(context: &InstallContext) -> Installation {
    let mut starts = context.launcher_paths.clone();
    starts.extend(context.executable.iter().cloned());
    let kind = choose_install(&candidate_paths(&starts));
    let local = Installation { kind: InstallKind::Local, executable: context.executable.clone(), cargo_source: None };
    if kind != InstallKind::Local {
        return Installation { kind, ..local };
    }
    if !context.launcher_paths.is_empty() {
        return local;
    }
    let Some(executable) = &context.executable else {
        return local;
    };
    let real = fs::canonicalize(executable).unwrap_or_else(|_| executable.clone());
    if normalize(&real.to_string_lossy()).contains("/node_modules/") || in_source_checkout(&real) {
        return local;
    }
    if let Some(source) = cargo_install_source(&real) {
        return Installation { kind: InstallKind::Cargo, executable: Some(real), cargo_source: Some(source) };
    }
    Installation { kind: InstallKind::NativeArchive, executable: Some(real), cargo_source: None }
}

/// True when the executable sits in a Cargo build directory of a source tree.
fn in_source_checkout(executable: &Path) -> bool {
    executable.ancestors().skip(1).any(|ancestor| {
        ancestor.file_name().is_some_and(|name| name == "target")
            && ancestor.parent().is_some_and(|parent| parent.join("Cargo.toml").is_file())
    })
}

/// The source of a `cargo install` that put this executable in `<root>/bin`.
fn cargo_install_source(executable: &Path) -> Option<CargoSource> {
    let bin = executable.parent()?;
    if bin.file_name()? != "bin" {
        return None;
    }
    let root = bin.parent()?;
    let record = root.join(".crates2.json");
    if !record.is_file() && !root.join(".crates.toml").is_file() {
        return None;
    }
    let executable_name = executable.file_name()?.to_string_lossy().into_owned();
    let Some(value) = fs::read(&record).ok().and_then(|bytes| crate::json::parse_bytes(&bytes).ok()) else {
        return Some(CargoSource::Unknown);
    };
    let Some(installs) = value.get("installs").and_then(|installs| installs.as_object()) else {
        return Some(CargoSource::Unknown);
    };
    for (key, install) in installs {
        let Some(rest) = key.strip_prefix(&format!("{PACKAGE_NAME} ")) else { continue };
        let names_this = install
            .get("bins")
            .and_then(|bins| bins.as_array())
            .is_some_and(|bins| bins.iter().any(|bin| bin.as_str() == Some(executable_name.as_str())));
        if !names_this {
            continue;
        }
        let source = rest.rsplit_once(" (").and_then(|(_, source)| source.strip_suffix(')')).unwrap_or("");
        return Some(parse_cargo_source(source));
    }
    Some(CargoSource::Unknown)
}

fn parse_cargo_source(source: &str) -> CargoSource {
    if let Some(git) = source.strip_prefix("git+") {
        let without_fragment = git.split('#').next().unwrap_or(git);
        let (url, query) = match without_fragment.split_once('?') {
            Some((url, query)) => (url, query),
            None => (without_fragment, ""),
        };
        let mut branch = None;
        let mut tag = None;
        for pair in query.split('&') {
            match pair.split_once('=') {
                Some(("branch", value)) => branch = Some(percent_decode(value)),
                Some(("tag", value)) => tag = Some(percent_decode(value)),
                _ => {}
            }
        }
        return CargoSource::Git { url: url.to_string(), branch, tag };
    }
    if let Some(path) = source.strip_prefix("path+file://") {
        let decoded = percent_decode(path);
        let bytes = decoded.as_bytes();
        // `file:///C:/...` names a Windows drive path.
        let local = if bytes.len() > 2 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
            decoded[1..].to_string()
        } else {
            decoded
        };
        return CargoSource::Path { path: PathBuf::from(local) };
    }
    if source.starts_with("registry+") || source.starts_with("sparse+") {
        return CargoSource::Registry;
    }
    CargoSource::Unknown
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && bytes[index + 1].is_ascii_hexdigit()
            && bytes[index + 2].is_ascii_hexdigit()
        {
            let hex = |byte: u8| (byte as char).to_digit(16).unwrap_or(0) as u8;
            out.push(hex(bytes[index + 1]) * 16 + hex(bytes[index + 2]));
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The `cargo install` command that updates a Cargo install from `source`.
pub fn cargo_update_command(source: &CargoSource) -> Vec<String> {
    let mut command: Vec<String> = vec!["cargo".into(), "install".into(), "--locked".into()];
    match source {
        CargoSource::Git { url, branch, tag } => {
            command.extend(["--git".into(), url.clone()]);
            if let Some(branch) = branch {
                command.extend(["--branch".into(), branch.clone()]);
            }
            if let Some(tag) = tag {
                command.extend(["--tag".into(), tag.clone()]);
            }
            command.push(PACKAGE_NAME.into());
        }
        CargoSource::Path { path } => command.extend(["--path".into(), path.display().to_string()]),
        CargoSource::Registry => command.push(PACKAGE_NAME.into()),
        CargoSource::Unknown => command.extend(["--git".into(), SOURCE_REPOSITORY.into(), PACKAGE_NAME.into()]),
    }
    command
}

const SKILL_INSTALL_NOTE: &str = "Then run `ath skill install` in each folder that has the agent skill.";

fn cargo_instructions(source: &CargoSource) -> String {
    let command = cargo_update_command(source).join(" ");
    let first = match source {
        CargoSource::Path { path } => {
            format!("To update it, update the source at {}, then run:", path.display())
        }
        _ => "To update it, run:".to_string(),
    };
    format!(
        "ath: This ath was installed with Cargo, so nothing was changed.\n{first}\n  {command}\n{SKILL_INSTALL_NOTE}"
    )
}

fn archive_instructions(executable: Option<&Path>) -> String {
    let place = executable.map(|path| path.display().to_string()).unwrap_or_else(|| "this ath".to_string());
    format!(
        "ath: This ath was installed from a release archive, so nothing was changed.\n\
         To update it, download the newest archive for {BUILD_TARGET} from\n  {RELEASES_URL}\n\
         and replace {place} with the ath inside it.\n{SKILL_INSTALL_NOTE}"
    )
}

/// What the launcher runs after this process exits. See the module notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePlan {
    /// The plan format, [`PLAN_FORMAT`].
    pub ath_update_plan: u32,
    pub manager: PackageManager,
    /// The program to run: the manager's own command name.
    pub program: String,
    /// The exact arguments, one per element. Never joined into a shell string.
    pub args: Vec<String>,
    pub current_version: String,
    pub latest_version: String,
    /// Printed on standard output after the install succeeds.
    pub success_message: String,
    /// Printed on standard error when the install fails or is cancelled.
    pub failure_message: String,
    /// Arguments for the newly installed `ath`.
    pub refresh_args: Vec<String>,
    /// Environment variables added for the newly installed `ath`.
    pub refresh_environment: BTreeMap<String, String>,
}

impl UpdatePlan {
    /// The plan that installs `latest` over `current` with `manager`.
    pub fn new(manager: PackageManager, current: &str, latest: &str) -> Self {
        UpdatePlan {
            ath_update_plan: PLAN_FORMAT,
            manager,
            program: manager.program().to_string(),
            args: manager.args().iter().map(|arg| arg.to_string()).collect(),
            current_version: current.to_string(),
            latest_version: latest.to_string(),
            success_message: format!("updated ath from {current} to {latest}."),
            failure_message: "ath: the install failed.".to_string(),
            refresh_args: vec!["--update".to_string()],
            refresh_environment: BTreeMap::from([(PHASE_VARIABLE.to_string(), REFRESH_PHASE.to_string())]),
        }
    }

    /// Refuse a plan whose program or arguments are not the fixed ones.
    pub fn check(&self) -> Result<()> {
        let expected = UpdatePlan::new(self.manager, &self.current_version, &self.latest_version);
        let versions_ok =
            [&self.current_version, &self.latest_version].iter().all(|version| is_release_version(version));
        if self.ath_update_plan != PLAN_FORMAT
            || self.program != expected.program
            || self.args != expected.args
            || self.refresh_args != expected.refresh_args
            || self.refresh_environment != expected.refresh_environment
            || !versions_ok
        {
            return Err(Error::refused("the update plan is not one this ath writes; nothing was installed"));
        }
        Ok(())
    }

    /// The JSON form: two-space indentation and a final newline.
    pub fn to_json(&self) -> String {
        let value = serde_json::to_value(self).expect("an update plan is plain data");
        let mut text = crate::json::to_pretty(&value);
        text.push('\n');
        text
    }

    /// Read and check a plan.
    pub fn from_json(text: &str) -> Result<UpdatePlan> {
        let plan: UpdatePlan = serde_json::from_str(text)
            .map_err(|error| Error::refused(format!("the update plan is not valid: {error}")))?;
        plan.check()?;
        Ok(plan)
    }
}

/// `/^\d+\.\d+\.\d+$/`.
fn is_release_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Which output stream a message belongs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
}

/// One line or block of text to print, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateMessage {
    pub stream: Stream,
    pub text: String,
}

/// The result of `ath --update`. The command prints `messages` in order and
/// exits with `exit_code`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateOutcome {
    pub install: InstallKind,
    pub exit_code: i32,
    pub messages: Vec<UpdateMessage>,
    /// The plan written for the launcher, when an install is needed.
    pub plan: Option<UpdatePlan>,
    pub latest_version: Option<String>,
    pub skill_installed: Vec<PathBuf>,
    pub mappings_installed: Vec<PathBuf>,
}

impl UpdateOutcome {
    fn new(install: InstallKind) -> Self {
        UpdateOutcome {
            install,
            exit_code: 0,
            messages: Vec::new(),
            plan: None,
            latest_version: None,
            skill_installed: Vec::new(),
            mappings_installed: Vec::new(),
        }
    }

    fn out(&mut self, text: impl Into<String>) {
        self.messages.push(UpdateMessage { stream: Stream::Stdout, text: text.into() });
    }

    fn refuse(mut self, text: impl Into<String>) -> Self {
        self.messages.push(UpdateMessage { stream: Stream::Stderr, text: text.into() });
        self.exit_code = 1;
        self
    }

    /// The lines for one stream, for tests and simple callers.
    pub fn lines(&self, stream: Stream) -> Vec<&str> {
        self.messages.iter().filter(|message| message.stream == stream).map(|message| message.text.as_str()).collect()
    }
}

/// The inputs to `ath --update`.
pub struct UpdateRequest<'a> {
    pub context: &'a InstallContext,
    pub current_version: &'a str,
    pub cwd: &'a Path,
    /// Reads the latest published version. Called only for global installs.
    pub latest_version: &'a dyn Fn() -> std::result::Result<String, String>,
}

/// `ath --update`. Never prints; the command prints the outcome's messages.
pub fn run_update(request: &UpdateRequest<'_>) -> Result<UpdateOutcome> {
    if request.context.phase == UpdatePhase::RefreshSkills {
        let mut outcome = UpdateOutcome::new(InstallKind::Local);
        refresh_skills(request.cwd, &mut outcome)?;
        return Ok(outcome);
    }

    let installation = classify_installation(request.context);
    let mut outcome = UpdateOutcome::new(installation.kind);
    match installation.kind {
        InstallKind::Local => return Ok(outcome.refuse(format!("ath: {LOCAL_MESSAGE}"))),
        InstallKind::Cargo => {
            let source = installation.cargo_source.unwrap_or(CargoSource::Unknown);
            return Ok(outcome.refuse(cargo_instructions(&source)));
        }
        InstallKind::NativeArchive => {
            return Ok(outcome.refuse(archive_instructions(installation.executable.as_deref())))
        }
        InstallKind::Npx | InstallKind::PnpmDlx | InstallKind::Bunx => {
            outcome.out(installation.kind.fetched_message().unwrap_or_default());
        }
        InstallKind::Npm | InstallKind::Pnpm | InstallKind::Bun => {
            let manager = installation.kind.manager().expect("a global install has a manager");
            let Ok(latest) = (request.latest_version)() else {
                return Ok(outcome.refuse("ath: the npm registry could not be reached, so nothing was changed."));
            };
            if !is_release_version(&latest) {
                return Ok(outcome.refuse("ath: the npm registry did not return a version, so nothing was changed."));
            }
            outcome.latest_version = Some(latest.clone());
            let current = request.current_version;
            match compare_versions(current, &latest) {
                0 => outcome.out(format!("ath {current} is the current version.")),
                order if order > 0 => {
                    outcome.out(format!("This ath is {current}, which is ahead of the published {latest}."))
                }
                _ => {
                    let plan = UpdatePlan::new(manager, current, &latest);
                    let Some(handoff) = &request.context.handoff else {
                        let command = std::iter::once(plan.program.as_str())
                            .chain(plan.args.iter().map(String::as_str))
                            .collect::<Vec<_>>()
                            .join(" ");
                        return Ok(outcome.refuse(format!(
                            "ath: This ath was started without its npm launcher, so nothing was changed.\n\
                             To update it, run:\n  {command}\n{SKILL_INSTALL_NOTE}"
                        )));
                    };
                    write_plan(handoff, &plan)?;
                    // The newly installed ath refreshes the skill after the install.
                    outcome.plan = Some(plan);
                    return Ok(outcome);
                }
            }
        }
    }
    refresh_skills(request.cwd, &mut outcome)?;
    Ok(outcome)
}

/// Write the plan to a new private file. An existing file is refused.
pub fn write_plan(path: &Path, plan: &UpdatePlan) -> Result<()> {
    crate::files::write_new_synced(path, plan.to_json().as_bytes())?;
    Ok(())
}

/// Replace an older tool skill and add missing skill and mapping files, but
/// only where a skill copy is already present (D81).
fn refresh_skills(cwd: &Path, outcome: &mut UpdateOutcome) -> Result<()> {
    if !skill::skill_present(cwd)? {
        return Ok(());
    }
    let written = skill::install_skill(cwd)?;
    let line = skill_update_line(cwd, &written);
    outcome.out(if line.is_empty() { "left installed skill files unchanged".to_string() } else { line });
    let mappings = skill::install_mappings(cwd)?;
    for path in &mappings {
        outcome.out(format!("added mapping file {}", path.display()));
    }
    outcome.skill_installed = written;
    outcome.mappings_installed = mappings;
    Ok(())
}

/// Ask the npm registry for the latest published version. This is the only
/// network call `ath --update` makes.
pub fn fetch_latest_version(url: &str) -> std::result::Result<String, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
    let response =
        agent.get(url).set("accept", "application/json").call().map_err(|error| format!("registry status {error}"))?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(response.into_reader(), 1024 * 1024), &mut bytes)
        .map_err(|error| error.to_string())?;
    let value = crate::json::parse_bytes(&bytes).map_err(|error| error.message)?;
    match value.get("version") {
        Some(crate::json::Value::String(version)) => Ok(version.clone()),
        _ => Err("registry version".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_sources_name_their_update_command() {
        let git = parse_cargo_source("git+https://github.com/mwhitham/ath?branch=main#0123abcd");
        assert_eq!(
            cargo_update_command(&git).join(" "),
            "cargo install --locked --git https://github.com/mwhitham/ath --branch main athleticstandard"
        );
        let path = parse_cargo_source("path+file:///home/me/my%20ath/native");
        assert_eq!(path, CargoSource::Path { path: PathBuf::from("/home/me/my ath/native") });
        assert_eq!(
            parse_cargo_source("path+file:///C:/src/ath"),
            CargoSource::Path { path: PathBuf::from("C:/src/ath") }
        );
        assert_eq!(parse_cargo_source("registry+https://github.com/rust-lang/crates.io-index"), CargoSource::Registry);
        assert_eq!(
            cargo_update_command(&CargoSource::Unknown).join(" "),
            "cargo install --locked --git https://github.com/mwhitham/ath athleticstandard"
        );
    }

    #[test]
    fn release_versions_are_three_numbers() {
        assert!(is_release_version("2.1.1"));
        assert!(!is_release_version("2.1"));
        assert!(!is_release_version("2.1.1-beta"));
        assert!(!is_release_version("v2.1.1"));
    }

    #[test]
    fn percent_decoding_leaves_bad_escapes() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
    }
}
