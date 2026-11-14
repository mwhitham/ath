//! Command-line parsing.
//!
//! Clap parses the words into values. It is built from the command table in
//! `spec.rs`. Clap's error precedence differs from the 2.1.0 program, which
//! used Commander: Commander shows help when `--help` follows an unknown
//! option, reports a missing required option before a missing argument,
//! names the whole unknown word (`-bad`, not `-b`), runs `--revision` and
//! `--limit` checks in the order the words appear, and prints its own error
//! texts. So before Clap runs, [`commander_outcome`] walks the same table
//! with Commander's rules and decides whether the words print the version,
//! print help, fail with a Commander error, or run a command. Clap then reads
//! the values only for words that run a command.

use std::collections::HashSet;

use clap::{Arg, ArgAction, ArgMatches, Command};

use super::spec::{self, CommandSpec, DefaultValue, OptionKind, OptionSpec, ValueParser, ROOT};
use crate::json::Value;

/// A whole number of at least 1, held as the JavaScript number the 2.1.0
/// program read (S6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositiveInteger(f64);

impl PositiveInteger {
    /// The number as an unsigned integer. Values beyond `u64` saturate; no
    /// stored revision or limit can reach them.
    pub fn get(self) -> u64 {
        self.0 as u64
    }

    /// The number as JSON, printed as JavaScript prints it.
    pub fn to_json(self) -> Value {
        Value::from(self.0)
    }
}

impl std::fmt::Display for PositiveInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::json::format_number(self.0))
    }
}

/// `parsePositiveInteger`: `Number(value)` must be an integer of at least 1.
pub fn parse_positive_integer(text: &str) -> Result<PositiveInteger, String> {
    let parsed = crate::text::js_number(text);
    if !parsed.is_finite() || parsed.trunc() != parsed || parsed < 1.0 {
        return Err(format!("expected a positive whole number, not '{text}'"));
    }
    Ok(PositiveInteger(parsed))
}

