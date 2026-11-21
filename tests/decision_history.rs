//! Recorded decisions stay appended (`build-history/README.md`).
//!
//! Committed decision files at the chosen base must remain a prefix of the
//! working copies. New D-numbers continue from the last number at that base.
//! A decision that exists only after that base may still be revised. Specs and
//! progress files are not checked.
//!
//! ```sh
//! cargo test --test decision_history
//! ATH_BASE_SHA=origin/main cargo test --locked --test decision_history -- --ignored working_tree_matches_recorded_decisions
//! ```
//!
//! Fetch the base first. A missing base fails the check. The ignored test reads
//! `ATH_BASE_SHA` and does not skip when that variable is empty.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const ORIGINAL: &str = "# Decisions\n\n## D1. Keep the original\n\nReason.\n";
const DECISION_FILE: &str = "build-history/v1.0.0/decisions.md";

static TEMP_DIRS: AtomicU64 = AtomicU64::new(0);

struct GitRepo {
    path: PathBuf,
}

impl GitRepo {
    fn new(text: &str) -> Self {
        let n = TEMP_DIRS.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ath-history-{}-{n}", std::process::id()));
        fs::create_dir_all(path.join("build-history/v1.0.0")).unwrap();
        let repo = Self { path };
        repo.git(&["init", "-q"]);
        repo.git(&["config", "user.name", "Synthetic Test"]);
        repo.git(&["config", "user.email", "test@example.invalid"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo.git(&["config", "core.autocrlf", "false"]);
        fs::write(repo.path.join(DECISION_FILE), text).unwrap();
        repo.git(&["add", "."]);
        repo.git(&["commit", "-qm", "Synthetic baseline"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .unwrap_or_else(|error| panic!("the decision check needs git: {error}"));
        assert!(output.status.success(), "git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).expect("git output is UTF-8")
    }

    fn base(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_string()
    }
}

impl Drop for GitRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn is_decision_file(path: &str) -> bool {
    let mut parts = path.split('/');
    parts.next() == Some("build-history")
        && parts.next().is_some()
        && parts.next() == Some("decisions.md")
        && parts.next().is_none()
}

fn decision_numbers(text: &str) -> Vec<u32> {
    text.lines().filter_map(decision_number).collect()
}

/// `## D12` followed by a word boundary, matching `^## D(\d+)\b`.
fn decision_number(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("## D")?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let after = &rest[digits.len()..];
    let boundary = after.chars().next().is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
    if !boundary {
        return None;
    }
    digits.parse().ok()
}

fn decision_history_errors(base: &BTreeMap<String, String>, current: &BTreeMap<String, String>) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen: BTreeMap<u32, String> = BTreeMap::new();
    let old_numbers: BTreeSet<u32> = base.values().flat_map(|text| decision_numbers(text)).collect();
    let last_old = old_numbers.iter().copied().max().unwrap_or(0);
    let mut added = Vec::new();

    for (path, old_text) in base {
        match current.get(path) {
            None => errors.push(format!("{path}: recorded decisions were deleted or moved")),
            Some(text) if !text.starts_with(old_text.as_str()) => {
                errors.push(format!("{path}: recorded decisions were rewritten; append a new decision"));
            }
            Some(_) => {}
        }
    }

    for (path, text) in current {
        for number in decision_numbers(text) {
            if let Some(first) = seen.get(&number) {
                errors.push(format!("D{number}: duplicate in {first} and {path}"));
            }
            seen.insert(number, path.clone());
            if !old_numbers.contains(&number) {
                added.push(number);
            }
        }
    }

    let sorted: Vec<u32> = added.into_iter().collect::<BTreeSet<_>>().into_iter().collect();
    for (index, number) in sorted.iter().copied().enumerate() {
        if number != last_old + index as u32 + 1 {
            errors.push(format!("D{number}: new decision numbers must continue from D{}", last_old + 1));
            break;
        }
    }
    errors
}

fn null_separated(bytes: &[u8]) -> Result<Vec<String>, String> {
    let text = String::from_utf8(bytes.to_vec()).map_err(|_| "git listed a path that is not UTF-8".to_string())?;
    Ok(text.split('\0').filter(|path| !path.is_empty()).map(str::to_string).collect())
}

fn git_bytes(cwd: &Path, args: &[String]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("the decision check needs git: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr).trim()))
    }
}

fn check_decision_history(base_ref: &str, cwd: &Path) -> Result<Vec<String>, String> {
    let spec = format!("{base_ref}^{{commit}}");
    let base = git_bytes(cwd, &["rev-parse".into(), "--verify".into(), "--end-of-options".into(), spec])?;
    let base = String::from_utf8(base).map_err(|_| "the base commit is not UTF-8".to_string())?;
    let base = base.trim();

    let listed = git_bytes(cwd, &["ls-tree".into(), "-r".into(), "--name-only".into(), "-z".into(), base.into()])?;
    let mut before = BTreeMap::new();
    for path in null_separated(&listed)?.into_iter().filter(|path| is_decision_file(path)) {
        let blob = git_bytes(cwd, &["show".into(), format!("{base}:{path}")])?;
        let text = String::from_utf8(blob).map_err(|_| format!("{path}: recorded decisions are not UTF-8"))?;
        before.insert(path, text);
    }

    let tracked = git_bytes(cwd, &["ls-files".into(), "-z".into()])?;
    let mut after = BTreeMap::new();
    for path in null_separated(&tracked)?.into_iter().filter(|path| is_decision_file(path)) {
        match fs::read(cwd.join(&path)) {
            Ok(bytes) => {
                let text = String::from_utf8(bytes).map_err(|_| format!("{path}: the working copy is not UTF-8"))?;
                after.insert(path, text);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{path}: {error}")),
        }
    }
    Ok(decision_history_errors(&before, &after))
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn accepts_unchanged_history_appended_decisions_and_revisions_to_unmerged_proposals() {
    let repo = GitRepo::new(ORIGINAL);
    let base = repo.base();
    assert!(check_decision_history(&base, &repo.path).unwrap().is_empty());

    fs::write(repo.path.join(DECISION_FILE), format!("{ORIGINAL}\n## D2. New proposal\n\nReason.\n")).unwrap();
    assert!(check_decision_history(&base, &repo.path).unwrap().is_empty());

    repo.git(&["add", "."]);
    repo.git(&["commit", "-qm", "Unmerged proposal"]);
    fs::write(repo.path.join(DECISION_FILE), format!("{ORIGINAL}\n## D2. Revised proposal\n\nBetter reason.\n"))
        .unwrap();
    assert!(
        check_decision_history(&base, &repo.path).unwrap().is_empty(),
        "a decision that is not in the base may still be revised"
    );
}

#[test]
fn rejects_a_rewrite_a_deletion_and_a_move_of_recorded_history() {
    let repo = GitRepo::new(ORIGINAL);
    let base = repo.base();
    fs::write(repo.path.join(DECISION_FILE), ORIGINAL.replace("Reason.", "Different reason.")).unwrap();
    let errors = check_decision_history(&base, &repo.path).unwrap();
    assert!(errors.iter().any(|error| error.contains("rewritten")), "{errors:?}");

    fs::remove_file(repo.path.join(DECISION_FILE)).unwrap();
    let errors = check_decision_history(&base, &repo.path).unwrap();
    assert!(errors.iter().any(|error| error.contains("deleted or moved")), "{errors:?}");

    fs::write(repo.path.join("build-history/v1.0.0/renamed.md"), ORIGINAL).unwrap();
    repo.git(&["add", "-A"]);
    let errors = check_decision_history(&base, &repo.path).unwrap();
    assert!(!errors.is_empty(), "moving a decision file must still fail");
}

#[test]
fn checks_numbering_across_version_folders_without_freezing_specs_or_progress() {
    let repo = GitRepo::new(ORIGINAL);
    let base = repo.base();
    fs::create_dir_all(repo.path.join("build-history/v2.0.0")).unwrap();
    let next = repo.path.join("build-history/v2.0.0/decisions.md");
    fs::write(&next, "## D2. New version\n").unwrap();
    fs::write(repo.path.join("build-history/v1.0.0/progress.md"), "A completed step.\n").unwrap();
    repo.git(&["add", "."]);
    assert!(check_decision_history(&base, &repo.path).unwrap().is_empty());

    fs::write(&next, "## D1. Duplicate\n").unwrap();
    let errors = check_decision_history(&base, &repo.path).unwrap();
    assert!(errors.iter().any(|error| error.contains("duplicate")), "{errors:?}");

    fs::write(&next, "## D3. Skipped number\n").unwrap();
    let errors = check_decision_history(&base, &repo.path).unwrap();
    assert!(errors.iter().any(|error| error.contains("continue from D2")), "{errors:?}");
}

#[test]
fn fails_closed_for_an_unavailable_base_and_does_not_change_files() {
    let repo = GitRepo::new(ORIGINAL);
    let error = check_decision_history("missing-base", &repo.path).expect_err("a missing base fails");
    assert!(!error.is_empty(), "{error}");
    assert_eq!(fs::read_to_string(repo.path.join(DECISION_FILE)).unwrap(), ORIGINAL);
}

#[test]
#[ignore]
fn working_tree_matches_recorded_decisions() {
    let base = std::env::var("ATH_BASE_SHA").expect("set ATH_BASE_SHA to the base commit; this test does not skip");
    let errors = check_decision_history(base.trim(), &repository_root()).expect("the base commit can be read");
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
