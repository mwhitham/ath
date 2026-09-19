/**
 * The skill's command reference, built from the CLI's own help (D80).
 *
 * `skill/references/cli.md` is generated from this and committed. A test fails
 * when it is behind the code, the same way the JSON Schema and the fixture are
 * checked. Regenerate, do not hand-edit.
 */
import type { Command } from "commander";

/** Fixed so the output does not depend on the terminal it was generated in. */
const HELP_WIDTH = 80;

interface HelpEntry {
  /** `ath key set` */
  path: string;
  summary: string;
  help: string;
}

function fixWidth(cmd: Command): void {
  cmd.configureHelp({ helpWidth: HELP_WIDTH });
  for (const sub of cmd.commands) fixWidth(sub);
}

function walk(cmd: Command, prefix: string, out: HelpEntry[]): void {
  for (const sub of cmd.commands) {
    if (sub.name() === "help") continue;
    const path = `${prefix} ${sub.name()}`;
    out.push({ path, summary: sub.description(), help: sub.helpInformation().trimEnd() });
    walk(sub, path, out);
  }
}

/** Every command the tree holds, as `ath key set`, in registration order. */
export function commandPaths(program: Command): string[] {
  const entries: HelpEntry[] = [];
  walk(program, program.name(), entries);
  return entries.map((e) => e.path);
}

export function renderCliReference(program: Command): string {
  fixWidth(program);
  const entries: HelpEntry[] = [];
  walk(program, program.name(), entries);

  const anchor = (path: string) => path.replace(/\s+/g, "-");

  const lines: string[] = [
    `# ath command reference`,
    ``,
    `Generated from the CLI's own help by \`pnpm generate:skill-cli\`. Do not edit by hand.`,
    `This is the text \`ath <command> --help\` prints, for every command, so an agent`,
    `can read the whole surface without a round trip.`,
    ``,
    `Every command takes \`--json\` where output is meant for an agent. Commands that`,
    `write show a summary and ask once; \`-y\` answers yes in advance. Where \`--dry-run\``,
    `is offered, it shows the write and writes nothing.`,
    ``,
    `## Contents`,
    ``,
    `- [ath](#ath)`,
    ...entries.map((e) => `- [${e.path}](#${anchor(e.path)}) — ${e.summary}`),
    ``,
    `## ath`,
    ``,
    "```",
    program.helpInformation().trimEnd(),
    "```",
  ];

  for (const e of entries) {
    lines.push(``, `## ${e.path}`, ``, "```", e.help, "```");
  }

  return `${lines.join("\n")}\n`;
}
