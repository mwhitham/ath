//! The command definition table.
//!
//! Every command, argument, and option of `ath` is defined once here. The Clap
//! parser and the Commander-style help text are both built from this table, so
//! a flag cannot be described one way in help and parsed another way.
//!
//! The words come from the 2.1.0 program (`native/reference/command-inventory.json`).
//! Option `flags` use Commander's notation: `-y, --yes` is a flag with a short
//! form, `--revision <number>` takes a required value, and `--no-skill` is a
//! negated flag whose value is true unless the flag is given.

/// The help group headings on `ath --help`.
pub const GROUP_SET_UP: &str = "Set up:";
pub const GROUP_GET_IN: &str = "Get your data in:";
pub const GROUP_READ: &str = "Read your data:";
pub const GROUP_PREDICT: &str = "Predict and grade:";

/// The description of the built-in `-h, --help` option and `help [command]`.
pub const HELP_DESCRIPTION: &str = "display help for command";

/// One command or subcommand.
#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// The heading this command is listed under in its parent's help.
    /// `None` lists it under `Commands:`.
    pub group: Option<&'static str>,
    pub arguments: &'static [ArgumentSpec],
    pub options: &'static [OptionSpec],
    pub subcommands: &'static [CommandSpec],
    /// True when the command runs on its own. A command with subcommands and
    /// no action is a group: it prints its help when no subcommand is named,
    /// and it has the implicit `help [command]` subcommand.
    pub action: bool,
    /// Text printed after the generated help, starting with its heading line.
    pub examples: Option<&'static str>,
}

impl CommandSpec {
    /// True for a group command, which has the implicit `help [command]`.
    pub fn has_help_command(&self) -> bool {
        !self.subcommands.is_empty() && !self.action
    }

    pub fn subcommand(&self, name: &str) -> Option<&'static CommandSpec> {
        self.subcommands.iter().find(|command| command.name == name)
    }

    pub fn option(&self, id: &str) -> Option<&'static OptionSpec> {
        self.options.iter().find(|option| option.id == id)
    }

    /// The option a command-line word names exactly, such as `-y` or `--yes`.
    pub fn option_named(&self, word: &str) -> Option<&'static OptionSpec> {
        self.options.iter().find(|option| option.is(word))
    }
}

/// A positional argument.
#[derive(Debug, Clone, Copy)]
pub struct ArgumentSpec {
    pub name: &'static str,
    pub required: bool,
    /// Collects every remaining word. Only the last argument can be variadic.
    pub variadic: bool,
    pub description: &'static str,
}

/// How an option's value is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueParser {
    /// The text as given.
    Text,
    /// A whole number of at least 1, read as JavaScript's `Number()` reads
    /// text. Anything else is refused (S6).
    PositiveInteger,
}

/// An option's default when it is not given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultValue {
    None,
    Text(&'static str),
    Integer(u64),
}

/// The special meaning of an option, beyond carrying a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionKind {
    /// An ordinary option.
    Ordinary,
    /// `-V, --version`: prints the version and exits as soon as it is read.
    Version,
}

/// One option.
#[derive(Debug, Clone, Copy)]
pub struct OptionSpec {
    /// The parsed value's name for handlers: Commander's attribute name in
    /// snake case. `--dry-run` is `dry_run`; `--no-skill` is `skill`.
    pub id: &'static str,
    /// Commander's flag text, shown in help and in error messages.
    pub flags: &'static str,
    pub description: &'static str,
    /// Required on the command line (Commander's `requiredOption`).
    pub mandatory: bool,
    /// Every occurrence is kept in order, starting from an empty list (S7).
    /// Help shows `(default: [])`.
    pub repeatable: bool,
    pub parser: ValueParser,
    pub default: DefaultValue,
    pub kind: OptionKind,
}

const fn option(id: &'static str, flags: &'static str, description: &'static str) -> OptionSpec {
    OptionSpec {
        id,
        flags,
        description,
        mandatory: false,
        repeatable: false,
        parser: ValueParser::Text,
        default: DefaultValue::None,
        kind: OptionKind::Ordinary,
    }
}

/// An option without a value, such as `--json` or `--no-skill`.
const fn flag(id: &'static str, flags: &'static str, description: &'static str) -> OptionSpec {
    option(id, flags, description)
}

/// An option with a required value, such as `--file <path>`.
const fn value(id: &'static str, flags: &'static str, description: &'static str) -> OptionSpec {
    option(id, flags, description)
}

