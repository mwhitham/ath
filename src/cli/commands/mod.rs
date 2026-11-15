//! One handler per command.
//!
//! A handler reads its parsed [`Args`], calls the library, and renders the
//! result through the [`Context`]. It returns a [`Failure`] instead of
//! printing an error, so a refusal is printed once (quirk S4 is not kept).
//!
//! Commands whose library operations are still being built route to
//! [`not_connected`]. Connecting one means adding its handler to
//! [`handler`] and its path to [`CONNECTED`].

mod edit;
mod init;
mod model;
mod package;
mod read;
mod routes;
mod setup;
mod shared;
mod write;

pub use package::render_package_preview;
pub use write::render_provider_preview;

use super::parse::Args;
use super::runtime::{CommandResult, Context, Failure};

/// The signature every command handler has.
pub type Handler = fn(&mut Context<'_>, &Args) -> CommandResult;

/// The handler for a command path, such as `["records", "show"]`.
pub fn handler(path: &[&str]) -> Handler {
    match path {
        [] => root,
        ["init"] => init::run,
        ["skill"] => setup::skill_status,
        ["skill", "install"] => setup::skill_install,
        ["migrate"] => setup::migrate,
        ["repair"] => setup::repair,
        ["key"] => model::key_status,
        ["key", "set"] => model::key_set,
        ["key", "clear"] => model::key_clear,
        ["models"] => model::models,
        ["import"] => write::import,
        ["conflicts", "show"] => package::conflicts_show,
        ["conflicts", "resolve"] => package::conflicts_resolve,
        ["export"] => package::export,
        ["erase"] => package::erase,
        ["pull"] => routes::pull,
        ["log"] => write::log,
        ["link"] => edit::link,
        ["correct"] => edit::correct,
        ["delete"] => edit::delete,
        ["check"] => read::check,
        ["stats"] => read::stats,
        ["records", "list"] => read::records_list,
        ["records", "show"] => read::records_show,
        ["series"] => read::series,
        ["calculate"] => read::calculate,
        ["apply"] => read::apply,
        ["predict"] => model::predict,
        ["backtest"] => model::backtest,
        ["share"] => model::share,
        ["grade"] => write::grade,
        _ => not_connected,
    }
}

/// Command paths whose handlers are connected to the native library. An
/// Apple Health export is refused by the import library until its reader is
/// added.
pub const CONNECTED: &[&[&str]] = &[
    &[],
    &["init"],
    &["skill"],
    &["skill", "install"],
    &["migrate"],
    &["repair"],
    &["key"],
    &["key", "set"],
    &["key", "clear"],
    &["models"],
    &["import"],
    &["conflicts", "show"],
    &["conflicts", "resolve"],
    &["export"],
    &["erase"],
    &["pull"],
    &["log"],
    &["link"],
    &["correct"],
    &["delete"],
    &["check"],
    &["stats"],
    &["records", "list"],
    &["records", "show"],
    &["series"],
    &["calculate"],
    &["apply"],
    &["predict"],
    &["backtest"],
    &["share"],
    &["grade"],
];

/// A command whose library operation is not connected to this executable
/// yet. It reads and writes nothing.
pub fn not_connected(_: &mut Context<'_>, args: &Args) -> CommandResult {
    let name = if args.path.is_empty() { "ath".to_string() } else { format!("ath {}", args.path.join(" ")) };
    Err(Failure::NotConnected(format!(
        "{name} is not connected to the native library yet; nothing was read or written"
    )))
}

/// Bare `ath` prints a short guide (section 2). `--update` asks the update
/// service how this ath was installed and what to do.
fn root(context: &mut Context<'_>, args: &Args) -> CommandResult {
    if args.flag("update") {
        return setup::self_update(context);
    }
    let guide = super::help::bare_guide(&context.io.cwd);
    context.say(&guide)
}
