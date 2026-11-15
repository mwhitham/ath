/**
 * Freeze the command inventory of the 2.1.0 TypeScript `ath` program.
 *
 * Spec 2.1.1 section 4.2 requires an inventory generated from the old program
 * before it is replaced. Every entry must map to a test of the new executable.
 *
 * The script builds the commander program from `src/program.ts` and walks it in
 * declaration order. It records each command's arguments, options, defaults,
 * custom parsers, and the exact `--help` text printed to a pipe. It also runs a
 * fixed set of parse-only probes, such as `--version` and a missing argument, and
 * records their exit status and output streams. No probe opens a database.
 *
 * Run with a checkout of the reference commit, as tools/reference/README.md
 * describes:
 *
 *   ATH_REFERENCE_ROOT=<checkout> node --import <checkout>/node_modules/tsx/dist/esm/index.mjs \
 *     tools/reference/command-inventory.ts
 *
 * Output: build-history/v2.1.1/appendix/command-inventory.json (2-space JSON, trailing newline).
 */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import type { Argument, Command, Option } from "commander";
import { reference, referenceModule, referencePackage, repository as root } from "./reference.ts";

const { CommanderError } = await referencePackage<typeof import("commander")>("commander");
const { buildProgram } = await referenceModule("src/program.ts");
const outPath = join(root, "build-history", "v2.1.1", "appendix", "command-inventory.json");

/** Commander fields this script reads that are not in its public type declarations. */
interface CommandInternals {
  _actionHandler: unknown;
  _hidden: boolean;
  _enablePositionalOptions: boolean;
  _passThroughOptions: boolean;
  _allowUnknownOption: boolean;
  _allowExcessArguments: boolean;
  _combineFlagAndOptionalValue: boolean;
  _showHelpAfterError: boolean | string;
  _showSuggestionAfterError: boolean;
  _versionOptionName?: string;
  _lifeCycleHooks: Record<string, unknown[]>;
  _getHelpOption(): Option | null;
  _getHelpCommand(): Command | null;
  listenerCount(event: string): number;
}

function internals(command: Command): CommandInternals {
  return command as unknown as CommandInternals;
}

/** Option fields that exist at run time but not in commander's type declarations. */
interface OptionInternals {
  conflictsWith: string[];
  implied: Record<string, unknown> | undefined;
}

/**
 * Commander uses this width when standard output is not a terminal. Fixing it
 * makes the recorded help equal to `ath <command> --help | cat`.
 */
const PIPE_HELP_WIDTH = 80;

/** A value commander may hold as a default, made safe for JSON. */
function jsonValue(value: unknown): unknown {
  if (value === undefined) return null;
  if (typeof value === "function") return `[function ${value.name || "anonymous"}]`;
  return JSON.parse(JSON.stringify(value)) as unknown;
}

function commandPath(command: Command): string {
  const names: string[] = [];
  for (let current: Command | null = command; current?.parent; current = current.parent) {
    names.unshift(current.name());
  }
  return names.join(" ");
}

function describeArgument(argument: Argument) {
  return {
    name: argument.name(),
    required: argument.required,
    variadic: argument.variadic,
    description: argument.description,
    default_value: jsonValue(argument.defaultValue),
    default_value_description: argument.defaultValueDescription ?? null,
    choices: argument.argChoices ?? null,
    custom_parser: argument.parseArg ? argument.parseArg.name || "anonymous" : null,
  };
}

/**
 * The value commander holds for an option before any argument is parsed.
 * Commander gives a lone `--no-foo` option (no `--foo` beside it) a default of
 * true, but only when that command starts parsing. Apply the same rule here.
 */
function effectiveDefault(command: Command, option: Option) {
  const attribute = option.attributeName();
  const value = command.getOptionValue(attribute) as unknown;
  if (
    value === undefined &&
    option.negate &&
    option.defaultValue === undefined &&
    option.long !== undefined &&
    !command.options.some((other) => other.long === option.long!.replace(/^--no-/, "--"))
  ) {
    return { value: true, source: "default" };
  }
  return { value: jsonValue(value), source: command.getOptionValueSource(attribute) ?? null };
}