/// What the words on the command line ask for.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Print the version on stdout and exit 0.
    Version,
    /// Print a command's help. With `error`, on stderr with exit status 1;
    /// otherwise on stdout with exit status 0.
    Help { path: Vec<&'static str>, error: bool },
    /// A Commander parse error, printed as is on stderr with exit status 1.
    /// The text starts with `error: `.
    CommanderError(String),
    /// An option value was refused by its parser. Printed as `ath: <text>`
    /// with exit status 1 (S6).
    ValueRefused(String),
    /// Run the command at this path.
    Run(Vec<&'static str>),
}

/// The parsed words of a command that runs.
#[derive(Debug)]
pub struct Args {
    /// The command path below the root, such as `["records", "show"]`.
    pub path: Vec<&'static str>,
    /// One level per command from the root to the command that runs.
    levels: Vec<(&'static CommandSpec, ArgMatches)>,
}

/// The result of parsing.
#[derive(Debug)]
pub enum Parsed {
    /// The words end before any command runs (version, help, or an error).
    Done(Outcome),
    Run(Args),
}

/// Parse the words after the program name.
pub fn parse(argv: &[String]) -> Parsed {
    match commander_outcome(argv) {
        Outcome::Run(path) => match clap_command().try_get_matches_from(argv) {
            Ok(matches) => match Args::from_matches(path.clone(), matches) {
                Some(args) => Parsed::Run(args),
                None => Parsed::Done(Outcome::CommanderError(format!(
                    "error: internal parser disagreement: Clap did not select `{}`",
                    path.join(" ")
                ))),
            },
            Err(error) => {
                Parsed::Done(Outcome::CommanderError(format!("error: internal parser disagreement: {}", error.kind())))
            }
        },
        outcome => Parsed::Done(outcome),
    }
}

// ---------------------------------------------------------------------------
// Clap

/// The Clap command tree for `ath`, built from the command table.
pub fn clap_command() -> Command {
    build(&ROOT, true)
}

/// The Clap id for an option: its flag text, which is unique and cannot be
/// mistaken for an argument name.
fn option_id(option: &OptionSpec) -> &'static str {
    option.flags
}

const HELP_ID: &str = "-h, --help";

fn build(command: &'static CommandSpec, root: bool) -> Command {
    let mut clap = Command::new(command.name)
        .about(command.description)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .disable_colored_help(true)
        .color(clap::ColorChoice::Never)
        .args_override_self(true);
    if root {
        clap = clap.no_binary_name(true);
    }
    for option in command.options {
        clap = clap.arg(clap_option(option));
    }
    clap = clap.arg(Arg::new(HELP_ID).short('h').long("help").action(ArgAction::SetTrue));
    for (index, argument) in command.arguments.iter().enumerate() {
        // Commands with arguments have no subcommands. Like Commander, they
        // read `-5` as an argument.
        let mut arg = Arg::new(argument.name)
            .index(index + 1)
            .required(argument.required)
            .value_name(argument.name)
            .allow_negative_numbers(true);
        if argument.variadic {
            arg = arg.num_args(1..).action(ArgAction::Append);
        }
        clap = clap.arg(arg);
    }
    for sub in command.subcommands {
        clap = clap.subcommand(build(sub, false));
    }
    clap
}

fn clap_option(option: &'static OptionSpec) -> Arg {
    let long = option.long().trim_start_matches("--");
    let mut arg = Arg::new(option_id(option)).long(long).required(option.mandatory);
    if let Some(short) = option.short().and_then(|short| short.chars().nth(1)) {
        arg = arg.short(short);
    }
    if let Some(value_name) = option.value_name() {
        // Commander takes the next word as the value even when it starts with
        // a hyphen: `--data --json` sets the data to `--json`.
        arg = arg.value_name(value_name).num_args(1).allow_hyphen_values(true);
        arg = arg.action(if option.repeatable { ArgAction::Append } else { ArgAction::Set });
        if option.parser == ValueParser::PositiveInteger {
            arg = arg.value_parser(parse_positive_integer);
        }
        if let DefaultValue::Text(text) = option.default {
            arg = arg.default_value(text);
        }
    } else if option.negated() {
        arg = arg.action(ArgAction::SetFalse);
    } else {
        arg = arg.action(ArgAction::SetTrue);
    }
    arg
}

impl Args {
    fn from_matches(path: Vec<&'static str>, matches: ArgMatches) -> Option<Args> {
        let mut levels = Vec::new();
        let mut command: &'static CommandSpec = &ROOT;
        let mut current = matches;
        for name in &path {
            let (sub_name, sub_matches) = current.remove_subcommand()?;
            if sub_name != *name {
                return None;
            }
            levels.push((command, current));
            command = command.subcommand(name)?;
            current = sub_matches;
        }
        if current.subcommand().is_some() {
            return None;
        }
        levels.push((command, current));
        Some(Args { path, levels })
    }

    /// The command that runs.
    pub fn command(&self) -> &'static CommandSpec {
        self.levels.last().expect("there is always a command").0
    }

    fn leaf(&self) -> &ArgMatches {
        &self.levels.last().expect("there is always a command").1
    }

    fn option(&self, id: &str) -> &'static OptionSpec {
        self.command().option(id).unwrap_or_else(|| panic!("`ath {}` has no option `{id}`", self.path.join(" ")))
    }

    /// A flag of the command that runs. A `--no-*` flag is true unless given.
    pub fn flag(&self, id: &str) -> bool {
        let option = self.option(id);
        assert!(!option.takes_value(), "option `{id}` takes a value");
        self.leaf().get_flag(option_id(option))
    }

    /// True when this flag is set on the command that runs or on a command
    /// above it, as `--json` on `skill` or `skill install`.
    pub fn flag_on_path(&self, id: &str) -> bool {
        self.levels.iter().any(|(command, matches)| {
            command.option(id).is_some_and(|option| !option.takes_value() && matches.get_flag(option_id(option)))
        })
    }

    /// The value of a text option: the last one given, else its default.
    pub fn value(&self, id: &str) -> Option<&str> {
        let option = self.option(id);
        assert!(option.parser == ValueParser::Text && !option.repeatable, "option `{id}` is not a single text value");
        self.leaf().get_one::<String>(option_id(option)).map(String::as_str)
    }

    /// Every value of a repeatable option, in order (S7).
    pub fn values(&self, id: &str) -> Vec<String> {
        let option = self.option(id);
        assert!(option.repeatable, "option `{id}` is not repeatable");
        self.leaf().get_many::<String>(option_id(option)).map(|values| values.cloned().collect()).unwrap_or_default()
    }

    /// The value of a positive-integer option, else its default.
    pub fn positive_integer(&self, id: &str) -> Option<PositiveInteger> {
        let option = self.option(id);
        assert!(option.parser == ValueParser::PositiveInteger, "option `{id}` is not a positive integer");
        match self.leaf().get_one::<PositiveInteger>(option_id(option)) {
            Some(value) => Some(*value),
            None => match option.default {
                DefaultValue::Integer(number) => Some(PositiveInteger(number as f64)),
                _ => None,
            },
        }
    }

    /// A single positional argument.
    pub fn argument(&self, name: &str) -> Option<&str> {
        self.assert_argument(name, false);
        self.leaf().get_one::<String>(name).map(String::as_str)
    }

    /// Every word of a variadic positional argument.
    pub fn arguments(&self, name: &str) -> Vec<String> {
        self.assert_argument(name, true);
        self.leaf().get_many::<String>(name).map(|values| values.cloned().collect()).unwrap_or_default()
    }

    fn assert_argument(&self, name: &str, variadic: bool) {
        let found = self.command().arguments.iter().find(|argument| argument.name == name);
        assert!(
            found.is_some_and(|argument| argument.variadic == variadic),
            "`ath {}` has no {}argument `{name}`",
            self.path.join(" "),
            if variadic { "variadic " } else { "single " }
        );
    }
}

