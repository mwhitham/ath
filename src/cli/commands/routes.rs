//! The keyed import routes of `ath import` (Garmin and Strava files, and a
//! JSON file read through `--mapping`) and `ath pull garmin`.
//!
//! These ran in the 2.1.0 helper. Their results have the `source` or
//! `mapping`, `record_ids`, `understood`, `preserved`, and `left_out` fields
//! the helper printed; `ProviderPreview::report_value` builds them.

use std::path::Path;

use super::shared::{with_store_at, CommandEnvironment, Open};
use crate::cli::parse::Args;
use crate::cli::runtime::{refuse, CommandResult, Context};
use crate::import::{self, mapping, ImportOptions, SharedImport};
use crate::json::{self, Value};
use crate::runtime::garmin_pull::{pull_day, PERSONAL_API_BASE};
use crate::runtime::keyring::{Keyring, GARMIN_ACCOUNT};

/// The line every keyed preview ends with.
const NOTHING_WRITTEN: &str = "nothing written; pass --yes after reviewing this preview";

/// The result with `write_confirmed: false`, as 2.1.0 printed it.
fn unconfirmed(mut result: Value) -> Value {
    if let Value::Object(fields) = &mut result {
        fields.insert("write_confirmed".into(), Value::Bool(false));
    }
    result
}

fn texts(value: &Value, field: &str) -> Vec<String> {
    match value.get(field) {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).map(ToString::to_string).collect(),
        _ => Vec::new(),
    }
}

fn joined_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

/// `renderSharedImport` from 2.1.0.
pub fn render_shared(kind: SharedImport, result: &Value) -> String {
    let mut lines = vec![
        format!("{} import", kind.as_str()),
        format!("understood: {}", joined_or_none(&texts(result, "understood"))),
        format!("kept under original names: {}", joined_or_none(&texts(result, "preserved"))),
    ];
    for item in texts(result, "left_out") {
        lines.push(format!("left out: {item}"));
    }
    let ids = texts(result, "record_ids");
    if !ids.is_empty() {
        lines.push(format!("records: {}", ids.join(", ")));
    }
    lines.join("\n")
}

/// `renderMappedImport` from 2.1.0.
pub fn render_mapped(result: &Value) -> String {
    let mapping = result.get("mapping").and_then(Value::as_str).unwrap_or("mapped");
    let mut lines = vec![
        format!("{mapping} import"),
        format!("understood: {}", joined_or_none(&texts(result, "understood"))),
        format!("kept under original names: {}", joined_or_none(&texts(result, "preserved"))),
    ];
    for item in texts(result, "left_out") {
        lines.push(format!("left out: {item}"));
    }
    let count = |name: &str| {
        result.get("outcome").and_then(|outcome| outcome.get(name)).map(json::to_string).unwrap_or_else(|| "0".into())
    };
    lines.push(format!(
        "outcome: {} new, {} changed, {} unchanged",
        count("new"),
        count("changed"),
        count("unchanged")
    ));
    let ids = texts(result, "record_ids");
    if !ids.is_empty() {
        lines.push(format!("records: {}. Pass --json to list their IDs.", ids.len()));
    }
    lines.join("\n")
}

/// `ath import <file> --mapping <name>`. The mapping file is chosen before
/// the database is opened: a path as given, else the installed copy in
/// `.agents/mappings`, else the shipped copy.
pub fn import_mapped(context: &mut Context<'_>, args: &Args, athlete_path: &Path, input: &Path) -> CommandResult {
    let name = args.value("mapping").expect("the mapped route has --mapping");
    let resolved = crate::assets::resolve_mapping(name, &context.io.cwd)?;
    let json = args.flag("json");
    let preview_only = json || !args.flag("yes");
    with_store_at(context, athlete_path, json, Open::Write, |context, store, _| {
        let mapping = mapping::load_mapping(&resolved.text)?;
        let preview = import::preview_mapped(store, input, &mapping, &ImportOptions::default())?;
        if !preview_only {
            preview.apply(store)?;
        }
        let result = preview.report_value(!preview_only)?;
        if json {
            return context.say_json(&unconfirmed(result));
        }
        context.say(&render_mapped(&result))?;
        if preview_only {
            context.say(NOTHING_WRITTEN)?;
        }
        Ok(())
    })
}

/// `ath import <path>` for Garmin and Strava files, folders, and archives.
/// A preview always comes first; `--yes` without `--json` writes it.
pub fn import_shared(
    context: &mut Context<'_>,
    args: &Args,
    athlete_path: &Path,
    input: &Path,
    kind: SharedImport,
) -> CommandResult {
    let json = args.flag("json");
    let yes = args.flag("yes");
    with_store_at(context, athlete_path, json, Open::Write, |context, store, _| {
        let preview = import::preview_shared(store, input, kind, &ImportOptions::default())?;
        if json || !yes {
            let result = preview.report_value(false)?;
            if json {
                return context.say_json(&unconfirmed(result));
            }
            context.say(&render_shared(kind, &result))?;
            return context.say(NOTHING_WRITTEN);
        }
        preview.apply(store)?;
        let written = preview.report_value(true)?;
        context.say(&format!("imported {}: {}", kind.as_str(), json::to_string(&written)))
    })
}

/// `ath pull garmin --date <day>`: one day from Garmin's personal
/// connection, written without a preview, as 2.1.0 did.
pub fn pull(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let source = super::shared::required_argument(args, "source");
    if source != "garmin" {
        return Err(refuse("pull currently reads garmin"));
    }
    let path = context.find_database(args.value("file"))?;
    let store_handle = context.secret_store();
    let env = context.io.env.clone();
    let environment = CommandEnvironment(&env);
    let key = Keyring::new(&*store_handle, &environment)
        .account_secret(GARMIN_ACCOUNT)
        .ok_or_else(|| refuse("no Garmin key is saved. Run ath key set garmin, or set GARMIN_PERSONAL_API_KEY."))?;
    let date = args.value("date").expect("the parser requires --date").to_string();
    let base_url = args.value("base_url").unwrap_or(PERSONAL_API_BASE).to_string();
    let json = args.flag("json");
    with_store_at(context, &path, json, Open::Write, |context, store, _| {
        let result = pull_day(store, &base_url, &key, &date, &ImportOptions::default())?;
        if json {
            context.say_json(&result)
        } else {
            context.say(&format!("pulled garmin for {date}"))
        }
    })
}