function describeOption(command: Command, option: Option) {
  const attribute = option.attributeName();
  const effective = effectiveDefault(command, option);
  return {
    flags: option.flags,
    long: option.long ?? null,
    short: option.short ?? null,
    attribute_name: attribute,
    description: option.description,
    takes_value: option.required || option.optional,
    value_required: option.required,
    value_optional: option.optional,
    variadic: option.variadic,
    mandatory: option.mandatory,
    negatable_no_form: option.negate,
    declared_default: jsonValue(option.defaultValue),
    default_value_description: option.defaultValueDescription ?? null,
    effective_default: effective.value,
    effective_default_source: effective.source,
    preset: jsonValue(option.presetArg),
    env_var: option.envVar ?? null,
    choices: option.argChoices ?? null,
    conflicts_with: (option as unknown as OptionInternals).conflictsWith,
    implies: jsonValue((option as unknown as OptionInternals).implied),
    hidden: option.hidden,
    help_group: option.helpGroupHeading ?? null,
    custom_parser: option.parseArg ? option.parseArg.name || "anonymous" : null,
  };
}

function describeHelpOption(command: Command) {
  const option = internals(command)._getHelpOption();
  if (option === null) return null;
  return {
    flags: option.flags,
    long: option.long ?? null,
    short: option.short ?? null,
    description: option.description,
  };
}

function describeImplicitHelpCommand(command: Command) {
  const help = internals(command)._getHelpCommand();
  if (help === null) return null;
  return {
    name: help.name(),
    usage: `${help.name()} ${help.registeredArguments
      .map((argument) =>
        argument.required ? `<${argument.name()}>` : `[${argument.name()}]`,
      )
      .join(" ")}`.trim(),
    description: help.description(),
    arguments: help.registeredArguments.map(describeArgument),
  };
}

function describeSettings(command: Command) {
  const inner = internals(command);
  return {
    has_action: inner._actionHandler !== null,
    hidden: inner._hidden,
    enable_positional_options: inner._enablePositionalOptions,
    pass_through_options: inner._passThroughOptions,
    allow_unknown_option: inner._allowUnknownOption,
    allow_excess_arguments: inner._allowExcessArguments,
    combine_flag_and_optional_value: inner._combineFlagAndOptionalValue,
    show_help_after_error: inner._showHelpAfterError,
    show_suggestion_after_error: inner._showSuggestionAfterError,
    lifecycle_hooks: Object.fromEntries(
      Object.entries(inner._lifeCycleHooks).map(([event, hooks]) => [event, hooks.length]),
    ),
    help_text_listeners: {
      before_all: inner.listenerCount("beforeAllHelp"),
      before: inner.listenerCount("beforeHelp"),
      after: inner.listenerCount("afterHelp"),
      after_all: inner.listenerCount("afterAllHelp"),
    },
  };
}

/**
 * The exact text `ath <path> --help` prints when standard output is a pipe:
 * no colour and commander's default 80-column width.
 */
function renderedHelp(command: Command): string {
  let stdout = "";
  let stderr = "";
  command.configureOutput({
    writeOut: (text) => {
      stdout += text;
    },
    writeErr: (text) => {
      stderr += text;
    },
    getOutHelpWidth: () => PIPE_HELP_WIDTH,
    getErrHelpWidth: () => PIPE_HELP_WIDTH,
    getOutHasColors: () => false,
    getErrHasColors: () => false,
  });
  command.outputHelp();
  if (stderr !== "") throw new Error(`help for '${commandPath(command)}' wrote to stderr`);
  return stdout;
}

function describeCommand(command: Command) {
  const path = commandPath(command);
  return {
    path,
    name: command.name(),
    parent_path: command.parent?.parent ? commandPath(command.parent) : null,
    aliases: command.aliases(),
    description: command.description(),
    summary: command.summary(),
    usage: command.usage(),
    help_group: command.helpGroup() || null,
    arguments: command.registeredArguments.map(describeArgument),
    options: command.options.map((option) => describeOption(command, option)),
    help_option: describeHelpOption(command),
    implicit_help_command: describeImplicitHelpCommand(command),
    subcommands: command.commands.map((sub) => sub.name()),
    settings: describeSettings(command),
    help_text: renderedHelp(command),
  };
}

function walk(command: Command, out: ReturnType<typeof describeCommand>[]): void {
  for (const sub of command.commands) {
    out.push(describeCommand(sub));
    walk(sub, out);
  }
}

/**
 * Parse-only probes. Each one stops inside commander or the argument parser
 * before any action runs, so none reads the working directory or a database.
 * The `ath:` prefix and exit status 1 for a non-commander error copy
 * `src/cli.ts`, which catches the error the same way.
 */
