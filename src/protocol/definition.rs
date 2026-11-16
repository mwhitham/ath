//! Operation definitions: the single list the worker dispatches on and the
//! TypeScript client is generated from.
//!
//! Each definition names the operation, how the client calls it, which
//! database access it needs, and the TypeScript text of its input, output,
//! streamed items, and callbacks. TypeScript names such as `CurrentRecordT`
//! refer to the npm package's compatibility schema module, which the generated
//! file imports.

use super::worker::{Call, Outcome};

/// How the JavaScript client waits for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A promise. The worker keeps standard input open for callbacks and
    /// cancellation.
    Async,
    /// A synchronous child process. Used only where the 2.1.0 export is
    /// synchronous. No database, callback, or streamed item is allowed.
    Sync,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Async => "async",
            Kind::Sync => "sync",
        }
    }
}

/// The database access an operation needs from the request's `target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetUse {
    /// No database. Operations that open or create a database by path name
    /// the path in their input.
    None,
    /// A read-only connection. Reads never change the file.
    Read,
    /// A read-write connection. Writes run in one transaction.
    Write,
}

impl TargetUse {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetUse::None => "none",
            TargetUse::Read => "read",
            TargetUse::Write => "write",
        }
    }
}

/// A function the client supplies and the worker calls during an operation.
#[derive(Debug, Clone, Copy)]
pub struct Callback {
    pub name: &'static str,
    /// TypeScript type of the value the worker sends.
    pub input: &'static str,
    /// TypeScript type of the value the client returns.
    pub output: &'static str,
}

/// The handler that runs an operation.
pub type Handler = fn(&mut Call<'_>) -> Outcome;

/// One protocol operation.
#[derive(Clone, Copy)]
pub struct Operation {
    /// The transport name, such as `records.read`.
    pub name: &'static str,
    pub kind: Kind,
    pub target: TargetUse,
    /// TypeScript type of `input`.
    pub input: &'static str,
    /// TypeScript type of the final `output`.
    pub output: &'static str,
    /// TypeScript type of each streamed item, when the operation streams.
    pub items: Option<&'static str>,
    pub callbacks: &'static [Callback],
    pub run: Handler,
}

impl std::fmt::Debug for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Operation").field("name", &self.name).finish()
    }
}

/// Every operation, in the order the generated client lists them.
pub static OPERATIONS: std::sync::LazyLock<Vec<Operation>> = std::sync::LazyLock::new(super::ops::all);

/// The operation with this transport name.
pub fn find(name: &str) -> Option<&'static Operation> {
    OPERATIONS.iter().find(|operation| operation.name == name)
}
