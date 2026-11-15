//! The package commands: `export`, the package route of `import`,
//! `conflicts show`, `conflicts resolve`, and `erase`.

use std::path::Path;

use super::shared::{confirm_flags, object, required_argument, with_store, Open};
use crate::cli::parse::Args;
use crate::cli::runtime::{refuse, CommandResult, Context, Failure};
use crate::json::{self, Value};
use crate::model::ErasureReasonCode;
use crate::package::{
    apply_package_import, assert_conflict_set_current, clear_conflict_set, conflict_resolutions, erase_record,
    export_package, preview_erasure, preview_package_import, read_conflict_set, resolve_conflict, save_conflict_set,
    show_conflicts, ConflictChoice, ExportOptions, PackageImportPreview,
};
use crate::store::Store;

/// `ath export <directory>`: a complete package in a new folder.
pub fn export(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let directory = context.resolve(required_argument(args, "directory"));
    let options = ExportOptions { exported_at: args.value("exported_at").map(ToString::to_string) };
    let json = args.flag("json");
    with_store(context, args, Open::ReadOnly, |context, store, _| {
        let result = export_package(store, &directory, &options)?;
        if json {
            return context.say_json(&json::to_value(&result));
        }
        context.say(&format!("exported database revision {} to {}", result.database_revision, result.directory))?;
        context.say("  athlete.ath.json")?;
        context.say("  export-manifest.json")
    })
}

/// The package route of `ath import`, in an open database.
pub fn import_package(context: &mut Context<'_>, store: &mut Store, input: &Path, args: &Args) -> CommandResult {
    let json = args.flag("json");
    let preview = preview_package_import(store, input)?;
    if preview.is_unrepresentable() {
        // 2.1.0 printed this preview and exited 0 without touching the
        // conflict file, even with --yes.
        if json {
            return context.say_json(&json::to_value(&preview));
        }
        context.say(&render_package_preview(&preview))?;
        return context.say("nothing written; every unknown field and unrepresentable record must be resolved");
    }
    if json {
        save_or_clear_conflicts(store, &preview)?;
        return context.say_json(&json::to_value(&preview));
    }
    context.say(&render_package_preview(&preview))?;
    if !args.flag("yes") {
        if save_or_clear_conflicts(store, &preview)? {
            context.say("review with ath conflicts show, then record each choice with ath conflicts resolve")?;
        }
        return context.say("nothing written; pass --yes after reviewing this preview");
    }
    let mut resolutions = Vec::new();
    if !preview.conflicts.is_empty() {
        let state = read_conflict_set(store)?;
        assert_conflict_set_current(store, &state, &preview)?;
        resolutions = conflict_resolutions(&state);
        let unresolved = preview.conflicts.len().saturating_sub(resolutions.len());
        if unresolved > 0 {
            return Err(refuse(format!(
                "import has {unresolved} unresolved conflict{}; use ath conflicts show and ath conflicts resolve",
                if unresolved == 1 { "" } else { "s" }
            )));
        }
    }
    let applied = apply_package_import(store, &preview, &resolutions)?;
    clear_conflict_set(store)?;
    context.say(&format!(
        "{} package at database revision {}",
        if applied.restored { "restored" } else { "imported" },
        applied.database_revision
    ))
}

/// Save the preview's conflicts, or clear the conflict file when there are
/// none. True when a conflict file was saved.
fn save_or_clear_conflicts(store: &Store, preview: &PackageImportPreview) -> Result<bool, Failure> {
    if preview.conflicts.is_empty() {
        clear_conflict_set(store)?;
        return Ok(false);
    }
    Ok(save_conflict_set(store, preview)?.is_some())
}

/// `renderImportPreview` from 2.1.0.
pub fn render_package_preview(preview: &PackageImportPreview) -> String {
    let mut lines = vec![
        format!("{} preview", if preview.restore { "restore" } else { "merge" }),
        format!("  source database:      {} revision {}", preview.source_database_id, preview.source_database_revision),
        format!(
            "  destination database: {} revision {}",
            preview.destination_database_id, preview.destination_database_revision
        ),
        format!("  new records:          {}", preview.new_record_count),
        format!("  matching records:     {}", preview.matching_record_count),
        format!("  changed records:      {}", preview.changed_record_count),
        format!("  conflicts:            {}", preview.conflicts.len()),
        format!("  unrepresentable:       {}", preview.unrepresentable_records.len()),
        format!("  unknown fields:        {}", preview.unknown_fields.len()),
    ];
    if preview.record_lists_truncated {
        lines.push("  record ID lists:      first 1,000 per category".to_string());
    }
    for conflict in &preview.conflicts {
        let target = conflict.record_id.as_ref().map(|id| format!("{id}: ")).unwrap_or_default();
        lines.push(format!("    {target}{}", conflict.message));
    }
    lines.join("\n")
}