const PROBES: readonly (readonly string[])[] = [
  ["--version"],
  ["-V"],
  ["--help"],
  ["-h"],
  ["help"],
  ["nosuch"],
  ["--nosuch"],
  ["log", "--update"],
  ["init", "--update"],
  ["conflicts"],
  ["records"],
  ["conflicts", "help"],
  ["conflicts", "help", "show"],
  ["records", "help", "list"],
  ["records", "nosuch"],
  ["skill", "help"],
  ["key", "help"],
  ["export"],
  ["link", "result-id"],
  ["link", "result-id", "session-id"],
  ["correct", "record-id", "--revision", "1"],
  ["conflicts", "resolve", "record-id"],
  ["grade", "prediction-id"],
  ["apply", "heart-rate-zones"],
  ["pull", "garmin"],
  ["init", "--units"],
  ["records", "list", "--limit", "0"],
  ["records", "list", "--limit", "1.5"],
  ["series", "heart_rate", "--limit", "abc"],
  ["delete", "record-id", "--revision", "0"],
  ["export", "out", "extra"],
];

function setUpForProbe(command: Command, sink: { stdout: string; stderr: string }): void {
  command.exitOverride();
  command.configureOutput({
    writeOut: (text) => {
      sink.stdout += text;
    },
    writeErr: (text) => {
      sink.stderr += text;
    },
    getOutHelpWidth: () => PIPE_HELP_WIDTH,
    getErrHelpWidth: () => PIPE_HELP_WIDTH,
    getOutHasColors: () => false,
    getErrHasColors: () => false,
  });
  // Actions must never run during a probe.
  if (internals(command)._actionHandler !== null) {
    command.action(() => {
      throw new Error("probe reached an action");
    });
  }
  for (const sub of command.commands) setUpForProbe(sub, sink);
}

async function runProbe(argv: readonly string[]) {
  const program = buildProgram();
  const sink = { stdout: "", stderr: "" };
  setUpForProbe(program, sink);
  let exitStatus: number;
  let commanderCode: string | null = null;
  try {
    await program.parseAsync(["node", "ath", ...argv]);
    throw new Error(`probe 'ath ${argv.join(" ")}' completed without stopping`);
  } catch (error) {
    if (error instanceof CommanderError) {
      exitStatus = error.exitCode;
      commanderCode = error.code;
    } else if (error instanceof Error && error.message === "probe reached an action") {
      throw new Error(`probe 'ath ${argv.join(" ")}' reached an action`);
    } else {
      // src/cli.ts: console.error(`ath: ${message}`) and exit status 1.
      sink.stderr += `ath: ${(error as Error).message}\n`;
      exitStatus = 1;
    }
  }
  return {
    argv: [...argv],
    exit_status: exitStatus,
    commander_code: commanderCode,
    stdout: sink.stdout,
    stderr: sink.stderr,
  };
}

async function main(): Promise<void> {
  const packageJson = JSON.parse(readFileSync(join(reference, "package.json"), "utf8")) as {
    version: string;
  };
  const commanderPackage = JSON.parse(
    readFileSync(join(reference, "node_modules", "commander", "package.json"), "utf8"),
  ) as { version: string };

  const program = buildProgram();
  const inner = internals(program);
  const versionOption =
    program.options.find((option) => option.attributeName() === inner._versionOptionName) ?? null;

  const commands: ReturnType<typeof describeCommand>[] = [];
  walk(program, commands);

  const probes = [];
  for (const argv of PROBES) probes.push(await runProbe(argv));

  const inventory = {
    generated_by: relative(root, fileURLToPath(import.meta.url)),
    source: {
      program_factory: "src/program.ts buildProgram()",
      entry_point: "src/cli.ts",
      package_version: packageJson.version,
      commander_version: commanderPackage.version,
      help_rendering:
        "outputHelp() captured as written to a pipe: no colour, commander default width 80",
    },
    program: {
      name: program.name(),
      description: program.description(),
      usage: program.usage(),
      version: program.version() ?? null,
      version_option:
        versionOption === null
          ? null
          : {
              flags: versionOption.flags,
              long: versionOption.long ?? null,
              short: versionOption.short ?? null,
              description: versionOption.description,
              behavior: "prints the version and a newline to stdout, exit status 0",
            },
      help_option: describeHelpOption(program),
      global_options: program.options.map((option) => describeOption(program, option)),
      implicit_help_command: describeImplicitHelpCommand(program),
      subcommands: program.commands.map((sub) => sub.name()),
      settings: describeSettings(program),
      help_text: renderedHelp(program),
    },
    command_count: commands.length,
    option_count: commands.reduce((total, command) => total + command.options.length, 0),
    commands,
    probes,
  };

  writeFileSync(outPath, `${JSON.stringify(inventory, null, 2)}\n`);
  console.log(
    `wrote ${relative(root, outPath)}: ${commands.length} commands, ` +
      `${inventory.option_count} command options, ${program.options.length} global options, ` +
      `${probes.length} probes`,
  );
}

await main();
