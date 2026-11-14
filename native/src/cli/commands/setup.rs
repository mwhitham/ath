//! `ath skill`, `ath skill install`, `ath migrate`, `ath repair`, and
//! `ath --update`: the set-up commands that do not create a database.

use super::shared::{confirm_flags, file_or_argument, CommandEnvironment};
use crate::assets::skill;
use crate::cli::parse::Args;
use crate::cli::runtime::{refuse, CommandResult, Context, Failure, VERSION};
use crate::json;
use crate::runtime::update::{self, InstallContext, Stream, UpdateRequest};
use crate::store::migrate::{inspect_migration, MigrationOptions};
use crate::store::repair::{execute_confirmed_repair, repair_preview, RepairOptions};

/// `ath skill`: where the skill is installed and whether each copy is current.
pub fn skill_status(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let cwd = context.io.cwd.clone();
    if args.flag("json") {
        return context.say_json(&json::to_value(&skill::skill_status_report(&cwd)?));
    }
    let copies = skill::skill_status(&cwd)?;
    context.say(&skill::render_skill_status(&cwd, &copies))
}

/// `ath skill install`: copy the tool skill, calculation skills, and mapping
/// files that are missing, and replace an older tool skill. The shipped
/// files are compiled into this program, so the 2.1.0 refusal for a missing
/// skill folder cannot occur.
pub fn skill_install(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let cwd = context.io.cwd.clone();
    let result = skill::install_skill_and_mappings(&cwd)?;
    // `--json` on `skill` or `skill install` selects JSON, as in 2.1.0.
    if args.flag_on_path("json") {
        return context.say_json(&json::to_value(&result));
    }
    for path in &result.mappings_installed {
        context.say(&format!("added mapping file {}", path.display()))?;
    }
    if result.skill_installed.is_empty() {
        return context.say("left installed skill files unchanged");
    }
    for path in &result.skill_installed {
        context.say(&format!("added skill files into {}", path.display()))?;
    }
    context.say(&format!("version {}", result.shipped))
}

/// `ath migrate`: preview, then apply after a verified backup and the exact
/// database ID.
pub fn migrate(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let flags = confirm_flags(args);
    let path = context.find_database(file_or_argument(args))?;
    let inspected = {
        // The inspection opens the file read-only and releases it at once.
        let store = context.open_read_only(&path, flags.json)?;
        drop(store);
        inspect_migration(&path, None)?
    };
    let preview = &inspected.preview;
    context.say_json(&json::to_value(&inspected))?;
    if flags.json {
        return Ok(());
    }
    if !preview.migration_needed {
        return context.say("no migration is required");
    }
    if preview.planned_steps.is_empty() {
        return Err(refuse(preview.compatibility_effect.clone()));
    }
    if flags.dry_run {
        return context.say("nothing migrated (--dry-run)");
    }
    if !flags.yes {
        return context.say("nothing migrated; pass --yes after reviewing this preview");
    }
    let second = match args.value("confirm_migration") {
        Some(value) => Some(value.to_string()),
        None if !context.io.terminal.stdin || !context.io.terminal.stdout => {
            return Err(refuse(format!(
                "second confirmation is required in non-interactive use; pass --confirm-migration {}",
                preview.database_id
            )));
        }
        None => context.ask(&format!("Type the database ID {} to confirm migration: ", preview.database_id)),
    };
    if second.as_deref() != Some(preview.database_id.as_str()) {
        return Err(refuse("second confirmation did not exactly match the database ID; nothing was migrated"));
    }
    let mut store = context.open_outdated_for_write(&path, false)?;
    let options = MigrationOptions {
        backup_path: preview.planned_backup_path.as_ref().map(std::path::PathBuf::from),
        now_millis: None,
    };
    let result = crate::store::migrate::execute_confirmed_migration(&mut store, &options);
    let closed = store.close();
    let result = result?;
    closed?;
    context.say(&format!(
        "migrated SQL schema {} to {} with verified backup at {}",
        preview.source_sql_schema_version,
        preview.target_sql_schema_version,
        result.backup_path.unwrap_or_default()
    ))
}

/// `ath repair`: preview typed-table repairs, then rebuild them from the
/// current revisions after a verified backup.
pub fn repair(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let flags = confirm_flags(args);
    let path = context.find_database(file_or_argument(args))?;
    let mut store = context.open_outdated_for_write(&path, flags.json)?;
    let result = repair_with(context, args, &mut store);
    let closed = store.close();
    result?;
    closed.map_err(Failure::from)
}

fn repair_with(context: &mut Context<'_>, args: &Args, store: &mut crate::store::Store) -> CommandResult {
    let flags = confirm_flags(args);
    let preview = repair_preview(store)?;
    context.say_json(&json::to_value(&preview))?;
    if flags.json {
        return Ok(());
    }
    if preview.repair_candidates == 0 && preview.revision_align_candidates == 0 {
        return context.say("no provider-encoding repair is required");
    }
    if flags.dry_run {
        return context.say("nothing repaired (--dry-run)");
    }
    if !flags.yes {
        return context.say("nothing repaired; pass --yes after reviewing this preview");
    }
    let database_id = store.metadata().database_id.clone();
    let from_revision = args.flag("from_revision");
    if preview.revision_align_candidates > 0 && !from_revision {
        return Err(refuse(format!(
            "{} record(s) need --from-revision and --confirm-revision-align {database_id}",
            preview.revision_align_candidates
        )));
    }
    if from_revision && preview.revision_align_candidates > 0 {
        match args.value("confirm_revision_align") {
            None => {
                return Err(refuse(format!(
                    "pass --confirm-revision-align {database_id} to align typed tables from the current revision"
                )))
            }
            Some(confirmation) if confirmation != database_id => {
                return Err(refuse("second confirmation did not exactly match the database ID; nothing was repaired"))
            }
            Some(_) => {}
        }
    }
    let result = execute_confirmed_repair(store, &RepairOptions { from_revision, ..RepairOptions::default() })?;
    let backup = if result.backup_path.is_empty() {
        String::new()
    } else {
        format!(" with verified backup at {}", result.backup_path)
    };
    let aligned = if result.revision_align_record_count > 0 {
        format!("; aligned {} record(s) from revision", result.revision_align_record_count)
    } else {
        String::new()
    };
    context.say(&format!(
        "repaired {} record(s); database revision {}{aligned}{backup}",
        result.repaired_record_count, result.database_revision
    ))
}

/// `ath --update`: classify the install and print the update service's
/// messages. A global npm, pnpm, or bun install gets a checked plan in the
/// launcher's handoff file; the launcher runs it after this process exits.
pub fn self_update(context: &mut Context<'_>) -> CommandResult {
    let install = InstallContext::from_environment(&CommandEnvironment(&context.io.env));
    let latest = context.services.latest_version;
    let cwd = context.io.cwd.clone();
    let outcome = update::run_update(&UpdateRequest {
        context: &install,
        current_version: VERSION,
        cwd: &cwd,
        latest_version: &latest,
    })?;
    for message in &outcome.messages {
        match message.stream {
            Stream::Stdout => context.say(&message.text)?,
            Stream::Stderr => context.note(&message.text),
        }
    }
    match outcome.exit_code {
        0 => Ok(()),
        status => Err(Failure::Exit(status)),
    }
}
