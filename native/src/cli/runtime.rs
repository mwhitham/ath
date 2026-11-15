//! What every command shares: where output goes, how the database is found
//! and opened, how a person confirms a write, and how failures are reported.
//!
//! This ports `src/cli/runtime.ts` and `src/file.ts` from 2.1.0. Rule IDs such
//! as S14 refer to `build-history/v2.1.1/appendix/command-behavior.md`.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use crate::error::Error;
use crate::files;
use crate::json::{Map, Value};
use crate::runtime::gateway::GatewayEndpoints;
use crate::runtime::keyring::{KeyRefusal, OsSecretStore, SecretStore};
use crate::store::{MigrationPreview, OpenOptions, Store};

use crate::text::js_trim;

/// The live database file suffix.
pub const FILE_SUFFIX: &str = ".ath.sqlite";
/// The only file name found without `--file` (S10).
pub const DEFAULT_FILENAME: &str = "athlete.ath.sqlite";
/// The prerelease JSON file suffix, refused as a live record.
pub const PORTABLE_FILE_SUFFIX: &str = ".ath.json";
/// The program version `--version` prints.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The non-interactive refusal for a write that needs a confirmation (S18).
pub const NO_TERMINAL_TO_CONFIRM: &str =
    "there is no terminal to ask on, so nothing was written. Pass --yes after reviewing the preview.";

/// Which standard streams are terminals, and their widths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Terminal {
    pub stdin: bool,
    pub stdout: bool,
    pub stderr: bool,
    /// The width of standard output when it is a terminal.
    pub stdout_columns: Option<usize>,
    /// The width of standard error when it is a terminal.
    pub stderr_columns: Option<usize>,
}

/// Opens the controlling terminal for reading answers (S19). Returns `None`
/// when the process has no controlling terminal.
pub type OpenTerminal<'a> = dyn FnMut() -> Option<Box<dyn BufRead>> + 'a;

/// The process surroundings a command reads and writes. Tests build one with
/// in-memory streams; `main` builds one from the real process.
pub struct Io<'a> {
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    pub stdin: &'a mut dyn BufRead,
    pub terminal: Terminal,
    pub open_terminal: &'a mut OpenTerminal<'a>,
    /// The working directory. Relative paths resolve against it.
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
}

/// Applies a confirmed SQL migration to a store opened with
/// `allow_outdated_schema`. It makes and verifies the backup at the
/// preview's `planned_backup_path`, migrates, leaves the store's metadata
/// current, and returns the verified backup path.
pub type MigrationExecutor = fn(&mut Store, &MigrationPreview) -> crate::Result<PathBuf>;

/// Library operations and optional runtime services the command shell calls
/// through a seam, so tests can replace them. Local record commands never
/// construct a password store, a network client, or a package-manager plan.
#[derive(Clone)]
pub struct Services {
    pub migrate: MigrationExecutor,
    /// The password store `key`, `models`, `predict`, and `backtest` read.
    /// `None` means this system's store, built only when a key is needed.
    pub secret_store: Option<Rc<dyn SecretStore>>,
    /// The gateway API roots. `None` means the published roots, or the
    /// loopback root named by [`GATEWAY_BASE_URL_VARIABLE`].
    pub gateway_endpoints: Option<GatewayEndpoints>,
    /// Hidden input for `ath key set`.
    pub prompt_secret: fn(&str) -> Result<String, KeyRefusal>,
    /// The latest published version, for `ath --update`.
    pub latest_version: fn() -> Result<String, String>,
}

impl Default for Services {
    fn default() -> Self {
        Services {
            migrate: confirmed_migration,
            secret_store: None,
            gateway_endpoints: None,
            prompt_secret: crate::runtime::keyring::prompt_secret,
            latest_version: published_version,
        }
    }
}

/// The environment variable that points both model gateways at a test
/// server on this computer. Only loopback roots are accepted, so a stray
/// value cannot send a key to another host.
pub const GATEWAY_BASE_URL_VARIABLE: &str = "ATH_GATEWAY_BASE_URL";

fn published_version() -> Result<String, String> {
    crate::runtime::update::fetch_latest_version(crate::runtime::update::REGISTRY_LATEST_URL)
}

/// True for an `http` root on 127.0.0.1, localhost, or [::1].
fn is_loopback_root(url: &str) -> bool {
    ["http://127.0.0.1", "http://localhost", "http://[::1]"].iter().any(|host| {
        url.strip_prefix(host).is_some_and(|rest| rest.is_empty() || rest.starts_with(':') || rest.starts_with('/'))
    })
}

