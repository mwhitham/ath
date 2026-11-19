//! The `ath` command line: arguments, help, confirmations, and rendering.
//!
//! [`run`] is the single entry point. It parses the words, then either prints
//! the version, help, or a parse error, or calls the one handler for the
//! command. Handlers take parsed arguments and a [`Context`], which carries
//! the standard streams, terminal facts, working directory, environment, and
//! the library services a command calls. Tests run commands in-process with
//! in-memory streams.

pub mod commands;
pub mod help;
pub mod parse;
pub mod progress;
pub mod runtime;
pub mod spec;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, IsTerminal};

pub use parse::{Args, Outcome, Parsed, PositiveInteger};
pub use runtime::{CommandResult, Confirm, Context, Failure, Io, Services, Terminal};

/// Run `ath` with the words after the program name. Returns the exit status.
pub fn run(argv: &[String], io: Io<'_>, services: Services) -> i32 {
    let mut context = Context { io, services };
    let status = match parse::parse(argv) {
        Parsed::Done(outcome) => finish(&mut context, outcome),
        Parsed::Run(args) => {
            note_older_tool_skill(&mut context, &args.path);
            match commands::handler(&args.path)(&mut context, &args) {
                Ok(()) => 0,
                Err(Failure::Refused(message)) | Err(Failure::NotConnected(message)) => {
                    context.note(&format!("ath: {message}"));
                    1
                }
                Err(Failure::Exit(status)) => status,
            }
        }
    };
    let _ = context.io.stdout.flush();
    let _ = context.io.stderr.flush();
    status
}

/// Say once, on standard error, when this folder's tool skill is older than
/// this `ath` (D253). It also prints on `--json` runs, which agents use; the
/// JSON on standard output is unchanged. It is skipped for bare `ath` and
/// `ath --update`, `init`, and `skill`, which show or replace the copy.
fn note_older_tool_skill(context: &mut Context<'_>, path: &[&str]) {
    if matches!(path, [] | ["init"] | ["skill", ..]) {
        return;
    }
    if let Some(notice) = crate::assets::skill::stale_skill_warning(&context.io.cwd) {
        context.note(&notice);
    }
}

/// Print what the words asked for when no command runs.
fn finish(context: &mut Context<'_>, outcome: Outcome) -> i32 {
    match outcome {
        Outcome::Version => {
            let _ = writeln!(context.io.stdout, "{}", runtime::VERSION);
            0
        }
        Outcome::Help { path, error } => {
            let terminal = context.io.terminal;
            let columns = if error { terminal.stderr_columns } else { terminal.stdout_columns };
            let text = help::render(&path, columns.unwrap_or(help::DEFAULT_HELP_WIDTH));
            if error {
                let _ = context.io.stderr.write_all(text.as_bytes());
                1
            } else {
                let _ = context.io.stdout.write_all(text.as_bytes());
                0
            }
        }
        Outcome::CommanderError(message) => {
            context.note(&message);
            1
        }
        Outcome::ValueRefused(message) => {
            context.note(&format!("ath: {message}"));
            1
        }
        Outcome::Run(_) => unreachable!("a command that runs is handled by its handler"),
    }
}

/// Run `ath` as this process: real streams, terminal facts, working
/// directory, and environment. Returns the exit status.
pub fn main() -> i32 {
    let argv: Vec<String> = std::env::args_os().skip(1).map(|word| word.to_string_lossy().into_owned()).collect();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let stdin = std::io::stdin();
    let terminal = Terminal {
        stdin: stdin.is_terminal(),
        stdout: stdout.is_terminal(),
        stderr: stderr.is_terminal(),
        stdout_columns: columns(&stdout),
        stderr_columns: columns(&stderr),
    };
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("ath: {error}");
            return 1;
        }
    };
    let env: BTreeMap<String, String> = std::env::vars_os()
        .map(|(key, value)| (key.to_string_lossy().into_owned(), value.to_string_lossy().into_owned()))
        .collect();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    let mut input = stdin.lock();
    let mut open_terminal = open_controlling_terminal;
    let io = Io {
        stdout: &mut out,
        stderr: &mut err,
        stdin: &mut input,
        terminal,
        open_terminal: &mut open_terminal,
        cwd,
        env,
    };
    run(&argv, io, Services::default())
}

/// The width of a stream that is a terminal.
#[cfg(any(unix, windows))]
fn columns<S: IsTerminal + TerminalHandle>(stream: &S) -> Option<usize> {
    if !stream.is_terminal() {
        return None;
    }
    stream.size().map(|(terminal_size::Width(width), _)| usize::from(width))
}

#[cfg(not(any(unix, windows)))]
fn columns<S>(_: &S) -> Option<usize> {
    None
}

/// A standard stream whose terminal size can be read.
#[cfg(any(unix, windows))]
trait TerminalHandle {
    fn size(&self) -> Option<(terminal_size::Width, terminal_size::Height)>;
}

#[cfg(unix)]
impl<T: std::os::fd::AsFd> TerminalHandle for T {
    fn size(&self) -> Option<(terminal_size::Width, terminal_size::Height)> {
        terminal_size::terminal_size_of(self.as_fd())
    }
}

#[cfg(windows)]
impl<T: std::os::windows::io::AsHandle> TerminalHandle for T {
    fn size(&self) -> Option<(terminal_size::Width, terminal_size::Height)> {
        terminal_size::terminal_size_of(self.as_handle())
    }
}

/// The controlling terminal, for answers when standard input is a pipe (S19).
fn open_controlling_terminal() -> Option<Box<dyn BufRead>> {
    #[cfg(windows)]
    const TERMINAL: &str = "CONIN$";
    #[cfg(not(windows))]
    const TERMINAL: &str = "/dev/tty";
    std::fs::File::open(TERMINAL).ok().map(|file| Box::new(BufReader::new(file)) as Box<dyn BufRead>)
}
