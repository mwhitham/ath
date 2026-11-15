//! A one-line progress bar for long operations (S23).
//!
//! It draws on stderr, and only when stderr is a terminal, so a piped run
//! prints nothing extra and the result on stdout stays clean. It clears its
//! line when a step ends. No command drew progress in 2.1.0; long imports
//! receive [`Silent`] unless they choose this bar.

use std::io::Write;
use std::time::{Duration, Instant};

use crate::text::{grouped_count, js_to_fixed, pad_end};

/// How a step's amounts are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Count,
    Bytes,
}

/// Something that reports progress.
pub trait Progress {
    /// Begin a step. With no total, only the amount done is shown.
    fn start(&mut self, label: &str, total: Option<u64>, unit: Unit);
    /// Set how much of the step is done.
    fn update(&mut self, done: u64);
    /// Add to how much of the step is done.
    fn advance(&mut self, amount: u64);
    /// End the step and clear the line.
    fn finish(&mut self);
}

/// Reports nothing. The default everywhere.
#[derive(Debug, Default, Clone, Copy)]
pub struct Silent;

impl Progress for Silent {
    fn start(&mut self, _: &str, _: Option<u64>, _: Unit) {}
    fn update(&mut self, _: u64) {}
    fn advance(&mut self, _: u64) {}
    fn finish(&mut self) {}
}

const BAR_WIDTH: usize = 24;
const REDRAW_EVERY: Duration = Duration::from_millis(80);

/// A bar that redraws in place on a terminal stream.
pub struct TerminalBar<W: Write> {
    stream: W,
    /// The terminal width; values of 20 or less are treated as 80.
    columns: Option<usize>,
    label: String,
    total: Option<u64>,
    unit: Unit,
    done: u64,
    active: bool,
    last_drawn: Option<Instant>,
}

impl<W: Write> TerminalBar<W> {
    pub fn new(stream: W, columns: Option<usize>) -> Self {
        TerminalBar {
            stream,
            columns,
            label: String::new(),
            total: None,
            unit: Unit::Count,
            done: 0,
            active: false,
            last_drawn: None,
        }
    }

    /// The bar's text for the current step.
    fn line(&self) -> String {
        let format = |amount: u64| match self.unit {
            Unit::Bytes => format_bytes(amount),
            Unit::Count => grouped_count(amount),
        };
        let mut line = match self.total {
            Some(total) if total > 0 => {
                let ratio = (self.done as f64 / total as f64).min(1.0);
                let filled = (ratio * BAR_WIDTH as f64).round() as usize;
                let bar = "█".repeat(filled) + &"░".repeat(BAR_WIDTH - filled);
                let percent = format!("{:>4}", format!("{}%", (ratio * 100.0).floor() as u64));
                format!("{} {bar} {percent}   {} / {}", pad_end(&self.label, 24), format(self.done), format(total))
            }
            _ => format!("{} {}", pad_end(&self.label, 24), format(self.done)),
        };
        let width = self.columns.filter(|columns| *columns > 20).unwrap_or(80);
        if line.chars().count() >= width {
            line = line.chars().take(width.saturating_sub(1)).collect();
        }
        line
    }

    fn render(&mut self, force: bool) {
        if !self.active {
            return;
        }
        let now = Instant::now();
        if !force && self.last_drawn.is_some_and(|last| now.duration_since(last) < REDRAW_EVERY) {
            return;
        }
        self.last_drawn = Some(now);
        let line = self.line();
        let _ = write!(self.stream, "\r\x1b[2K{line}");
        let _ = self.stream.flush();
    }
}

impl<W: Write> Progress for TerminalBar<W> {
    fn start(&mut self, label: &str, total: Option<u64>, unit: Unit) {
        if self.active {
            self.finish();
        }
        self.label = label.to_string();
        self.total = total;
        self.unit = unit;
        self.done = 0;
        self.active = true;
        self.last_drawn = None;
        self.render(true);
    }

    fn update(&mut self, done: u64) {
        self.done = done;
        self.render(false);
    }

    fn advance(&mut self, amount: u64) {
        self.done += amount;
        self.render(false);
    }

    fn finish(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let _ = write!(self.stream, "\r\x1b[2K");
        let _ = self.stream.flush();
    }
}

/// `formatBytes`: GB, MB, and kB use powers of ten.
fn format_bytes(amount: u64) -> String {
    let value = amount as f64;
    if value >= 1e9 {
        format!("{} GB", js_to_fixed(value / 1e9, 1))
    } else if value >= 1e6 {
        format!("{} MB", js_to_fixed(value / 1e6, 1))
    } else if value >= 1e3 {
        format!("{} kB", js_to_fixed(value / 1e3, 0))
    } else {
        format!("{amount} B")
    }
}

/// The reporter for the current process: the bar on a terminal stderr,
/// otherwise nothing.
pub fn for_stderr(terminal: super::runtime::Terminal) -> Box<dyn Progress> {
    if terminal.stderr {
        Box::new(TerminalBar::new(std::io::stderr(), terminal.stderr_columns))
    } else {
        Box::new(Silent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_draws_and_clears() {
        let mut out = Vec::new();
        {
            let mut bar = TerminalBar::new(&mut out, Some(100));
            bar.start("Reading export", Some(2_000), Unit::Count);
            bar.finish();
        }
        let text = String::from_utf8(out).unwrap();
        let expected_bar = "░".repeat(24);
        assert_eq!(text, format!("\r\x1b[2KReading export           {expected_bar}   0%   0 / 2,000\r\x1b[2K"));
    }

    #[test]
    fn bytes_and_narrow_lines() {
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(1_500), "2 kB");
        assert_eq!(format_bytes(1_250_000_000), "1.3 GB");
        let mut bar = TerminalBar::new(Vec::new(), Some(30));
        bar.start("a label that is long enough to be cut", None, Unit::Count);
        assert_eq!(bar.line().chars().count(), 29);
    }
}