const fn version(id: &'static str, flags: &'static str, description: &'static str) -> OptionSpec {
    OptionSpec { kind: OptionKind::Version, ..option(id, flags, description) }
}

impl OptionSpec {
    const fn mandatory(self) -> Self {
        OptionSpec { mandatory: true, ..self }
    }

    const fn repeatable(self) -> Self {
        OptionSpec { repeatable: true, ..self }
    }

    const fn positive_integer(self) -> Self {
        OptionSpec { parser: ValueParser::PositiveInteger, ..self }
    }

    const fn default_text(self, text: &'static str) -> Self {
        OptionSpec { default: DefaultValue::Text(text), ..self }
    }

    const fn default_integer(self, number: u64) -> Self {
        OptionSpec { default: DefaultValue::Integer(number), ..self }
    }

    /// The flag words, split as Commander splits them: `-y, --yes` gives
    /// `["-y", "--yes"]` and `--revision <number>` gives
    /// `["--revision", "<number>"]`.
    fn words(&self) -> impl Iterator<Item = &'static str> {
        self.flags.split([' ', ',', '|']).filter(|word| !word.is_empty())
    }

    /// The short form, such as `-y`.
    pub fn short(&self) -> Option<&'static str> {
        self.words().find(|word| word.len() == 2 && word.starts_with('-') && !word.starts_with("--"))
    }

    /// The long form, such as `--yes`.
    pub fn long(&self) -> &'static str {
        self.words().find(|word| word.starts_with("--")).expect("every option in the table has a long form")
    }

    /// The value placeholder without brackets, such as `number`, for an
    /// option that takes a value.
    pub fn value_name(&self) -> Option<&'static str> {
        self.words().find(|word| word.starts_with('<')).map(|word| word.trim_start_matches('<').trim_end_matches('>'))
    }

    pub fn takes_value(&self) -> bool {
        self.value_name().is_some()
    }

    /// A `--no-*` flag. Its value is true unless the flag is given.
    pub fn negated(&self) -> bool {
        self.long().starts_with("--no-")
    }

    /// True when `word` is this option's short or long form.
    pub fn is(&self, word: &str) -> bool {
        self.short() == Some(word) || self.long() == word
    }

    /// The default as help prints it: `JSON.stringify` of the default value.
    pub fn default_display(&self) -> Option<String> {
        if self.repeatable {
            return Some("[]".to_string());
        }
        match self.default {
            DefaultValue::None => None,
            DefaultValue::Text(text) => Some(crate::json::to_string(&crate::json::Value::String(text.to_string()))),
            DefaultValue::Integer(number) => Some(number.to_string()),
        }
    }
}

const fn required(name: &'static str, description: &'static str) -> ArgumentSpec {
    ArgumentSpec { name, required: true, variadic: false, description }
}

const fn optional(name: &'static str, description: &'static str) -> ArgumentSpec {
    ArgumentSpec { name, required: false, variadic: false, description }
}

const fn optional_variadic(name: &'static str, description: &'static str) -> ArgumentSpec {
    ArgumentSpec { name, required: false, variadic: true, description }
}

/// The root command, `ath`.
pub static ROOT: CommandSpec = CommandSpec {
    name: "ath",
    description: "Athletic Standard — an open, local-first format for training and recovery data that keeps measured signals apart from self-reported ones.",
    group: None,
    arguments: &[],
    options: &[
        version("version", "-V, --version", "output the version number"),
        flag(
            "update",
            "--update",
            "install the latest ath with the tool that installed it, replace an older tool skill, and add skill files that are not already here",
        ),
    ],
    subcommands: &[
        INIT, SKILL, MIGRATE, REPAIR, IMPORT, CONFLICTS, EXPORT, LOG, LINK, CORRECT, DELETE, ERASE, CHECK, KEY, MODELS,
        PREDICT, BACKTEST, SHARE, GRADE, APPLY, PULL, CALCULATE, SERIES, STATS, RECORDS,
    ],
    action: true,
    examples: Some(include_str!("examples/ath.txt")),
};

/// Find a command by its path below the root, such as `["records", "show"]`.
pub fn find(path: &[&str]) -> Option<&'static CommandSpec> {
    let mut command: &'static CommandSpec = &ROOT;
    for name in path {
        command = command.subcommand(name)?;
    }
    Some(command)
}

