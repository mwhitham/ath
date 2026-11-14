//! `ath init`: the checks and questions before a new database is created.
//!
//! Creating the database, seeding the benchmarks, and installing the skill
//! belong to library operations that are not connected yet. Everything up to
//! that point is here: the target name, the existing-file refusal, the
//! non-interactive refusal, and the profile questions.

use crate::cli::parse::Args;
use crate::cli::runtime::{refuse, CommandResult, Context, Failure, FILE_SUFFIX};

/// The refusal when questions would be asked but standard input is not a
/// terminal. 2.1.0 exited 0 having created nothing (deliberate change; see
/// `native/reference/cli-changes.md`).
pub const NO_TERMINAL_FOR_QUESTIONS: &str =
    "there is no terminal to ask on, so nothing was created. Pass --yes to skip the questions.";

/// The refusal when standard input ends before every question is answered.
pub const QUESTIONS_NOT_ANSWERED: &str =
    "the questions were not answered, so nothing was created. Pass --yes to skip the questions.";

/// What `ath init` creates, after the questions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitRequest {
    /// Absolute path of the new database.
    pub path: std::path::PathBuf,
    pub name: Option<String>,
    /// The birth year text as given; 2.1.0 stored `Number(<text>)`.
    pub birth_year: Option<String>,
    /// The sex text as given; 2.1.0 kept only `male` or `female`.
    pub sex: Option<String>,
    /// `imperial` only when exactly `imperial` was given.
    pub imperial: bool,
    pub timezone: Option<String>,
    pub empty: bool,
    pub install_skill: bool,
    pub json: bool,
}

pub fn run(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let path = context.resolve(args.value("file").expect("--file has a default"));
    let shown = path.display().to_string();
    if !shown.ends_with(FILE_SUFFIX) {
        return Err(refuse(format!("the live database name must end in {FILE_SUFFIX}: {shown}")));
    }
    if path.exists() {
        return Err(refuse(format!(
            "{shown} already exists, and overwriting it would lose whatever is in it. Read it with `ath stats`, or start a second file with `--file <name>{FILE_SUFFIX}`."
        )));
    }
    let mut request = InitRequest {
        path,
        name: args.value("name").map(ToString::to_string),
        birth_year: args.value("birth_year").map(ToString::to_string),
        sex: args.value("sex").map(ToString::to_string),
        imperial: args.value("units") == Some("imperial"),
        timezone: args.value("timezone").map(ToString::to_string),
        empty: args.flag("empty"),
        install_skill: args.flag("skill"),
        json: args.flag("json"),
    };
    if !args.flag("yes") && !request.json {
        if !context.io.terminal.stdin {
            return Err(refuse(NO_TERMINAL_FOR_QUESTIONS));
        }
        ask_unless_given(context, &mut request.name, "Name (optional): ")?;
        ask_unless_given(context, &mut request.birth_year, "Birth year (optional): ")?;
        ask_unless_given(context, &mut request.sex, "Sex, male/female (optional): ")?;
        ask_unless_given(context, &mut request.timezone, "Default timezone, such as America/Chicago (optional): ")?;
    }
    create(context, request)
}

/// Ask one profile question on stdout and read the answer from standard
/// input, unless a flag already gave the value. An empty answer means absent.
fn ask_unless_given(context: &mut Context<'_>, value: &mut Option<String>, question: &str) -> CommandResult {
    if value.is_some() {
        return Ok(());
    }
    context.io.stdout.write_all(question.as_bytes())?;
    context.io.stdout.flush()?;
    let answer = context.read_stdin_line().ok_or_else(|| refuse(QUESTIONS_NOT_ANSWERED))?;
    if !answer.is_empty() {
        *value = Some(answer);
    }
    Ok(())
}

/// Create the database, seed it, and install the skill. Not connected yet.
fn create(_: &mut Context<'_>, _: InitRequest) -> CommandResult {
    Err(Failure::NotConnected("ath init is not connected to the native library yet; nothing was created".to_string()))
}