// ---------------------------------------------------------------------------
// Commander's rules

/// Decide what the words ask for, following Commander 15 as configured by
/// the 2.1.0 program: positional options on every command, no unknown
/// options, no excess arguments, and suggestions after an unknown option or
/// command.
pub fn commander_outcome(argv: &[String]) -> Outcome {
    parse_command(&ROOT, Vec::new(), Vec::new(), argv.to_vec())
}

struct Level {
    operands: Vec<String>,
    unknown: Vec<String>,
    /// Ids of options given on the command line.
    given: HashSet<&'static str>,
}

/// `Command._parseCommand`.
fn parse_command(
    command: &'static CommandSpec,
    path: Vec<&'static str>,
    operands: Vec<String>,
    unknown: Vec<String>,
) -> Outcome {
    let parsed = match parse_options(command, &unknown) {
        Ok(parsed) => parsed,
        Err(outcome) => return outcome,
    };
    let mut all_operands = operands;
    all_operands.extend(parsed.operands);
    let operands = all_operands;
    let unknown = parsed.unknown;
    let arg_count = operands.len() + unknown.len();

    if let Some(sub) = operands.first().and_then(|name| command.subcommand(name)) {
        let mut sub_path = path;
        sub_path.push(sub.name);
        return parse_command(sub, sub_path, operands[1..].to_vec(), unknown);
    }
    if command.has_help_command() && operands.first().map(String::as_str) == Some("help") {
        return dispatch_help_command(command, path, operands.get(1).map(String::as_str));
    }
    if !command.subcommands.is_empty() && arg_count == 0 && !command.action {
        return Outcome::Help { path, error: true };
    }
    if unknown.iter().any(|word| word == "-h" || word == "--help") {
        return Outcome::Help { path, error: false };
    }
    if let Some(option) = command.options.iter().find(|option| option.mandatory && !parsed.given.contains(option.id)) {
        return Outcome::CommanderError(format!("error: required option '{}' not specified", option.flags));
    }
    if command.action {
        if let Some(word) = unknown.first() {
            return unknown_option(command, word);
        }
        for (index, argument) in command.arguments.iter().enumerate() {
            if argument.required && operands.get(index).is_none() {
                return Outcome::CommanderError(format!("error: missing required argument '{}'", argument.name));
            }
        }
        let variadic = command.arguments.last().is_some_and(|argument| argument.variadic);
        if !variadic && operands.len() > command.arguments.len() {
            return excess_arguments(command, &path, &operands);
        }
        return Outcome::Run(path);
    }
    if let Some(word) = operands.first() {
        // Only group commands have no action, so this is an unknown command.
        return unknown_command(command, word);
    }
    if let Some(word) = unknown.first() {
        return unknown_option(command, word);
    }
    Outcome::Help { path, error: true }
}

