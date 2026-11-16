//! Small helpers every command module uses: reading the confirmation flags,
//! opening and releasing the database, building ordered JSON objects, and
//! the "nothing written" steps after a preview.

use std::path::Path;

use crate::cli::parse::Args;
use crate::cli::runtime::{CommandResult, Confirm, Context, Failure};
use crate::json::{Map, Value};
use crate::runtime::Environment;
use crate::store::Store;

/// The flags a previewing command reads (S18).
pub fn confirm_flags(args: &Args) -> Confirm {
    Confirm { yes: args.flag("yes"), dry_run: args.flag("dry_run"), json: args.flag("json") }
}

/// A positional argument the parser requires.
pub fn required_argument<'a>(args: &'a Args, name: &str) -> &'a str {
    args.argument(name).unwrap_or_else(|| panic!("the parser requires <{name}>"))
}

/// How a command opens its database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Open {
    /// `openOrFail` (S14, S15).
    Write,
    /// `openReadOnlyOrFail` (S16).
    ReadOnly,
}

/// Find the database (`--file`, else discovery), open it, run `body`, and
/// release the database afterwards whatever happened (S31).
pub fn with_store(
    context: &mut Context<'_>,
    args: &Args,
    open: Open,
    body: impl FnOnce(&mut Context<'_>, &mut Store, &Path) -> CommandResult,
) -> CommandResult {
    let path = context.find_database(args.value("file"))?;
    with_store_at(context, &path, args.flag("json"), open, body)
}

/// Open the database at `path`, run `body`, and release the database.
pub fn with_store_at(
    context: &mut Context<'_>,
    path: &Path,
    json: bool,
    open: Open,
    body: impl FnOnce(&mut Context<'_>, &mut Store, &Path) -> CommandResult,
) -> CommandResult {
    let mut store = match open {
        Open::Write => context.open_for_write(path, json)?,
        Open::ReadOnly => context.open_read_only(path, json)?,
    };
    let result = body(context, &mut store, path);
    let closed = store.close();
    result?;
    closed.map_err(Failure::from)
}

/// `--file`, else the positional `[file]` (S13).
pub fn file_or_argument(args: &Args) -> Option<&str> {
    args.value("file").or_else(|| args.argument("file"))
}

/// An object with fields in the given order.
pub fn object(fields: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in fields {
        map.insert(key.to_string(), value);
    }
    Value::Object(map)
}

/// The preview is shown; ask before writing. `Ok(false)` means stop.
pub fn proceed(context: &mut Context<'_>, flags: Confirm, question: &str) -> Result<bool, Failure> {
    if flags.dry_run {
        context.say("nothing written (--dry-run)")?;
        return Ok(false);
    }
    if !context.confirm(flags, question)? {
        context.say("nothing written")?;
        return Ok(false);
    }
    Ok(true)
}

/// The command's environment, for runtime services that read variables.
pub struct CommandEnvironment<'a>(pub &'a std::collections::BTreeMap<String, String>);

impl Environment for CommandEnvironment<'_> {
    fn var(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}
