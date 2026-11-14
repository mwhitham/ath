//! One handler per command.
//!
//! A handler reads its parsed [`Args`], calls the library, and renders the
//! result through the [`Context`]. It returns a [`Failure`] instead of
//! printing an error, so a refusal is printed once (quirk S4 is not kept).
//!
//! Commands whose library operations are still being built route to
//! [`not_connected`]. Connecting one means adding its handler to
//! [`handler`].

mod edit;
mod init;

use super::parse::Args;
use super::runtime::{CommandResult, Context, Failure};

/// The signature every command handler has.
pub type Handler = fn(&mut Context<'_>, &Args) -> CommandResult;

/// The handler for a command path, such as `["records", "show"]`.
pub fn handler(path: &[&str]) -> Handler {
    match path {
        [] => root,
        ["init"] => init::run,
        ["link"] => edit::link,
        ["correct"] => edit::correct,
        ["delete"] => edit::delete,
        _ => not_connected,
    }
}

/// Command paths whose handlers are connected to the native library.
pub const CONNECTED: &[&[&str]] = &[&[], &["link"], &["correct"], &["delete"]];

/// A command whose library operation is not connected to this executable
/// yet. It reads and writes nothing.
pub fn not_connected(_: &mut Context<'_>, args: &Args) -> CommandResult {
    let name = if args.path.is_empty() { "ath".to_string() } else { format!("ath {}", args.path.join(" ")) };
    Err(Failure::NotConnected(format!(
        "{name} is not connected to the native library yet; nothing was read or written"
    )))
}

/// Bare `ath` prints a short guide (section 2). `--update` belongs to the
/// update service, which is not connected yet.
fn root(context: &mut Context<'_>, args: &Args) -> CommandResult {
    if args.flag("update") {
        return Err(Failure::NotConnected(
            "ath --update is not connected to the native library yet; nothing was changed".to_string(),
        ));
    }
    let guide = super::help::bare_guide(&context.io.cwd);
    context.say(&guide)
}