/// `Command._dispatchHelpCommand` for `help [command]` under a group.
fn dispatch_help_command(command: &'static CommandSpec, path: Vec<&'static str>, name: Option<&str>) -> Outcome {
    match name {
        None => Outcome::Help { path, error: false },
        Some(name) => match command.subcommand(name) {
            Some(sub) => {
                let mut sub_path = path;
                sub_path.push(sub.name);
                Outcome::Help { path: sub_path, error: false }
            }
            None => Outcome::Help { path, error: true },
        },
    }
}

/// `maybeOption`: a word of two or more characters that starts with `-`.
fn maybe_option(word: &str) -> bool {
    word.starts_with('-') && word.chars().count() > 1
}

/// `negativeNumberArg`: `/^-(\d+|\d*\.\d+)(e[+-]?\d+)?$/`. No option in the
/// table uses a digit as its short form, so the regular expression decides.
fn negative_number(word: &str) -> bool {
    let Some(rest) = word.strip_prefix('-') else { return false };
    let bytes = rest.as_bytes();
    let mut index = 0;
    let count_digits = |index: &mut usize| {
        let start = *index;
        while *index < bytes.len() && bytes[*index].is_ascii_digit() {
            *index += 1;
        }
        *index - start
    };
    let whole = count_digits(&mut index);
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        if count_digits(&mut index) == 0 {
            return false;
        }
    } else if whole == 0 {
        return false;
    }
    if index < bytes.len() && bytes[index] == b'e' {
        index += 1;
        if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
            index += 1;
        }
        if count_digits(&mut index) == 0 {
            return false;
        }
    }
    index == bytes.len()
}

/// `Command.parseOptions` for one command. Known options are read wherever
/// they appear. The first unknown option moves it and every later word that
/// is not an option into `unknown`, for a subcommand to read. A subcommand
/// name or `help` seen before any operand ends this command's options.
fn parse_options(command: &'static CommandSpec, args: &[String]) -> Result<Level, Outcome> {
    let mut level = Level { operands: Vec::new(), unknown: Vec::new(), given: HashSet::new() };
    let mut to_unknown = false;
    let mut index = 0;
    let mut active_group: Option<String> = None;
    loop {
        let arg = match active_group.take() {
            Some(group) => group,
            None if index < args.len() => {
                index += 1;
                args[index - 1].clone()
            }
            None => break,
        };

        if arg == "--" {
            let rest = args[index..].iter().cloned();
            if to_unknown {
                level.unknown.push(arg);
                level.unknown.extend(rest);
            } else {
                level.operands.extend(rest);
            }
            break;
        }

        if maybe_option(&arg) {
            if let Some(option) = command.option_named(&arg) {
                if option.takes_value() {
                    let Some(value) = args.get(index) else {
                        return Err(Outcome::CommanderError(format!(
                            "error: option '{}' argument missing",
                            option.flags
                        )));
                    };
                    index += 1;
                    receive(&mut level, option, Some(value))?;
                } else {
                    receive(&mut level, option, None)?;
                }
                continue;
            }
        }

        // A group of short options, such as `-yh`, or `-mvalue`.
        let mut chars = arg.chars();
        if arg.chars().count() > 2 && chars.next() == Some('-') {
            let second = chars.next().expect("the word has more than two characters");
            if second != '-' {
                if let Some(option) = command.option_named(&format!("-{second}")) {
                    let rest: String = chars.collect();
                    if option.takes_value() {
                        receive(&mut level, option, Some(&rest))?;
                    } else {
                        receive(&mut level, option, None)?;
                        active_group = Some(format!("-{rest}"));
                    }
                    continue;
                }
            }
        }

        // `--name=value` for an option that takes a value.
        if let Some(equals) = arg.strip_prefix("--").and_then(|rest| rest.find('=')).filter(|position| *position > 0) {
            let split = equals + 2;
            if let Some(option) = command.option_named(&arg[..split]).filter(|option| option.takes_value()) {
                receive(&mut level, option, Some(&arg[split + 1..]))?;
                continue;
            }
        }

        if !to_unknown && maybe_option(&arg) && !(command.subcommands.is_empty() && negative_number(&arg)) {
            to_unknown = true;
        }

        // Positional options: options after a subcommand name belong to it.
        if level.operands.is_empty() && level.unknown.is_empty() {
            if command.subcommand(&arg).is_some() {
                level.operands.push(arg);
                level.unknown.extend(args[index..].iter().cloned());
                break;
            }
            if command.has_help_command() && arg == "help" {
                level.operands.push(arg);
                level.operands.extend(args[index..].iter().cloned());
                break;
            }
        }

        if to_unknown {
            level.unknown.push(arg);
        } else {
            level.operands.push(arg);
        }
    }
    Ok(level)
}

