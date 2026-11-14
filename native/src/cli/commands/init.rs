//! `ath init`: the target name, the existing-file refusal, the profile
//! questions, then a new database with the profile, the manual source, and
//! the seeded benchmarks, and the agent skill unless `--no-skill`.

use std::path::PathBuf;

use super::shared::object;
use crate::assets::skill;
use crate::cli::parse::Args;
use crate::cli::runtime::{refuse, CommandResult, Context, FILE_SUFFIX};
use crate::cli::text::js_number;
use crate::error::Error;
use crate::json::{self, Value};
use crate::model::{time, PathSegment, RecordActor, RecordData, RecordType, ATHLETIC_STANDARD_VERSION};
use crate::prediction::benchmarks::{seed_benchmark_records, SEED_BENCHMARKS};
use crate::prediction::grade::tool_actor;
use crate::store::{Applied, Changes, CreateOptions, NewRecord, Store};

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

/// Create the database, seed it, and install the skill.
fn create(context: &mut Context<'_>, request: InitRequest) -> CommandResult {
    let created_at = time::now_utc();
    let seeded = match create_database(&request, &created_at) {
        Ok(seeded) => seeded,
        Err(error) => {
            for suffix in ["", "-wal", "-shm"] {
                let mut path = request.path.clone().into_os_string();
                path.push(suffix);
                let _ = std::fs::remove_file(path);
            }
            return Err(refuse(format!("could not create a valid v1 database: {error}. Nothing was kept.")));
        }
    };
    // The skill always goes to .agents/skills, which every major harness
    // reads, and into any harness folder already here (D79).
    let cwd = context.io.cwd.clone();
    let (installed, mappings) = if request.install_skill {
        (skill::install_skill(&cwd)?, skill::install_mappings(&cwd)?)
    } else {
        (Vec::new(), Vec::new())
    };
    if request.json {
        let paths = |paths: &[PathBuf]| Value::Array(paths.iter().map(|path| path_value(path)).collect());
        return context.say_json(&object(vec![
            ("created", path_value(&request.path)),
            ("athleticstandard_version", Value::String(ATHLETIC_STANDARD_VERSION.to_string())),
            ("database_revision", Value::from(seeded.as_ref().map_or(0, |applied| applied.database_revision))),
            (
                "benchmark_record_ids",
                Value::Array(
                    seeded
                        .iter()
                        .flat_map(|applied| &applied.records)
                        .filter(|record| record.record_type() == RecordType::BenchmarkDefinition)
                        .map(|record| Value::String(record.record_id.clone()))
                        .collect(),
                ),
            ),
            ("skill_installed", paths(&installed)),
            ("mappings_installed", paths(&mappings)),
        ]));
    }
    context.say(&format!("created {}", request.path.display()))?;
    if request.empty {
        context.say("  empty database, ready for a v1 package restore")?;
    } else {
        let keys: Vec<&str> = SEED_BENCHMARKS.iter().map(|seed| seed.key).collect();
        context.say(&format!("  seeded {} benchmarks: {}", keys.len(), keys.join(", ")))?;
    }
    for path in &installed {
        context.say(&format!("  installed the agent skill into {}", path.display()))?;
    }
    if !mappings.is_empty() {
        context.say(&format!("  copied {} mapping files into {}/mappings", mappings.len(), skill::ALWAYS_DIR))?;
    }
    if !installed.is_empty() {
        context.say(&format!(
            "  {}/skills is read by Claude Code, Cursor, Codex, and Gemini CLI. `ath skill install` replaces an older tool skill after an upgrade.",
            skill::ALWAYS_DIR
        ))?;
    } else if request.install_skill {
        context.say("  the skill was not installed. `ath skill install` does it later.")?;
    }
    context.say("  next: `ath import <export-file>` to load device data")
}

/// A path as JSON text.
fn path_value(path: &std::path::Path) -> Value {
    Value::String(path.display().to_string())
}

/// Create the database and, unless `--empty`, write the profile, the manual
/// source, and the seeded benchmarks as one change by the tool.
fn create_database(request: &InitRequest, created_at: &str) -> crate::Result<Option<Applied>> {
    let mut store =
        Store::create(&request.path, CreateOptions { database_id: None, created_at: Some(created_at.to_string()) })?;
    let seeded = if request.empty {
        None
    } else {
        let tool = tool_actor();
        let mut creates = vec![
            new_record(RecordType::AthleteProfile, profile_data(request)?, &tool, created_at)?,
            new_record(RecordType::Source, manual_source_data(), &tool, created_at)?,
        ];
        creates.extend(seed_benchmark_records(created_at));
        Some(store.apply(Changes { creates, ..Changes::default() })?)
    };
    store.close()?;
    Ok(seeded)
}

fn new_record(record_type: RecordType, data: Value, tool: &RecordActor, created_at: &str) -> crate::Result<NewRecord> {
    let data = RecordData::decode(record_type, &data)
        .map_err(|error| Error::Invalid(error.under(&[PathSegment::Key("data".to_string())])))?;
    let mut record = NewRecord::new(data, tool.clone());
    record.created_at = Some(created_at.to_string());
    Ok(record)
}

/// The profile 2.1.0 wrote: a name and birth year when given, a sex only
/// when it is exactly `male` or `female`, the display units, and the
/// default timezone when given.
fn profile_data(request: &InitRequest) -> crate::Result<Value> {
    let mut fields = Vec::new();
    if let Some(name) = request.name.as_deref().filter(|name| !name.is_empty()) {
        fields.push(("name", Value::String(name.to_string())));
    }
    if let Some(year) = request.birth_year.as_deref().filter(|year| !year.is_empty()) {
        // `Number(<text>)`, kept as JavaScript would write it. JSON cannot
        // hold NaN or Infinity, so the validator's refusal is named here.
        let number = js_number(year);
        if !number.is_finite() {
            let received = if number.is_nan() { "NaN" } else { "Infinity" };
            return Err(Error::other(format!("data.birth_year: Invalid input: expected number, received {received}")));
        }
        let value = json::parse(&json::format_number(number))?;
        fields.push(("birth_year", value));
    }
    if let Some(sex) = request.sex.as_deref().filter(|sex| *sex == "male" || *sex == "female") {
        fields.push(("sex", Value::String(sex.to_string())));
    }
    fields.push(("display_units", Value::String(if request.imperial { "imperial" } else { "metric" }.to_string())));
    if let Some(zone) = request.timezone.as_deref().filter(|zone| !zone.is_empty()) {
        fields.push(("default_time_zone", Value::String(zone.to_string())));
    }
    Ok(object(fields))
}

fn manual_source_data() -> Value {
    object(vec![
        ("source_type", Value::String("manual".into())),
        ("data_writer", Value::String("ath".into())),
        ("data_route", Value::String("manual_entry".into())),
        ("detail", Value::String("Manually recorded data".into())),
    ])
}
