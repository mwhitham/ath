//! The JavaScript client protocol: a hidden, versioned stdio interface.
//!
//! The optional npm client calls library operations through this interface.
//! The native command line never uses it. It exposes typed domain operations
//! and checked data, not SQL or the old action interpreter.
//!
//! # Mode selection
//!
//! `ath __client` with no other argument runs one protocol worker instead of
//! the command line. `main` checks for that exact argument list before the
//! command line parses anything, so no command, option, or help text changes.
//! The name starts with two underscores so it cannot collide with a command.
//!
//! # One worker per operation
//!
//! Each client operation starts its own worker process. The worker reads one
//! request, runs one operation, writes its events, and exits. There is no
//! daemon, listening port, or background service.
//!
//! # Messages
//!
//! Every message is one line of JSON text. Requests and responses carry
//! `protocol` (the version, [`PROTOCOL_VERSION`]) and `id` (the request ID).
//!
//! The request is the first line on standard input:
//!
//! ```text
//! {"protocol":1,"id":"7","op":"records.read","input":{...},"target":{...}}
//! ```
//!
//! `target` names the database for operations that use one. The worker checks
//! the version before it reads anything else, so an incompatible client is
//! refused before any database is opened. Every request opens and checks its
//! database again, as a new open would: symbolic links, the file header, the
//! metadata, the SQL schema step, and the database ID the client expects.
//!
//! The worker writes events on standard output, in order:
//!
//! - `{"type":"progress","progress":...}`: optional progress.
//! - `{"type":"item","item":...}`: one element of a streamed list. Large
//!   results stream one element per line instead of one buffered document.
//! - `{"type":"callback","callback_id":"1","name":"complete","input":...}`:
//!   the operation asks the client to run a function it supplied. The worker
//!   waits for the client's answer on standard input:
//!   `{"type":"callback_result","callback_id":"1","output":...}` or
//!   `{"type":"callback_error","callback_id":"1","error":{"message":"..."}}`.
//! - `{"type":"result","output":...,"database_id":...}`: the last event of a
//!   successful operation. `database_id` is present when a database was used.
//! - `{"type":"error","error":{"code":...,"message":...}}`: the last event of a
//!   failed operation. Validation errors add `issues`; a migration refusal adds
//!   `preview`; a package refusal adds the package facts.
//!
//! # Backpressure and cancellation
//!
//! Events go through a bounded buffer to a pipe. When the client stops reading,
//! the pipe fills and the worker waits; nothing grows without limit. An
//! asynchronous operation keeps standard input open until its last event. A
//! `{"type":"cancel"}` line or the end of standard input before the last event
//! cancels it: the worker writes a `cancelled` error and exits at once. An
//! uncommitted SQLite transaction is then discarded, as it is when any process
//! stops during a write. Synchronous operations close standard input after the
//! request; for them the end of input means nothing.
//!
//! # Generated client
//!
//! [`definition::OPERATIONS`] lists every operation with its TypeScript input,
//! output, item, and callback types. [`typescript::generate`] writes the npm
//! client's request and response types and its dispatch table from that list,
//! so transport names are not maintained by hand in a second place.

pub mod definition;
mod ops;
pub mod typescript;
pub mod worker;

pub use definition::{Kind, Operation, TargetUse, OPERATIONS};
pub use worker::serve;

/// The protocol version. A request naming another version is refused.
pub const PROTOCOL_VERSION: u32 = 1;

/// The sole command-line argument that selects a protocol worker.
pub const MODE_ARGUMENT: &str = "__client";

/// Exit status after a result.
pub const EXIT_OK: i32 = 0;
/// Exit status after an operation error.
pub const EXIT_FAILED: i32 = 1;
/// Exit status when the request itself is unreadable or names another version.
pub const EXIT_PROTOCOL: i32 = 2;
/// Exit status after cancellation.
pub const EXIT_CANCELLED: i32 = 130;

/// True when these command-line words select a protocol worker.
pub fn selects_worker<S: AsRef<std::ffi::OsStr>>(words: &[S]) -> bool {
    words.len() == 1 && words[0].as_ref() == std::ffi::OsStr::new(MODE_ARGUMENT)
}
