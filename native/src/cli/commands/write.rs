//! The writing commands connected here: `log`, `grade`, and provider
//! `import`. Each previews, stops for `--json` or `--dry-run`, and writes
//! one change after confirmation.

use super::shared::{confirm_flags, proceed, required_argument, with_store, Open};
use crate::cli::parse::Args;
use crate::cli::runtime::{refuse, CommandResult, Context, Failure};
use crate::ids::new_record_id;
use crate::import::{self, ImportOptions, ImportRoute, ProviderPreview};
use crate::json;
use crate::log::{add_attachments, plan_log_entry, render_log_plan, LogRecord, LogRequest};
use crate::model::{time, RecordActor};
use crate::package::{apply_with_attachments, prepare_attachments};
use crate::prediction::grade::{grade_prediction, plan_prediction_grade, tool_actor, GradeRequest};
use std::path::PathBuf;

/// `ath log`: one plain-text record or one validated structured batch.
pub fn log(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let flags = confirm_flags(args);
    with_store(context, args, Open::Write, |context, store, path| {
        let words = args.arguments("entry");
        let text = if words.is_empty() { context.read_stdin()? } else { words.join(" ") };
        let actor = match args.value("agent") {
            None => RecordActor::Athlete,
            Some(agent) => RecordActor::Agent { agent: agent.to_string(), agent_version: None },
        };
        let request = LogRequest {
            text,
            actor,
            created_at: args.value("at").map(ToString::to_string),
            benchmark: args.value("benchmark").map(ToString::to_string),
        };
        let plan = plan_log_entry(store.connection(), &request, &mut new_record_id)?;
        // Each attachment file is read and checked now; it is written only
        // with the records, and linked to the batch's one report or entry.
        let paths: Vec<PathBuf> = args.values("attachment").iter().map(|path| context.resolve(path)).collect();
        let created_at = plan.inputs.first().map_or_else(time::now_utc, |input| input.created_at.clone());
        let prepared = prepare_attachments(store, &paths, &request.actor, &created_at)?;
        let records = prepared
            .iter()
            .map(|file| {
                LogRecord::attachment(
                    file.record_id.clone(),
                    file.created_at.clone(),
                    file.changed_by.clone(),
                    file.data.clone(),
                )
            })
            .collect();
        let plan = add_attachments(plan, records)?;
        if flags.json {
            return context.say_json(&json::to_value(&plan.preview(&path.display().to_string())));
        }
        context.say(&render_log_plan(&plan))?;
        if flags.dry_run {
            return context.say("nothing written (--dry-run)");
        }
        if !context.confirm(flags, "Write these records?")? {
            return context.say("nothing written");
        }
        let written = apply_with_attachments(store, &prepared, plan.changes())?;
        let count = written.records.len();
        context.say(&format!(
            "wrote {count} record{} at database revision {}",
            if count == 1 { "" } else { "s" },
            written.database_revision
        ))?;
        for record in &written.records {
            context.say(&format!("  {}  {}", record.record_id, record.record_type()))?;
        }
        Ok(())
    })
}

/// `ath grade <prediction-id> --result <record-id>`.
pub fn grade(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let flags = confirm_flags(args);
    let request = GradeRequest {
        prediction_id: required_argument(args, "prediction-id").to_string(),
        result_id: args.value("result").expect("the parser requires --result").to_string(),
        actor: tool_actor(),
        calculation_version: args.value("calculation_version").map(ToString::to_string),
        created_at: None,
    };
    with_store(context, args, Open::Write, |context, store, path| {
        let plan = plan_prediction_grade(store.connection(), &request, &mut new_record_id)?;
        context.say_json(&json::to_value(&plan.preview(&path.display().to_string())))?;
        if flags.json || !proceed(context, flags, "Write this grade?")? {
            return Ok(());
        }
        let written = grade_prediction(store, &plan)?;
        context.say(&written.summary_line())?;
        context.say(&format!("grade {} at database revision {}", written.grade.record_id, written.database_revision))
    })
}

/// `ath import <path>`. Provider exports and packages are connected. Mapped
/// JSON files and Garmin and Strava inputs have their own library
/// operations, which are not connected to this executable yet.
pub fn import(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let athlete_path = context.find_database(args.value("file"))?;
    let input = context.resolve(required_argument(args, "path"));
    let route = import::classify(&input, args.value("mapping").is_some())?;
    let not_connected = |what: &str| {
        Err(Failure::NotConnected(format!(
            "ath import of {what} is not connected to the native library yet; nothing was read or written"
        )))
    };
    match route {
        ImportRoute::Mapped => return not_connected("a file with --mapping"),
        ImportRoute::Shared(shared) => return not_connected(&format!("{} data", shared.as_str())),
        ImportRoute::Package => {
            return super::shared::with_store_at(context, &athlete_path, args.flag("json"), Open::Write, |c, s, _| {
                super::package::import_package(c, s, &input, args)
            })
        }
        ImportRoute::Provider => {}
    }
    let json = args.flag("json");
    let yes = args.flag("yes");
    super::shared::with_store_at(context, &athlete_path, json, Open::Write, |context, store, _| {
        let preview = import::preview(store, &input, &ImportOptions::default())?;
        if json {
            return context.say_json(&json::to_value(&preview));
        }
        context.say(&render_provider_preview(&preview))?;
        if !yes {
            return context.say("nothing written; pass --yes after reviewing this preview");
        }
        let conflicts = preview.conflicts.len();
        if conflicts > 0 {
            return Err(refuse(format!(
                "import has {conflicts} unresolved conflict{}; nothing was written",
                if conflicts == 1 { "" } else { "s" }
            )));
        }
        let imported = preview.apply(store)?;
        context.say(&format!(
            "imported {}: {} new, {} changed, {} matching at database revision {}",
            preview.imported,
            imported.new_record_count,
            imported.changed_record_count,
            imported.matching_record_count,
            imported.database_revision
        ))
    })
}

/// `renderProviderImportPreview` from 2.1.0.
pub fn render_provider_preview(preview: &ProviderPreview) -> String {
    let skipped: u64 = preview.skipped.iter().map(|(_, skipped)| skipped.count).sum();
    let mut lines = vec![
        format!("{} preview", preview.imported),
        format!("  destination revision: {}", preview.destination_database_revision),
        format!("  new records:         {}", preview.new_record_count()),
        format!("  matching records:    {}", preview.matching_record_count()),
        format!("  changed records:     {}", preview.changed_record_count()),
        format!("  conflicts:           {}", preview.conflicts.len()),
        format!("  skipped rows:        {skipped}"),
    ];
    if preview.route_points_stored > 0 {
        lines.push(format!("  route points stored: {}", preview.route_points_stored));
    }
    for conflict in &preview.conflicts {
        let target = conflict.record_id.as_ref().map(|id| format!("{id}: ")).unwrap_or_default();
        lines.push(format!("    {target}{}", conflict.message));
    }
    for (reason, skipped) in &preview.skipped {
        let example = skipped.example.as_ref().map(|example| format!(" (example: {example})")).unwrap_or_default();
        lines.push(format!("    skipped {} — {reason}{example}", skipped.count));
    }
    lines.join("\n")
}