/// Migrate after the person confirmed the previewed backup path.
fn confirmed_migration(store: &mut Store, preview: &MigrationPreview) -> crate::Result<PathBuf> {
    let options = crate::store::migrate::MigrationOptions {
        backup_path: preview.planned_backup_path.as_ref().map(PathBuf::from),
        now_millis: None,
    };
    let result = crate::store::migrate::execute_confirmed_migration(store, &options)?;
    result.backup_path.map(PathBuf::from).ok_or_else(|| Error::other("Migration backup path is missing"))
}

/// One running command: its I/O and the services it may call.
pub struct Context<'a> {
    pub io: Io<'a>,
    pub services: Services,
}

/// Why a command stopped early.
#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    /// Print `ath: <message>` on stderr and exit 1 (S2, S3). The message is
    /// printed once.
    Refused(String),
    /// The command is in the table but its library operation is not
    /// connected yet. Printed like a refusal.
    NotConnected(String),
    /// Exit with this status. The command has already written its output.
    Exit(i32),
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Failure::Refused(error.to_string())
    }
}

impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Failure::Refused(error.to_string())
    }
}

/// A refusal with this message.
pub fn refuse(message: impl Into<String>) -> Failure {
    Failure::Refused(message.into())
}

pub type CommandResult = Result<(), Failure>;

/// The confirmation flags a previewing command reads (S18).
#[derive(Debug, Clone, Copy, Default)]
pub struct Confirm {
    pub yes: bool,
    pub dry_run: bool,
    pub json: bool,
}