/// Commander's option listener: the version option prints and exits; a
/// custom parser runs as soon as its value is read.
fn receive(level: &mut Level, option: &'static OptionSpec, value: Option<&str>) -> Result<(), Outcome> {
    if option.kind == OptionKind::Version {
        return Err(Outcome::Version);
    }
    if let (Some(value), ValueParser::PositiveInteger) = (value, option.parser) {
        parse_positive_integer(value).map_err(Outcome::ValueRefused)?;
    }
    level.given.insert(option.id);
    Ok(())
}

/// `Command.unknownOption`, with Commander's suggestion for a long option.
fn unknown_option(command: &CommandSpec, word: &str) -> Outcome {
    let mut suggestion = String::new();
    if word.starts_with("--") {
        let mut candidates: Vec<&str> = command.options.iter().map(|option| option.long()).collect();
        candidates.push("--help");
        suggestion = suggest_similar(word, &candidates);
    }
    Outcome::CommanderError(format!("error: unknown option '{word}'{suggestion}"))
}

/// `Command.unknownCommand` under a group command.
fn unknown_command(command: &CommandSpec, word: &str) -> Outcome {
    let mut candidates: Vec<&str> = command.subcommands.iter().map(|sub| sub.name).collect();
    if command.has_help_command() {
        candidates.push("help");
    }
    let suggestion = suggest_similar(word, &candidates);
    Outcome::CommanderError(format!("error: unknown command '{word}'{suggestion}"))
}

/// `Command._excessArguments`.
fn excess_arguments(command: &CommandSpec, path: &[&str], operands: &[String]) -> Outcome {
    let expected = command.arguments.len();
    let plural = if expected == 1 { "" } else { "s" };
    let for_subcommand = if path.is_empty() { String::new() } else { format!(" for '{}'", command.name) };
    Outcome::CommanderError(format!(
        "error: too many arguments{for_subcommand}. Expected {expected} argument{plural} but got {}: {}.",
        operands.len(),
        operands.join(", ")
    ))
}

