//! The read commands: `check`, `stats`, `records list`, `records show`,
//! `series`, `calculate`, and `apply`. They open the database read-only and
//! never change it.

use super::shared::{file_or_argument, required_argument, with_store, with_store_at, Open};
use crate::calculate::{self, CalculateOptions, Group, Operation, RecordKind};
use crate::cli::parse::Args;
use crate::cli::runtime::{refuse, CommandResult, Context, Failure};
use crate::json;
use crate::model::{ProviderReportingPeriodType, RecordType, SeriesQuantity, ATHLETIC_STANDARD_VERSION};
use crate::store::check::{self, Severity};
use crate::store::read::ListOptions;
use crate::store::stats;
use crate::store::{OpenOptions, Store};
use crate::view::records::{self, ShowOptions};
use crate::view::series::{self, SeriesViewOptions};

/// A positive-integer option as a `u32` limit. Larger values saturate; the
/// library refuses any limit above its maximum.
fn limit(args: &Args) -> u32 {
    let value = args.positive_integer("limit").expect("--limit has a default").get();
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// `ath check`. `--validate-export` needs the package module.
pub fn check(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let json = args.flag("json");
    let path = context.find_database(file_or_argument(args))?;
    if args.flag("validate_export") {
        return Err(Failure::NotConnected(
            "ath check --validate-export is not connected to the native library yet; nothing was read or written"
                .to_string(),
        ));
    }
    let mut valid = true;
    with_store_at(context, &path, json, Open::ReadOnly, |context, store, path| {
        let report = check::check(store, None)?;
        valid = report.result.valid;
        if json {
            return context.say_json(&json::to_value(&report));
        }
        for issue in &report.result.issues {
            let target = issue.record_id.as_ref().map(|id| format!(" {id}")).unwrap_or_default();
            let severity = match issue.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            context.note(&format!("{severity}  {}{target}: {}", issue.check, issue.message));
        }
        if report.result.valid {
            let version = report
                .result
                .metadata
                .as_ref()
                .map(|metadata| metadata.athleticstandard_version.clone())
                .unwrap_or_else(|| ATHLETIC_STANDARD_VERSION.to_string());
            context.say(&format!(
                "{}: valid Athletic Standard {version} database ({} records)",
                path.display(),
                report.result.record_count
            ))?;
        } else {
            let errors = report.result.issues.iter().filter(|issue| issue.severity == Severity::Error).count();
            context.note(&format!("\n{}: INVALID — {errors} error(s)", path.display()));
        }
        Ok(())
    })?;
    if valid {
        Ok(())
    } else {
        Err(Failure::Exit(1))
    }
}

/// `ath stats`.
pub fn stats(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let json = args.flag("json");
    let path = context.find_database(file_or_argument(args))?;
    with_store_at(context, &path, json, Open::ReadOnly, |context, store, _| {
        let stats = stats::database_stats(store)?;
        if json {
            context.say_json(&json::to_value(&stats))
        } else {
            context.say(&stats::render_database_stats(&stats))
        }
    })
}

/// `ath records list`.
pub fn records_list(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let record_type = match args.value("type") {
        None => None,
        Some(text) => Some(RecordType::parse(text).ok_or_else(|| {
            let names: Vec<&str> = RecordType::ALL.iter().map(|record_type| record_type.as_str()).collect();
            refuse(format!("unknown record type '{text}'. Known types: {}", names.join(", ")))
        })?),
    };
    let options = ListOptions { record_type, include_deleted: args.flag("include_deleted"), limit: limit(args) };
    let json = args.flag("json");
    with_store(context, args, Open::ReadOnly, |context, store, _| {
        let list = records::list_records(store, &options)?;
        if json {
            context.say_json(&json::to_value(&list))
        } else {
            context.say(&records::render_record_list(&list.records))
        }
    })
}

/// `ath records show`.
pub fn records_show(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let record_id = required_argument(args, "record-id").to_string();
    let options = ShowOptions {
        revision: args.positive_integer("revision").map(|revision| revision.get()),
        history: args.flag("history"),
        links: args.flag("links"),
    };
    let json = args.flag("json");
    with_store(context, args, Open::ReadOnly, |context, store, _| {
        let shown = records::show_record(store, &record_id, options)?;
        if json {
            context.say_json(&json::to_value(&shown))
        } else {
            context.say(&records::render_record(&shown))
        }
    })
}

/// `ath series`.
pub fn series(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let quantity_text = required_argument(args, "quantity").to_string();
    let quantity = SeriesQuantity::parse(&quantity_text).ok_or_else(|| {
        refuse(format!(
            "unknown quantity '{quantity_text}'. Known quantities: {}. `ath stats` lists the ones this database actually holds.",
            SeriesQuantity::NAMES.join(", ")
        ))
    })?;
    let source = args.value("source").map(ToString::to_string);
    let options = SeriesViewOptions {
        from: args.value("from").map(ToString::to_string),
        to: args.value("to").map(ToString::to_string),
        source_id: source.clone(),
        time_zone: args.value("timezone").map(ToString::to_string),
        raw: args.flag("raw"),
        limit: Some(limit(args)),
    };
    let json = args.flag("json");
    with_store(context, args, Open::ReadOnly, |context, store, _| {
        let view = series::read_series_view(store, quantity, &options)?;
        if view.row_count() == 0 {
            return Err(refuse(match &source {
                Some(source) => format!(
                    "no {quantity_text} samples from '{source}'. Drop --source or run `ath stats` for source IDs."
                ),
                None => format!("no {quantity_text} samples matched. Run `ath stats` to see the recorded series."),
            }));
        }
        if json {
            context.say_json(&json::to_value(&view))
        } else {
            context.say(&series::render_series_view(&view))
        }
    })
}

/// `ath calculate`. The names are checked before the database is found.
pub fn calculate(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let operation_text = required_argument(args, "operation");
    let operation = Operation::parse(operation_text)
        .ok_or_else(|| refuse(format!("unknown calculation '{operation_text}'. Use {}", Operation::names())))?;
    let group_text = args.value("group").expect("--group has a default");
    let group =
        Group::parse(group_text).ok_or_else(|| refuse("unknown --group value. Use none, day, week, or month"))?;
    let kind_text = args.value("record_kind").expect("--record-kind has a default");
    let record_kind = RecordKind::parse(kind_text)
        .ok_or_else(|| refuse(format!("unknown --record-kind '{kind_text}'. Use {}", RecordKind::names())))?;
    let provider_period_type = match args.value("period_type") {
        None => None,
        Some(text) => Some(ProviderReportingPeriodType::parse(text).ok_or_else(|| {
            refuse(format!("unknown --period-type '{text}'. Use {}", ProviderReportingPeriodType::NAMES.join(", ")))
        })?),
    };
    let text = |id: &str| args.value(id).map(ToString::to_string);
    let mut options = CalculateOptions::new(operation, required_argument(args, "quantity"));
    options.record_kind = record_kind;
    options.provider_period_type = provider_period_type;
    options.group = group;
    options.source_id = text("source");
    options.compare_source_id = text("compare_source");
    options.other_quantity = text("other_quantity");
    options.workout_session_id = text("session");
    options.from = text("from");
    options.to = text("to");
    options.time_zone = text("timezone");
    let json = args.flag("json");
    with_store(context, args, Open::ReadOnly, |context, store, _| {
        let receipt = calculate::calculate_range(store, &options)?;
        if json {
            context.say_json(&json::to_value(&receipt))
        } else {
            context.say(&calculate::render_calculation(&receipt))
        }
    })
}

/// `ath apply`: one calculation file on one record. The installed copy in
/// `.agents/skills` wins over the shipped copy. As in 2.1.0, the database is
/// read without the migration check or the file-permission warnings.
pub fn apply(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let name = required_argument(args, "calculation").to_string();
    let record_id = args.value("record").expect("the parser requires --record").to_string();
    let path = context.find_database(args.value("file"))?;
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true })?;
    let result = calculate::apply(&store, &name, &record_id, &context.io.cwd);
    let closed = store.close();
    let result = result?;
    closed?;
    if args.flag("json") {
        context.say_json(&json::to_value(&result))
    } else {
        context.say(&calculate::render_apply_result(&result))
    }
}