impl Context<'_> {
    /// `console.log`: the text and a newline on stdout.
    pub fn say(&mut self, text: &str) -> CommandResult {
        self.io.stdout.write_all(text.as_bytes())?;
        self.io.stdout.write_all(b"\n")?;
        Ok(())
    }

    /// `JSON.stringify(value, null, 2)` and a newline on stdout (S9).
    pub fn say_json(&mut self, value: &Value) -> CommandResult {
        let text = crate::json::to_pretty(value);
        self.say(&text)
    }

    /// One line on stderr. A failure to write to stderr is ignored: there is
    /// nowhere left to report it.
    pub fn note(&mut self, text: &str) {
        let _ = self.io.stderr.write_all(text.as_bytes());
        let _ = self.io.stderr.write_all(b"\n");
    }

    /// `ath: warning: <text>` for each file-permission warning, unless the
    /// output is JSON (S17).
    fn warn_readable_paths(&mut self, path: &Path, json: bool) {
        if json {
            return;
        }
        for warning in files::readable_path_warnings(path) {
            self.note(&format!("ath: warning: {warning}"));
        }
    }

    /// Resolve a path against the working directory.
    pub fn resolve(&self, path: &str) -> PathBuf {
        resolve_path(&self.io.cwd, path)
    }

    /// `findOrFail`: the database a command uses (S10–S13).
    pub fn find_database(&self, explicit: Option<&str>) -> Result<PathBuf, Failure> {
        find_database(&self.io.cwd, explicit).map_err(Failure::Refused)
    }

    /// `openOrFail`: open for writing (S14). A database that needs a SQL
    /// migration is refused, unless a person at a terminal confirms the
    /// migration first (S15).
    pub fn open_for_write(&mut self, path: &Path, json: bool) -> Result<Store, Failure> {
        self.check_path(path)?;
        self.warn_readable_paths(path, json);
        let mut store = match Store::open(path, OpenOptions { read_only: false, allow_outdated_schema: false }) {
            Ok(store) => store,
            Err(error @ Error::MigrationRequired(_)) => {
                if json || !self.io.terminal.stdin || !self.io.terminal.stdout {
                    return Err(error.into());
                }
                let Error::MigrationRequired(preview) = error else { unreachable!() };
                self.migrate_on_open(path, *preview)?
            }
            Err(error) => return Err(error.into()),
        };
        self.finish_pending_file_work(&mut store, json);
        Ok(store)
    }

    /// Finish attachment file work an earlier command left pending (spec
    /// 9.2): approved removals, and files an interrupted write left. Cleanup
    /// never stops the command; what it cannot finish stays pending, `ath
    /// check` reports it, and the next writable command tries again.
    fn finish_pending_file_work(&mut self, store: &mut Store, json: bool) {
        let unfinished = match crate::package::resume_pending_file_operations(store) {
            Ok(report) => report
                .failed_files
                .into_iter()
                .map(|failure| format!("{}: {}", failure.relative_path, failure.message))
                .collect(),
            Err(error) => vec![error.to_string()],
        };
        if json {
            return;
        }
        for detail in unfinished {
            self.note(&format!("ath: warning: pending attachment cleanup is not finished: {detail}"));
        }
    }

    /// The write-capable open `migrate` and `repair` use: an outdated
    /// schema is allowed and nothing is asked.
    pub fn open_outdated_for_write(&mut self, path: &Path, json: bool) -> Result<Store, Failure> {
        self.check_path(path)?;
        self.warn_readable_paths(path, json);
        Ok(Store::open(path, OpenOptions { read_only: false, allow_outdated_schema: true })?)
    }

    /// `openReadOnlyOrFail`: open without changing the file, even when it
    /// needs a migration (S16).
    pub fn open_read_only(&mut self, path: &Path, json: bool) -> Result<Store, Failure> {
        self.check_path(path)?;
        self.warn_readable_paths(path, json);
        Ok(Store::open(path, OpenOptions { read_only: true, allow_outdated_schema: true })?)
    }

    /// The path checks the 2.1.0 open ran before its permission warnings.
    fn check_path(&self, path: &Path) -> Result<(), Failure> {
        files::refuse_symbolic_link_components(path, false)?;
        files::assert_parent_not_writable_by_others(path)?;
        Ok(())
    }

    /// `interactiveMigrationOnOpen` (S15).
    fn migrate_on_open(&mut self, path: &Path, preview: MigrationPreview) -> Result<Store, Failure> {
        let mut shown = Map::new();
        shown.insert("file".into(), Value::String(path.display().to_string()));
        if let Value::Object(fields) = crate::json::to_value(&preview) {
            shown.extend(fields);
        }
        shown.insert("write_confirmed".into(), Value::Bool(false));
        self.say_json(&Value::Object(shown))?;
        if !preview.migration_needed {
            return self.open_for_write(path, false);
        }
        if preview.planned_steps.is_empty() {
            return Err(refuse(preview.compatibility_effect));
        }
        if preview.planned_backup_path.is_none() {
            return Err(refuse("SQL schema migration requires a local database file"));
        }
        if !self.confirm(Confirm::default(), "Create a verified backup and apply this migration?")? {
            return Err(refuse("migration was not confirmed; run ath migrate or answer yes when prompted on open"));
        }
        let typed = self.ask(&format!("Type the database ID {} to confirm migration: ", preview.database_id));
        if typed.as_deref() != Some(preview.database_id.as_str()) {
            return Err(refuse("second confirmation did not exactly match the database ID; nothing was migrated"));
        }
        self.warn_readable_paths(path, false);
        let mut store = Store::open(path, OpenOptions { read_only: false, allow_outdated_schema: true })?;
        let backup = (self.services.migrate)(&mut store, &preview)?;
        self.say(&format!(
            "migrated SQL schema {} to {} with verified backup at {}",
            preview.source_sql_schema_version,
            preview.target_sql_schema_version,
            backup.display()
        ))?;
        Ok(store)
    }

    /// `confirmWrite` (S18): false without asking for `--json` or
    /// `--dry-run`, true for `--yes`, a refusal without a terminal on
    /// stdout, else the person's answer. Only `y` or `yes`, in any case,
    /// means yes.
    pub fn confirm(&mut self, flags: Confirm, question: &str) -> Result<bool, Failure> {
        if flags.json || flags.dry_run {
            return Ok(false);
        }
        if flags.yes {
            return Ok(true);
        }
        if !self.io.terminal.stdout {
            return Err(refuse(NO_TERMINAL_TO_CONFIRM));
        }
        let answer = self.ask(&format!("{question} [y] yes  [n] no ")).unwrap_or_default();
        Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
    }

    /// `askOnTerminal` (S19): print the question on stdout, then read one
    /// line from stdin when it is a terminal, otherwise from the controlling
    /// terminal. The answer is trimmed. `None` when nothing can be read.
    pub fn ask(&mut self, question: &str) -> Option<String> {
        let _ = self.io.stdout.write_all(question.as_bytes());
        let _ = self.io.stdout.flush();
        let line = if self.io.terminal.stdin {
            read_line(self.io.stdin)
        } else {
            let mut terminal = (self.io.open_terminal)()?;
            read_line(&mut *terminal)
        };
        line.map(|line| js_trim(&line).to_string())
    }

    /// One line from standard input, for a question that standard input
    /// answers (`ath init`). The line ending is removed; nothing else is.
    /// `None` at the end of input.
    pub fn read_stdin_line(&mut self) -> Option<String> {
        read_line(self.io.stdin)
    }

    /// `readStdin` (S22): all of standard input as UTF-8. At a terminal, a
    /// line on stderr says how to end the entry.
    pub fn read_stdin(&mut self) -> Result<String, Failure> {
        if self.io.terminal.stdin {
            self.note("Paste the entry, then press Ctrl-D:");
        }
        let mut bytes = Vec::new();
        self.io.stdin.read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// An environment variable.
    pub fn env(&self, name: &str) -> Option<&str> {
        self.io.env.get(name).map(String::as_str)
    }

    /// The password store. This system's store is built on first use.
    pub fn secret_store(&mut self) -> Rc<dyn SecretStore> {
        self.services.secret_store.get_or_insert_with(|| Rc::new(OsSecretStore::for_this_system())).clone()
    }

    /// The gateway API roots: the injected roots, a loopback root from the
    /// environment, or the published roots.
    pub fn gateway_endpoints(&self) -> Result<GatewayEndpoints, Failure> {
        if let Some(endpoints) = &self.services.gateway_endpoints {
            return Ok(endpoints.clone());
        }
        match self.env(GATEWAY_BASE_URL_VARIABLE).filter(|value| !value.is_empty()) {
            None => Ok(GatewayEndpoints::default()),
            Some(root) if is_loopback_root(root) => Ok(GatewayEndpoints::both(root.trim_end_matches('/'))),
            Some(_) => Err(refuse(format!(
                "{GATEWAY_BASE_URL_VARIABLE} must name a test server on this computer, such as http://127.0.0.1:8080; nothing was sent"
            ))),
        }
    }
}

