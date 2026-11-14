//! The skill's command reference, built from the command tree's own help (D80).
//!
//! `skills/athletic-standard/references/cli.md` is generated from this text and
//! committed. A check fails when the committed file is behind the commands.
//! Regenerate it; do not edit it by hand.

use clap::Command;

/// Fixed so the output does not depend on the terminal it was generated in.
pub const HELP_WIDTH: usize = 80;

struct HelpEntry {
    /// `ath key set`
    path: String,
    summary: String,
    help: String,
}

fn help_text(command: &Command) -> String {
    let mut command = command.clone();
    command.render_help().to_string().trim_end().to_string()
}

fn walk(command: &Command, prefix: &str, out: &mut Vec<HelpEntry>) {
    for sub in command.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        let path = format!("{prefix} {}", sub.get_name());
        let summary = sub.get_about().map(|about| about.to_string()).unwrap_or_default();
        out.push(HelpEntry { path: path.clone(), summary, help: help_text(sub) });
        walk(sub, &path, out);
    }
}

fn built(program: &Command) -> Command {
    let mut program = program.clone().term_width(HELP_WIDTH).max_term_width(HELP_WIDTH);
    program.build();
    program
}

/// Every command in the tree, such as `ath key set`, in definition order.
/// Implicit `help` subcommands are left out.
pub fn command_paths(program: &Command) -> Vec<String> {
    let program = built(program);
    let mut entries = Vec::new();
    walk(&program, program.get_name(), &mut entries);
    entries.into_iter().map(|entry| entry.path).collect()
}

/// The complete `references/cli.md` text.
///
/// The root section shows the root help without its trailing text. Each
/// command section shows the complete help, including examples.
pub fn render_cli_reference(program: &Command) -> String {
    let program = built(program);
    let mut entries = Vec::new();
    walk(&program, program.get_name(), &mut entries);

    let anchor = |path: &str| path.split_whitespace().collect::<Vec<_>>().join("-");
    let mut root = program.clone().after_help(None::<&str>).after_long_help(None::<&str>);
    let root_help = root.render_help().to_string().trim_end().to_string();

    let mut lines: Vec<String> = vec![
        "# ath command reference".into(),
        String::new(),
        "Generated from the CLI's own help by `pnpm generate:skill-cli`. Do not edit by hand.".into(),
        "This is the text `ath <command> --help` prints, for every command, so an agent".into(),
        "can read the whole surface without a round trip.".into(),
        String::new(),
        "Every command takes `--json` where output is meant for an agent. Commands that".into(),
        "write show a summary and ask once; `-y` answers yes in advance. Where `--dry-run`".into(),
        "is offered, it shows the write and writes nothing.".into(),
        String::new(),
        "## Contents".into(),
        String::new(),
        "- [ath](#ath)".into(),
    ];
    for entry in &entries {
        lines.push(format!("- [{}](#{}) — {}", entry.path, anchor(&entry.path), entry.summary));
    }
    lines.extend([String::new(), "## ath".into(), String::new(), "```".into(), root_help, "```".into()]);
    for entry in entries {
        lines.extend([
            String::new(),
            format!("## {}", entry.path),
            String::new(),
            "```".into(),
            entry.help,
            "```".into(),
        ]);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}
