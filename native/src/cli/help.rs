//! Help text in the layout of the 2.1.0 program.
//!
//! The old command line printed Commander's help. This module ports
//! Commander 15's `Help` formatter: the usage line, the wrapped description,
//! the `Arguments:`, `Options:`, and grouped command sections, the term
//! column, and the wrapping rules. It renders from the command table in
//! `spec.rs`, so help cannot drift from what the parser accepts.

use std::path::Path;

use super::spec::{CommandSpec, HELP_DESCRIPTION};
use super::text::{is_js_whitespace, js_length, pad_end};

/// Commander's width for output that is not a terminal.
pub const DEFAULT_HELP_WIDTH: usize = 80;

/// Commander does not wrap a column narrower than this.
const MIN_WIDTH_TO_WRAP: usize = 40;

/// Spaces before every item.
const ITEM_INDENT: usize = 2;

/// Spaces between a term and its description.
const SPACER_WIDTH: usize = 2;

/// The complete help for a command, as `ath <path> --help` prints it,
/// including the examples that follow the generated part.
pub fn render(path: &[&str], help_width: usize) -> String {
    let command = super::spec::find(path).expect("help is rendered only for commands in the table");
    let mut out = format_help(command, path, help_width);
    if let Some(examples) = command.examples {
        out.push('\n');
        out.push_str(examples);
    }
    out
}

struct Item {
    term: String,
    description: String,
}

/// One help section: a heading and its items.
struct Section {
    heading: &'static str,
    items: Vec<Item>,
}

/// `Help.formatHelp`.
fn format_help(command: &CommandSpec, path: &[&str], help_width: usize) -> String {
    let arguments = visible_arguments(command);
    let options = visible_options(command);
    let command_sections = command_sections(command);
    let term_width = arguments
        .iter()
        .chain(options.iter())
        .chain(command_sections.iter().flat_map(|section| section.items.iter()))
        .map(|item| js_length(&item.term))
        .max()
        .unwrap_or(0);

    let mut output = vec![format!("Usage: {}", command_usage(command, path)), String::new()];
    if !command.description.is_empty() {
        output.push(box_wrap(command.description, help_width));
        output.push(String::new());
    }
    let mut sections = Vec::new();
    if !arguments.is_empty() {
        sections.push(Section { heading: "Arguments:", items: arguments });
    }
    if !options.is_empty() {
        sections.push(Section { heading: "Options:", items: options });
    }
    sections.extend(command_sections);
    for section in sections {
        if section.items.is_empty() {
            continue;
        }
        output.push(section.heading.to_string());
        for item in &section.items {
            output.push(format_item(&item.term, term_width, &item.description, help_width));
        }
        output.push(String::new());
    }
    output.join("\n")
}

/// `Help.commandUsage` with Commander's default `usage()`.
fn command_usage(command: &CommandSpec, path: &[&str]) -> String {
    let mut words = vec!["ath".to_string()];
    words.extend(path.iter().map(ToString::to_string));
    let mut usage = vec!["[options]".to_string()];
    if !command.subcommands.is_empty() {
        usage.push("[command]".to_string());
    }
    usage.extend(command.arguments.iter().map(human_readable_argument));
    format!("{} {}", words.join(" "), usage.join(" "))
}

/// `humanReadableArgName`: `<name>`, `[name]`, or with `...` when variadic.
fn human_readable_argument(argument: &super::spec::ArgumentSpec) -> String {
    let name = if argument.variadic { format!("{}...", argument.name) } else { argument.name.to_string() };
    if argument.required {
        format!("<{name}>")
    } else {
        format!("[{name}]")
    }
}

/// Arguments are listed only when one of them has a description.
fn visible_arguments(command: &CommandSpec) -> Vec<Item> {
    if !command.arguments.iter().any(|argument| !argument.description.is_empty()) {
        return Vec::new();
    }
    command
        .arguments
        .iter()
        .map(|argument| Item { term: argument.name.to_string(), description: argument.description.to_string() })
        .collect()
}

/// The command's options, then the built-in help option.
fn visible_options(command: &CommandSpec) -> Vec<Item> {
    let mut items: Vec<Item> = command
        .options
        .iter()
        .map(|option| {
            let description = match option.default_display() {
                Some(default) if option.takes_value() => {
                    if option.description.is_empty() {
                        format!("(default: {default})")
                    } else {
                        format!("{} (default: {default})", option.description)
                    }
                }
                _ => option.description.to_string(),
            };
            Item { term: option.flags.to_string(), description }
        })
        .collect();
    items.push(Item { term: "-h, --help".to_string(), description: HELP_DESCRIPTION.to_string() });
    items
}

/// `Help.subcommandTerm`.
fn subcommand_term(command: &CommandSpec) -> String {
    let mut term = command.name.to_string();
    if !command.options.is_empty() {
        term.push_str(" [options]");
    }
    let arguments: Vec<String> = command.arguments.iter().map(human_readable_argument).collect();
    if !arguments.is_empty() {
        term.push(' ');
        term.push_str(&arguments.join(" "));
    }
    term
}

/// Subcommands grouped by heading, in order of first appearance, with the
/// implicit `help [command]` last under `Commands:`.
fn command_sections(command: &CommandSpec) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut add = |heading: &'static str, item: Option<Item>| {
        let index = match sections.iter().position(|section| section.heading == heading) {
            Some(index) => index,
            None => {
                sections.push(Section { heading, items: Vec::new() });
                sections.len() - 1
            }
        };
        if let Some(item) = item {
            sections[index].items.push(item);
        }
    };
    for sub in command.subcommands {
        add(sub.group.unwrap_or("Commands:"), None);
    }
    for sub in command.subcommands {
        add(
            sub.group.unwrap_or("Commands:"),
            Some(Item { term: subcommand_term(sub), description: sub.description.to_string() }),
        );
    }
    if command.has_help_command() {
        add("Commands:", Some(Item { term: "help [command]".to_string(), description: HELP_DESCRIPTION.to_string() }));
    }
    sections
}