/// One line without its line ending, or `None` at the end of input. Bytes
/// that are not UTF-8 become replacement characters, as Node decodes them.
fn read_line(input: &mut dyn BufRead) -> Option<String> {
    let mut bytes = Vec::new();
    match input.read_until(b'\n', &mut bytes) {
        Ok(0) | Err(_) => None,
        Ok(_) => {
            if bytes.last() == Some(&b'\n') {
                bytes.pop();
                if bytes.last() == Some(&b'\r') {
                    bytes.pop();
                }
            }
            Some(String::from_utf8_lossy(&bytes).into_owned())
        }
    }
}

/// `path.resolve(cwd, path)`: an absolute path with `.` and `..` resolved
/// without reading the file system.
pub fn resolve_path(cwd: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    let joined = if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) };
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

/// `findFile` (S10–S13). Without a path, only `athlete.ath.sqlite` in the
/// working directory is used. An empty `--file` counts as no path, as it
/// did in 2.1.0.
pub fn find_database(cwd: &Path, explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(explicit) = explicit.filter(|text| !text.is_empty()) {
        let path = resolve_path(cwd, explicit);
        let shown = path.display().to_string();
        if !path.exists() {
            return Err(format!("file not found: {shown}"));
        }
        if shown.ends_with(PORTABLE_FILE_SUFFIX) {
            return Err(format!(
                "Athletic Standard prerelease JSON files are not supported by v1. Keep {shown} unchanged and run `ath init` to create a v1 SQLite database."
            ));
        }
        if !shown.ends_with(FILE_SUFFIX) {
            return Err(format!("the live athlete record must be a {FILE_SUFFIX} SQLite file, not {shown}"));
        }
        return Ok(path);
    }
    let default_path = resolve_path(cwd, DEFAULT_FILENAME);
    if default_path.exists() {
        return Ok(default_path);
    }
    let mut portable: Vec<String> = std::fs::read_dir(cwd)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(PORTABLE_FILE_SUFFIX))
        .collect();
    portable.sort();
    if portable.len() == 1 {
        return Err(format!(
            "found {}, but Athletic Standard prerelease JSON files are not supported by v1. Keep it unchanged and run `ath init` to create {DEFAULT_FILENAME}.",
            portable[0]
        ));
    }
    Err(format!("no {DEFAULT_FILENAME} in {} — run `ath init` to create one, or pass --file", cwd.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_is_lexical() {
        let cwd = Path::new("/work/here");
        assert_eq!(resolve_path(cwd, "a/../b.ath.sqlite"), PathBuf::from("/work/here/b.ath.sqlite"));
        assert_eq!(resolve_path(cwd, "../x"), PathBuf::from("/work/x"));
        assert_eq!(resolve_path(cwd, "/abs/./y/"), PathBuf::from("/abs/y"));
    }

    #[test]
    fn lines_lose_only_their_ending() {
        let mut input: &[u8] = b"  Sam \r\nnext";
        assert_eq!(read_line(&mut input).as_deref(), Some("  Sam "));
        assert_eq!(read_line(&mut input).as_deref(), Some("next"));
        assert_eq!(read_line(&mut input), None);
    }
}