/// `ath conflicts show [record-id]`: always JSON.
pub fn conflicts_show(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let record_id = args.argument("record-id").map(ToString::to_string);
    with_store(context, args, Open::ReadOnly, |context, store, _| {
        let shown = show_conflicts(store, record_id.as_deref())?;
        context.say_json(&json::to_value(&shown))
    })
}

/// `ath conflicts resolve <record-id> --choice <choice>`: record one choice
/// in the conflict file. Athlete data does not change.
pub fn conflicts_resolve(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let record_id = required_argument(args, "record-id").to_string();
    let choice_text = args.value("choice").expect("the parser requires --choice");
    let data = args.value("data");
    if !["local", "imported", "corrected"].contains(&choice_text) {
        return Err(refuse("--choice must be local, imported, or corrected"));
    }
    if choice_text == "corrected" && data.is_none() {
        return Err(refuse("--data is required when --choice is corrected"));
    }
    if choice_text != "corrected" && data.is_some() {
        return Err(refuse("--data is used only when --choice is corrected"));
    }
    let choice = match (choice_text, data) {
        ("corrected", Some(text)) => ConflictChoice::Corrected { data: parse_data(text)? },
        ("imported", _) => ConflictChoice::Imported,
        _ => ConflictChoice::Local,
    };
    let json = args.flag("json");
    with_store(context, args, Open::Write, |context, store, _| {
        let result = resolve_conflict(store, &record_id, choice)?;
        if json {
            return context.say_json(&json::to_value(&result));
        }
        let unresolved = result.unresolved_conflicts;
        context.say(&format!(
            "recorded {} for {}; {unresolved} unresolved conflict{}",
            result.choice,
            result.record_id,
            if unresolved == 1 { "" } else { "s" }
        ))?;
        context.say(&format!(
            "nothing written to athlete data; run ath import {} --yes after all conflicts are resolved",
            result.source_path
        ))
    })
}

/// `--data <json>`, refused with the reader's reason when it is not JSON.
pub fn parse_data(text: &str) -> Result<Value, Failure> {
    json::parse(text).map_err(|error| {
        let reason = error.message.strip_prefix("Invalid JSON: ").unwrap_or(&error.message).to_string();
        refuse(format!("--data is not valid JSON: {reason}"))
    })
}

/// `ath erase <record-id> --revision <n>`: preview, two confirmations, then
/// a permanent erasure of the record, its dependents, and their files.
pub fn erase(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let reason_text = args.value("reason").expect("--reason has a default");
    let reason = ErasureReasonCode::parse(reason_text).ok_or_else(|| {
        refuse(format!("unknown erasure reason '{reason_text}'. Use {}", ErasureReasonCode::NAMES.join(", ")))
    })?;
    let record_id = required_argument(args, "record-id").to_string();
    let expected = args.positive_integer("revision").expect("the parser requires --revision");
    let flags = confirm_flags(args);
    with_store(context, args, Open::Write, |context, store, path| {
        let preview = preview_erasure(store, &record_id, expected.get())?;
        context.say_json(&object(vec![
            ("file", Value::String(path.display().to_string())),
            ("root_record_id", Value::String(preview.root_record_id.clone())),
            ("expected_revision", Value::from(preview.root_expected_revision)),
            ("database_revision", Value::from(preview.database_revision)),
            ("permanent", Value::Bool(true)),
            ("records", json::to_value(&preview.records)),
            ("attachments", json::to_value(&preview.attachments)),
            ("reason_code", Value::String(reason.as_str().to_string())),
            ("write_confirmed", Value::Bool(false)),
        ]))?;
        if flags.json {
            return Ok(());
        }
        let count = preview.records.len();
        context.say(&format!(
            "WARNING: this permanently removes {count} record{} and cannot be undone from the database.",
            if count == 1 { "" } else { "s" }
        ))?;
        if flags.dry_run {
            return context.say("nothing erased (--dry-run)");
        }
        if !context.confirm(flags, "Permanently erase these records?")? {
            return context.say("nothing erased");
        }
        let second = match args.value("confirm_erasure") {
            Some(value) => Some(value.to_string()),
            None if !context.io.terminal.stdin || !context.io.terminal.stdout => {
                return Err(refuse(format!(
                    "second confirmation is required in non-interactive use; pass --confirm-erasure {}",
                    preview.root_record_id
                )));
            }
            None => {
                context.ask(&format!("Type the record ID {} to confirm permanent erasure: ", preview.root_record_id))
            }
        };
        if second.as_deref() != Some(preview.root_record_id.as_str()) {
            return Err(refuse("second confirmation did not exactly match the record ID; nothing was erased"));
        }
        let erased = erase_record(store, &preview, reason, None)?;
        let records = erased.erased_record_ids.len();
        let files = erased.erased_attachment_count;
        context.say(&format!(
            "erased {records} record{} and {files} attachment file{} at database revision {}",
            if records == 1 { "" } else { "s" },
            if files == 1 { "" } else { "s" },
            erased.database_revision
        ))
    })
}