/// Every command below the root, depth first, with its path.
pub fn all_commands() -> Vec<(Vec<&'static str>, &'static CommandSpec)> {
    fn walk(
        prefix: &[&'static str],
        command: &'static CommandSpec,
        out: &mut Vec<(Vec<&'static str>, &'static CommandSpec)>,
    ) {
        for sub in command.subcommands {
            let mut path = prefix.to_vec();
            path.push(sub.name);
            out.push((path.clone(), sub));
            walk(&path, sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&[], &ROOT, &mut out);
    out
}

pub const INIT: CommandSpec = CommandSpec {
    name: "init",
    description: "create a new athlete.ath.sqlite here, with the well-known benchmarks defined",
    group: Some(GROUP_SET_UP),
    arguments: &[],
    options: &[
        value("name", "--name <name>", "your name, so the file says whose it is"),
        value("birth_year", "--birth-year <year>", "birth year, which some readings are read against"),
        value("sex", "--sex <sex>", "male | female, for the same reason"),
        value("units", "--units <units>", "metric | imperial — display only; stored values are always metric")
            .default_text("metric"),
        value("timezone", "--timezone <name>", "default IANA timezone for calendar views, such as America/Chicago"),
        value("file", "--file <path>", "write somewhere other than the default name")
            .default_text("athlete.ath.sqlite"),
        flag("yes", "-y, --yes", "skip the questions and use the flags and defaults"),
        flag("empty", "--empty", "create no athlete records, for restoring a v1 package"),
        flag("skill", "--no-skill", "do not install the agent skill, even if an agent folder is here"),
        flag("json", "--json", "structured output, for an agent rather than a person"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/init.txt")),
};

pub const SKILL: CommandSpec = CommandSpec {
    name: "skill",
    description: "where the agent skill is installed, and whether each copy is current",
    group: Some(GROUP_SET_UP),
    arguments: &[],
    options: &[flag("json", "--json", "structured output, for an agent rather than a person")],
    subcommands: &[SKILL_INSTALL],
    action: true,
    examples: Some(include_str!("examples/skill.txt")),
};

pub const SKILL_INSTALL: CommandSpec = CommandSpec {
    name: "install",
    description: "copy the skill into .agents/skills and every harness folder here",
    group: None,
    arguments: &[],
    options: &[flag("json", "--json", "structured output, for an agent rather than a person")],
    subcommands: &[],
    action: true,
    examples: None,
};

pub const MIGRATE: CommandSpec = CommandSpec {
    name: "migrate",
    description: "preview and apply a SQL schema migration after a verified backup",
    group: Some(GROUP_SET_UP),
    arguments: &[optional("file", "the database to migrate (default: the one in this folder)")],
    options: &[
        value("file", "--file <path>", "migrate a database other than the one in this folder"),
        value(
            "confirm_migration",
            "--confirm-migration <database-id>",
            "repeat the database ID exactly for non-interactive use",
        ),
        flag("yes", "-y, --yes", "apply the preview after reviewing it"),
        flag("dry_run", "--dry-run", "show the preview and write nothing"),
        flag("json", "--json", "structured preview; never confirms the migration"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/migrate.txt")),
};

pub const REPAIR: CommandSpec = CommandSpec {
    name: "repair",
    description: "repair typed-table JSON bytes left by npm 1.0.0 provider imports",
    group: Some(GROUP_SET_UP),
    arguments: &[optional("file", "the database to repair (default: the one in this folder)")],
    options: &[
        value("file", "--file <path>", "repair a database other than the one in this folder"),
        flag("yes", "-y, --yes", "apply the repair after reviewing the preview"),
        flag(
            "from_revision",
            "--from-revision",
            "copy typed-table JSON from the current revision when bytes disagree in meaning",
        ),
        value(
            "confirm_revision_align",
            "--confirm-revision-align <database-id>",
            "repeat the database ID exactly when using --from-revision",
        ),
        flag("dry_run", "--dry-run", "show how many records would change and write nothing"),
        flag("json", "--json", "structured preview only"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/repair.txt")),
};

pub const IMPORT: CommandSpec = CommandSpec {
    name: "import",
    description: "preview and import a package or an Apple Health, WHOOP, Oura, Garmin, or Strava export",
    group: Some(GROUP_GET_IN),
    arguments: &[required("path", "v1 package or supported provider export")],
    options: &[
        value(
            "mapping",
            "--mapping <name>",
            "read a JSON file with a mapping file: a name in .agents/mappings or mappings/, or a path",
        ),
        value("file", "--file <path>", "import into a database other than the one in this folder"),
        flag("yes", "-y, --yes", "apply a conflict-free preview"),
        flag("json", "--json", "structured preview; never confirms the write"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/import.txt")),
};

pub const CONFLICTS: CommandSpec = CommandSpec {
    name: "conflicts",
    description: "review and resolve the temporary conflicts from a package import",
    group: Some(GROUP_GET_IN),
    arguments: &[],
    options: &[],
    subcommands: &[CONFLICTS_SHOW, CONFLICTS_RESOLVE],
    action: false,
    examples: Some(include_str!("examples/conflicts.txt")),
};

pub const CONFLICTS_SHOW: CommandSpec = CommandSpec {
    name: "show",
    description: "show complete local and imported values from the current conflict preview",
    group: None,
    arguments: &[optional("record-id", "show one conflicting stable record ID")],
    options: &[
        value("file", "--file <path>", "read conflicts for a database other than the one in this folder"),
        flag("json", "--json", "structured output"),
    ],
    subcommands: &[],
    action: true,
    examples: None,
};

pub const CONFLICTS_RESOLVE: CommandSpec = CommandSpec {
    name: "resolve",
    description: "choose the local, imported, or a corrected value for one conflict",
    group: None,
    arguments: &[required("record-id", "conflicting stable record ID")],
    options: &[
        value("choice", "--choice <choice>", "local, imported, or corrected").mandatory(),
        value("data", "--data <json>", "complete corrected data object; required with corrected"),
        value("file", "--file <path>", "resolve conflicts for a database other than the one in this folder"),
        flag("json", "--json", "structured output"),
    ],
    subcommands: &[],
    action: true,
    examples: None,
};

pub const EXPORT: CommandSpec = CommandSpec {
    name: "export",
    description: "write a complete deterministic Athletic Standard JSON package",
    group: Some(GROUP_GET_IN),
    arguments: &[required("directory", "new directory to create for the package")],
    options: &[
        value("exported_at", "--exported-at <timestamp>", "include this exact UTC export time in the manifest"),
        value("file", "--file <path>", "export a database other than the one in this folder"),
        flag("json", "--json", "structured command result"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/export.txt")),
};

pub const LOG: CommandSpec = CommandSpec {
    name: "log",
    description: "write one plain-text record or one validated structured batch",
    group: Some(GROUP_GET_IN),
    arguments: &[optional_variadic("entry", "the entry, unquoted. Leave it off to paste one, ending with Ctrl-D")],
    options: &[
        value("benchmark", "--benchmark <name>", "name the workout yourself, instead of naming it after the day"),
        value("at", "--at <timestamp>", "exact UTC event time with milliseconds; defaults to now"),
        value("agent", "--agent <name>", "name the agent making a structured write"),
        value(
            "attachment",
            "--attachment <path>",
            "attach a PNG, JPEG, WebP, or PDF; pass more than once for several files",
        )
        .repeatable(),
        value("file", "--file <path>", "write to a file other than the one in this folder"),
        flag("yes", "-y, --yes", "write it without asking — for scripts, and when you are sure"),
        flag("dry_run", "--dry-run", "show what would be written, and write nothing"),
        flag("json", "--json", "structured preview for an agent; never confirms the write"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/log.txt")),
};

pub const LINK: CommandSpec = CommandSpec {
    name: "link",
    description: "attach a benchmark result to the workout session it happened in",
    group: Some(GROUP_GET_IN),
    arguments: &[
        required("result-id", "stable benchmark-result record ID"),
        required("session-id", "stable workout-session record ID"),
    ],
    options: &[
        value("revision", "--revision <number>", "current result revision from the preview")
            .mandatory()
            .positive_integer(),
        value("file", "--file <path>", "change a database other than the one in this folder"),
        flag("yes", "-y, --yes", "write without asking after reviewing the preview"),
        flag("dry_run", "--dry-run", "show what would change and write nothing"),
        flag("json", "--json", "structured preview; never confirms the write"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/link.txt")),
};

pub const CORRECT: CommandSpec = CommandSpec {
    name: "correct",
    description: "replace one record's complete data and retain its prior revision",
    group: Some(GROUP_GET_IN),
    arguments: &[required("record-id", "stable record ID")],
    options: &[
        value("revision", "--revision <number>", "current revision from the preview").mandatory().positive_integer(),
        value("data", "--data <json>", "complete replacement for the record's data object").mandatory(),
        value("agent", "--agent <name>", "name the agent making the correction"),
        value(
            "attachment",
            "--attachment <path>",
            "attach a PNG, JPEG, WebP, or PDF; pass more than once for several files",
        )
        .repeatable(),
        value("file", "--file <path>", "change a database other than the one in this folder"),
        flag("yes", "-y, --yes", "write without asking after reviewing the preview"),
        flag("dry_run", "--dry-run", "show what would change and write nothing"),
        flag("json", "--json", "structured preview; never confirms the write"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/correct.txt")),
};

pub const DELETE: CommandSpec = CommandSpec {
    name: "delete",
    description: "hide one current record while retaining its complete history",
    group: Some(GROUP_GET_IN),
    arguments: &[required("record-id", "stable record ID")],
    options: &[
        value("revision", "--revision <number>", "current revision from the preview").mandatory().positive_integer(),
        value("file", "--file <path>", "change a database other than the one in this folder"),
        flag("yes", "-y, --yes", "write without asking after reviewing the preview"),
        flag("dry_run", "--dry-run", "show what would change and write nothing"),
        flag("json", "--json", "structured preview; never confirms the write"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/delete.txt")),
};

pub const ERASE: CommandSpec = CommandSpec {
    name: "erase",
    description: "permanently remove one record and every record that depends on it",
    group: Some(GROUP_GET_IN),
    arguments: &[required("record-id", "stable record ID to erase")],
    options: &[
        value("revision", "--revision <number>", "current revision from the preview").mandatory().positive_integer(),
        value("reason", "--reason <code>", "one of: athlete_request, legal_requirement, recorded_in_error, other")
            .default_text("athlete_request"),
        value("confirm_erasure", "--confirm-erasure <record-id>", "second confirmation: repeat the record ID exactly"),
        value("file", "--file <path>", "change a database other than the one in this folder"),
        flag("yes", "-y, --yes", "accept the first confirmation after reviewing the preview"),
        flag("dry_run", "--dry-run", "show everything that would be erased and write nothing"),
        flag("json", "--json", "structured preview; never confirms the erasure"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/erase.txt")),
};

pub const CHECK: CommandSpec = CommandSpec {
    name: "check",
    description: "make sure the file still obeys every rule of the format",
    group: Some(GROUP_READ),
    arguments: &[optional("file", "the file to check (default: the one in this folder)")],
    options: &[
        value("file", "--file <path>", "check a database other than the one in this folder"),
        flag("validate_export", "--validate-export", "also build and validate a temporary complete JSON package"),
        flag("json", "--json", "structured output, for an agent rather than a person"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/check.txt")),
};

pub const KEY: CommandSpec = CommandSpec {
    name: "key",
    description: "save or show a gateway key — never stored in the athlete file",
    group: Some(GROUP_SET_UP),
    arguments: &[],
    options: &[flag("json", "--json", "structured output, for an agent rather than a person")],
    subcommands: &[KEY_SET, KEY_CLEAR],
    action: true,
    examples: Some(include_str!("examples/key.txt")),
};

pub const KEY_SET: CommandSpec = CommandSpec {
    name: "set",
    description: "save a Vercel or OpenRouter key in the OS password store",
    group: None,
    arguments: &[required("gateway", "vercel or openrouter")],
    options: &[],
    subcommands: &[],
    action: true,
    examples: None,
};

pub const KEY_CLEAR: CommandSpec = CommandSpec {
    name: "clear",
    description: "delete the saved gateway key from the password store",
    group: None,
    arguments: &[optional("gateway", "vercel or openrouter; leave off to clear both")],
    options: &[],
    subcommands: &[],
    action: true,
    examples: None,
};

pub const MODELS: CommandSpec = CommandSpec {
    name: "models",
    description: "list live text models that can reason, or save your usual one",
    group: Some(GROUP_SET_UP),
    arguments: &[],
    options: &[
        value(
            "default",
            "--default <name>",
            "save this model in athlete.ath.model next to the database, for the next predict",
        ),
        value("gateway", "--gateway <name>", "vercel or openrouter, when both keys are available"),
        value("file", "--file <path>", "the athlete file whose default to save"),
        flag("json", "--json", "structured output, for an agent rather than a person"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/models.txt")),
};

pub const PREDICT: CommandSpec = CommandSpec {
    name: "predict",
    description: "predict a benchmark — needs a model in a bare terminal; --json is evidence",
    group: Some(GROUP_PREDICT),
    arguments: &[required("benchmark", "the benchmark to predict, e.g. `fran`. See them all with `ath stats`")],
    options: &[
        value("model", "--model <name>", "the model for this run. See them with `ath models`"),
        value("gateway", "--gateway <name>", "vercel or openrouter, when both keys are available"),
        value("as_of", "--as-of <time>", "historical day (YYYY-MM-DD) or exact UTC timestamp"),
        value("timezone", "--timezone <name>", "IANA timezone when --as-of is a calendar day"),
        value("file", "--file <path>", "read a file other than the one in this folder"),
        flag("yes", "-y, --yes", "write the prediction without asking"),
        flag("dry_run", "--dry-run", "show the prediction, and write nothing"),
        flag("json", "--json", "evidence for a harness, not a prediction"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/predict.txt")),
};

pub const BACKTEST: CommandSpec = CommandSpec {
    name: "backtest",
    description: "replay history with one or more models — writes a report, not the athlete file",
    group: Some(GROUP_PREDICT),
    arguments: &[],
    options: &[
        value("model", "-m, --model <name>", "a model to run; pass more than once to compare").repeatable(),
        flag("all", "--all", "run every live text model that can reason, after showing the count"),
        value("gateway", "--gateway <name>", "vercel or openrouter, when both keys are available"),
        value("file", "--file <path>", "read a file other than the one in this folder"),
        flag("yes", "-y, --yes", "do not ask before a long --all run"),
        flag("json", "--json", "structured output, for an agent rather than a person"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/backtest.txt")),
};

pub const SHARE: CommandSpec = CommandSpec {
    name: "share",
    description: "not built yet — names the latest backtest report",
    group: Some(GROUP_PREDICT),
    arguments: &[],
    options: &[
        value("file", "--file <path>", "look next to a file other than the one in this folder"),
        flag("json", "--json", "structured output, for an agent rather than a person"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/share.txt")),
};

pub const GRADE: CommandSpec = CommandSpec {
    name: "grade",
    description: "grade an existing prediction against an existing benchmark result",
    group: Some(GROUP_PREDICT),
    arguments: &[required("prediction-id", "stable prediction record ID")],
    options: &[
        value("result", "--result <record-id>", "stable benchmark-result record ID").mandatory(),
        value("calculation_version", "--calculation-version <version>", "grade calculation version")
            .default_text("prediction-grade-v1"),
        value("file", "--file <path>", "change a database other than the one in this folder"),
        flag("yes", "-y, --yes", "write without asking after reviewing the preview"),
        flag("dry_run", "--dry-run", "show what would be written, and write nothing"),
        flag("json", "--json", "structured preview; never confirms the write"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/grade.txt")),
};

pub const APPLY: CommandSpec = CommandSpec {
    name: "apply",
    description: "run one calculation skill file on one record",
    group: Some(GROUP_READ),
    arguments: &[required("calculation", "calculation file name, such as heart-rate-zones")],
    options: &[
        value("record", "--record <id>", "the one record to use").mandatory(),
        value("file", "--file <path>", "read a database other than the one in this folder"),
        flag("json", "--json", "structured result"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/apply.txt")),
};

pub const PULL: CommandSpec = CommandSpec {
    name: "pull",
    description: "read one day from Garmin's personal connection",
    group: Some(GROUP_GET_IN),
    arguments: &[required("source", "garmin")],
    options: &[
        value("date", "--date <day>", "YYYY-MM-DD").mandatory(),
        value("base_url", "--base-url <url>", "personal API root, for a test server"),
        value("file", "--file <path>", "write into a database other than the one in this folder"),
        flag("json", "--json", "structured result"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/pull.txt")),
};

pub const CALCULATE: CommandSpec = CommandSpec {
    name: "calculate",
    description: "run a named calculation without writing athlete data",
    group: Some(GROUP_READ),
    arguments: &[required("operation", "one of: summary, baseline, trend, coverage, compare-sources, correlation, workout-drift, provider-period-summary"), required("quantity", "measurement quantity, provider metric, or summary type")],
    options: &[
        value("record_kind", "--record-kind <kind>", "one of: measurement, provider-score, provider-summary, session-summary").default_text("measurement"),
        value("period_type", "--period-type <type>", "provider period type; required for provider-period-summary"),
        value("source", "--source <id>", "use one source only"),
        value("compare_source", "--compare-source <id>", "second source for compare-sources or correlation"),
        value("other_quantity", "--other-quantity <quantity>", "second quantity for correlation"),
        value("session", "--session <record-id>", "workout session for workout-drift"),
        value("from", "--from <time>", "earliest day (YYYY-MM-DD) or exact UTC timestamp"),
        value("to", "--to <time>", "latest day (YYYY-MM-DD) or exact UTC timestamp"),
        value("timezone", "--timezone <name>", "IANA timezone for calendar operations"),
        value("group", "--group <period>", "none, day, week, or month").default_text("none"),
        value("file", "--file <path>", "read a database other than the one in this folder"),
        flag("json", "--json", "structured calculation receipt"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/calculate.txt")),
};

pub const SERIES: CommandSpec = CommandSpec {
    name: "series",
    description: "read a sample series back: one row per day, or the raw samples",
    group: Some(GROUP_READ),
    arguments: &[required("quantity", "one of: heart_rate, hrv_beats, ecg_beats, hrv_sdnn, respiratory_rate, oxygen_saturation, steps, active_energy, distance_walking_running, distance_cycling, distance_swimming, running_speed, running_power, running_stride_length, running_vertical_oscillation, running_ground_contact_time, physical_effort, basal_energy, exercise_time, flights_climbed, walking_speed, walking_step_length, walking_asymmetry_percentage, time_in_daylight")],
    options: &[
        value("from", "--from <time>", "earliest day (YYYY-MM-DD) or exact UTC timestamp"),
        value("to", "--to <time>", "latest day (YYYY-MM-DD) or exact UTC timestamp"),
        value("source", "--source <id>", "one source only, for when two measured the same thing — `ath stats` lists the ids"),
        value("timezone", "--timezone <name>", "IANA timezone for calendar days; uses the athlete profile when omitted"),
        value("limit", "--limit <count>", "maximum samples to read").positive_integer().default_integer(10000),
        flag("raw", "--raw", "every sample, rather than one row per day"),
        flag("json", "--json", "structured output, for an agent rather than a person"),
        value("file", "--file <path>", "read a file other than the one in this folder"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/series.txt")),
};

pub const STATS: CommandSpec = CommandSpec {
    name: "stats",
    description: "what is in the file: counts, date ranges, sources, and baselines",
    group: Some(GROUP_READ),
    arguments: &[optional("file", "the file to summarize (default: the one in this folder)")],
    options: &[
        value("file", "--file <path>", "summarize a database other than the one in this folder"),
        flag("json", "--json", "structured output, for an agent rather than a person"),
    ],
    subcommands: &[],
    action: true,
    examples: Some(include_str!("examples/stats.txt")),
};

pub const RECORDS: CommandSpec = CommandSpec {
    name: "records",
    description: "find stable record IDs, inspect one record, and read its revision history",
    group: Some(GROUP_READ),
    arguments: &[],
    options: &[],
    subcommands: &[RECORDS_LIST, RECORDS_SHOW],
    action: false,
    examples: Some(include_str!("examples/records.txt")),
};

pub const RECORDS_LIST: CommandSpec = CommandSpec {
    name: "list",
    description: "list current records in stable order",
    group: None,
    arguments: &[],
    options: &[
        value("type", "--type <record-type>", "show one record type"),
        flag("include_deleted", "--include-deleted", "include records hidden by a normal deletion"),
        value("limit", "--limit <count>", "maximum records to return").positive_integer().default_integer(100),
        value("file", "--file <path>", "read a database other than the one in this folder"),
        flag("json", "--json", "structured output, for an agent rather than a person"),
    ],
    subcommands: &[],
    action: true,
    examples: None,
};

pub const RECORDS_SHOW: CommandSpec = CommandSpec {
    name: "show",
    description: "show one record, one revision, its history, or its links",
    group: None,
    arguments: &[required("record-id", "stable UUIDv7 record ID")],
    options: &[
        value("revision", "--revision <number>", "show this retained revision").positive_integer(),
        flag("history", "--history", "include the revision timeline"),
        flag("links", "--links", "include incoming and outgoing record links"),
        value("file", "--file <path>", "read a database other than the one in this folder"),
        flag("json", "--json", "structured output, for an agent rather than a person"),
    ],
    subcommands: &[],
    action: true,
    examples: None,
};