/// Commander's `suggestSimilar`: the closest candidates by optimal string
/// alignment distance, when they are similar enough.
fn suggest_similar(word: &str, candidates: &[&str]) -> String {
    const MAX_DISTANCE: usize = 3;
    const MIN_SIMILARITY: f64 = 0.4;
    let mut unique: Vec<&str> = Vec::new();
    for candidate in candidates {
        if !unique.contains(candidate) {
            unique.push(candidate);
        }
    }
    let searching_options = word.starts_with("--");
    let word = if searching_options { &word[2..] } else { word };
    let word: Vec<u16> = word.encode_utf16().collect();
    let mut similar: Vec<String> = Vec::new();
    let mut best = MAX_DISTANCE;
    for candidate in unique {
        let candidate = if searching_options { candidate.get(2..).unwrap_or("") } else { candidate };
        let units: Vec<u16> = candidate.encode_utf16().collect();
        if units.len() <= 1 {
            continue;
        }
        let distance = edit_distance(&word, &units);
        let length = word.len().max(units.len());
        let similarity = (length as f64 - distance as f64) / length as f64;
        if similarity > MIN_SIMILARITY {
            if distance < best {
                best = distance;
                similar = vec![candidate.to_string()];
            } else if distance == best {
                similar.push(candidate.to_string());
            }
        }
    }
    // `localeCompare` orders these lowercase names as their bytes do.
    similar.sort_by_key(|name| name.to_lowercase());
    if searching_options {
        similar = similar.into_iter().map(|name| format!("--{name}")).collect();
    }
    match similar.len() {
        0 => String::new(),
        1 => format!("\n(Did you mean {}?)", similar[0]),
        _ => format!("\n(Did you mean one of {}?)", similar.join(", ")),
    }
}