/// `Help.preformatted`: a line break followed by a space or tab means the
/// author wrapped the text by hand.
fn preformatted(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    chars.windows(2).any(|pair| pair[0] == '\n' && is_js_whitespace(pair[1]) && pair[1] != '\r' && pair[1] != '\n')
}

/// `Help.formatItem`: pad the term and wrap the description beside it.
fn format_item(term: &str, term_width: usize, description: &str, help_width: usize) -> String {
    let indent = " ".repeat(ITEM_INDENT);
    if description.is_empty() {
        return format!("{indent}{term}");
    }
    let padded_term = pad_end(term, term_width);
    let remaining_width = help_width as isize - term_width as isize - SPACER_WIDTH as isize - ITEM_INDENT as isize;
    let formatted = if remaining_width < MIN_WIDTH_TO_WRAP as isize || preformatted(description) {
        description.to_string()
    } else {
        box_wrap(description, remaining_width as usize)
            .replace('\n', &format!("\n{}", " ".repeat(term_width + SPACER_WIDTH)))
    };
    format!("{indent}{padded_term}{}{}", " ".repeat(SPACER_WIDTH), formatted.replace('\n', &format!("\n{indent}")))
}

/// `Help.boxWrap`: wrap at whitespace, keeping existing line breaks. Text is
/// left alone when the width is below Commander's minimum.
pub fn box_wrap(text: &str, width: usize) -> String {
    if width < MIN_WIDTH_TO_WRAP {
        return text.to_string();
    }
    let mut wrapped = Vec::new();
    for line in text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)) {
        let chunks = chunks(line);
        if chunks.is_empty() {
            wrapped.push(String::new());
            continue;
        }
        let mut current = chunks[0].to_string();
        let mut current_width = js_length(chunks[0]);
        for chunk in &chunks[1..] {
            let chunk_width = js_length(chunk);
            if current_width + chunk_width <= width {
                current.push_str(chunk);
                current_width += chunk_width;
                continue;
            }
            wrapped.push(current);
            let next = chunk.trim_start_matches(is_js_whitespace);
            current = next.to_string();
            current_width = js_length(next);
        }
        wrapped.push(current);
    }
    wrapped.join("\n")
}

/// The matches of `/[\s]*[^\s]+/g`: each word with the whitespace before it.
/// Whitespace at the end of a line belongs to no chunk.
fn chunks(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_word = false;
    for (index, c) in line.char_indices() {
        let space = is_js_whitespace(c);
        if in_word && space {
            out.push(&line[start..index]);
            start = index;
            in_word = false;
        } else if !space {
            in_word = true;
        }
    }
    if in_word {
        out.push(&line[start..]);
    }
    out
}

/// What bare `ath` prints (section 2): the first commands to run. It names
/// the first `*.ath.sqlite` file in the folder when there is one.
pub fn bare_guide(cwd: &Path) -> String {
    let mut files: Vec<String> = std::fs::read_dir(cwd)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(super::runtime::FILE_SUFFIX))
                .collect()
        })
        .unwrap_or_default();
    // Node's readdir returns names in byte order on every supported system.
    files.sort();
    let lines: Vec<String> = match files.first() {
        None => [
            "ath — Athletic Standard, an open file format for your training and recovery data.",
            "",
            "There is no .ath.sqlite file in this folder, so start with one:",
            "",
            "  ath init                       make the file, here",
            "  ath import <export>            load an Apple Health, WHOOP or Oura export into it",
            "",
            "Then you can log what you did, read it back, and predict against it:",
            "",
            "  ath log slept badly, about 5 hours",
            "  ath stats",
            "  ath predict fran --json",
            "",
            "Every command explains itself: `ath <command> --help`, with examples.",
            "The whole list: `ath --help`.",
        ]
        .iter()
        .map(ToString::to_string)
        .collect(),
        Some(name) => vec![
            format!("ath — Athletic Standard. Working on {name} in this folder."),
            String::new(),
            "  ath stats                      what is in the file, and where it came from".to_string(),
            "  ath records list               stable IDs for the records in the database".to_string(),
            "  ath log HRV 61 this morning    write something down".to_string(),
            "  ath import <export>            load more device data".to_string(),
            "  ath predict fran               predict Fran — needs a model in a bare terminal".to_string(),
            "  ath check                      make sure the file is still sound".to_string(),
            String::new(),
            "Every command explains itself: `ath <command> --help`, with examples.".to_string(),
            "The whole list: `ath --help`.".to_string(),
        ],
    };
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_wrap_breaks_at_whitespace() {
        let text = "one two three four five six seven eight nine ten eleven twelve";
        assert_eq!(box_wrap(text, 40), "one two three four five six seven eight\nnine ten eleven twelve");
        assert_eq!(box_wrap(text, 39), text);
        assert_eq!(box_wrap("a\n\nb  ", 40), "a\n\nb");
    }

    #[test]
    fn narrow_columns_are_not_wrapped() {
        let long = "x ".repeat(60);
        let item = format_item("term", 40, long.trim_end(), 80);
        assert!(!item.contains('\n'));
    }
}