/// Optimal string alignment distance, with Commander's early exit.
fn edit_distance(a: &[u16], b: &[u16]) -> usize {
    const MAX_DISTANCE: usize = 3;
    if a.len().abs_diff(b.len()) > MAX_DISTANCE {
        return a.len().max(b.len());
    }
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for j in 1..=b.len() {
        for i in 1..=a.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

/// Every command path in the table, for tests and generated references.
pub fn command_paths() -> Vec<Vec<&'static str>> {
    spec::all_commands().into_iter().map(|(path, _)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(ToString::to_string).collect()
    }

    fn outcome(text: &str) -> Outcome {
        commander_outcome(&words(text))
    }

    fn error(text: &str) -> String {
        match outcome(text) {
            Outcome::CommanderError(message) => message,
            other => panic!("{text}: expected an error, got {other:?}"),
        }
    }

    #[test]
    fn positive_integers_follow_number() {
        assert_eq!(parse_positive_integer("3").unwrap().get(), 3);
        assert_eq!(parse_positive_integer(" 2 ").unwrap().get(), 2);
        assert_eq!(parse_positive_integer("1e2").unwrap().get(), 100);
        assert_eq!(parse_positive_integer("0x10").unwrap().get(), 16);
        assert_eq!(parse_positive_integer("4.0").unwrap().get(), 4);
        for text in ["0", "-1", "1.5", "abc", "", "Infinity"] {
            assert_eq!(
                parse_positive_integer(text).unwrap_err(),
                format!("expected a positive whole number, not '{text}'")
            );
        }
    }

    #[test]
    fn help_wins_over_a_later_unknown_option() {
        assert_eq!(outcome("log --nosuch --help"), Outcome::Help { path: vec!["log"], error: false });
        assert_eq!(outcome("--help log"), Outcome::Help { path: vec![], error: false });
        assert_eq!(outcome("records list -h"), Outcome::Help { path: vec!["records", "list"], error: false });
        assert_eq!(outcome("log -yh"), Outcome::Help { path: vec!["log"], error: false });
    }

    #[test]
    fn a_value_option_takes_the_next_word() {
        assert_eq!(outcome("correct 1 --revision 1 --data --help"), Outcome::Run(vec!["correct"]));
        assert_eq!(error("init --units"), "error: option '--units <units>' argument missing");
    }

    #[test]
    fn value_checks_run_in_word_order() {
        assert_eq!(
            outcome("records list --nosuch --limit 0"),
            Outcome::ValueRefused("expected a positive whole number, not '0'".to_string())
        );
        assert_eq!(
            outcome("records list --limit=0 --help"),
            Outcome::ValueRefused("expected a positive whole number, not '0'".to_string())
        );
    }

    #[test]
    fn unknown_words_are_named_whole() {
        assert_eq!(error("log felt -bad"), "error: unknown option '-bad'");
        assert_eq!(error("log --jsn"), "error: unknown option '--jsn'\n(Did you mean --json?)");
        assert_eq!(error("records lst"), "error: unknown command 'lst'\n(Did you mean list?)");
        assert_eq!(error("nosuch --x"), "error: unknown option '--x'");
        assert_eq!(outcome("log ran -5 km"), Outcome::Run(vec!["log"]));
        assert_eq!(outcome("log -- --json"), Outcome::Run(vec!["log"]));
    }

    #[test]
    fn root_options_only_before_a_subcommand() {
        assert_eq!(error("log --update"), "error: unknown option '--update'");
        assert_eq!(outcome("--update log"), Outcome::Run(vec!["log"]));
        assert_eq!(outcome("nosuch --version"), Outcome::Version);
    }

    #[test]
    fn help_command_only_under_groups() {
        assert_eq!(outcome("records help"), Outcome::Help { path: vec!["records"], error: false });
        assert_eq!(outcome("records help show x"), Outcome::Help { path: vec!["records", "show"], error: false });
        assert_eq!(outcome("records help nosuch"), Outcome::Help { path: vec!["records"], error: true });
        assert_eq!(error("skill help"), "error: too many arguments for 'skill'. Expected 0 arguments but got 1: help.");
    }

    #[test]
    fn clap_agrees_on_commands_that_run() {
        let cases = [
            "",
            "--update",
            "--update log",
            "init -y --name Sam --no-skill --units imperial",
            "skill --json install",
            "skill install --json",
            "log slept badly, about 5 hours",
            "log ran -5 km --json --attachment a.png --attachment b.png",
            "log -- --json -x",
            "correct 0190 --revision=2 --data --json --yes",
            "correct 0190 --revision 2 --data {} --data [] -y",
            "link a b --revision 1e0",
            "backtest -m one -mtwo --model three --all",
            "records list --limit 5 --type prediction --include-deleted",
            "records show x --revision 0x2 --history --links",
            "series heart_rate --limit 7 --raw",
            "check other.ath.sqlite --file x.ath.sqlite --json",
            "key set openrouter",
            "key clear",
            "erase id --revision 1 --reason other --confirm-erasure id --dry-run",
            "export - --exported-at 2026-01-01T00:00:00.000Z",
        ];
        for case in cases {
            let argv = words(case);
            let Outcome::Run(path) = commander_outcome(&argv) else {
                panic!("{case}: expected a run");
            };
            match parse(&argv) {
                Parsed::Run(args) => assert_eq!(args.path, path, "{case}"),
                Parsed::Done(outcome) => panic!("{case}: {outcome:?}"),
            }
        }
    }

    #[test]
    fn values_are_read_as_commander_reads_them() {
        let Parsed::Run(args) = parse(&words("backtest -m one -mtwo --model three")) else { panic!() };
        assert_eq!(args.values("model"), ["one", "two", "three"]);
        let Parsed::Run(args) = parse(&words("correct id --revision 2 --data --json --revision 3")) else { panic!() };
        assert_eq!(args.value("data"), Some("--json"));
        assert_eq!(args.positive_integer("revision").map(PositiveInteger::get), Some(3));
        assert!(!args.flag("json"));
        assert_eq!(args.argument("record-id"), Some("id"));
        let Parsed::Run(args) = parse(&words("init")) else { panic!() };
        assert!(args.flag("skill"));
        assert_eq!(args.value("units"), Some("metric"));
        assert_eq!(args.value("file"), Some("athlete.ath.sqlite"));
        let Parsed::Run(args) = parse(&words("init --no-skill")) else { panic!() };
        assert!(!args.flag("skill"));
        let Parsed::Run(args) = parse(&words("series heart_rate")) else { panic!() };
        assert_eq!(args.positive_integer("limit").map(PositiveInteger::get), Some(10_000));
        let Parsed::Run(args) = parse(&words("log ran -5 km")) else { panic!() };
        assert_eq!(args.arguments("entry"), ["ran", "-5", "km"]);
        let Parsed::Run(args) = parse(&words("skill --json install")) else { panic!() };
        assert!(args.flag_on_path("json"));
        assert!(!args.flag("json"));
    }
}
