/**
 * Help written for someone who has never run this (D50).
 *
 * The old help listed flags and nothing else, which tells a reader what exists and
 * not what to type. So every command carries worked examples with real values, the
 * command list is grouped by what the reader is trying to do, and bare `ath` says
 * what to do next rather than printing everything at once.
 *
 * `ath log` gets the longest example of any command, because it is the one used most
 * and the only one that reads what a person typed (D60).
 */
import { existsSync, readdirSync } from "node:fs";
import { FILE_SUFFIX } from "./file.js";

/** The four things a reader might be trying to do. Commander prints these as headings. */
export const GROUPS = {
  setUp: "Set up:",
  getIn: "Get your data in:",
  read: "Read your data:",
  predict: "Predict and grade:",
} as const;

/**
 * What bare `ath` prints.
 *
 * Different with and without a file, because the two readers want different things:
 * one has nothing yet and needs a first command, the other has a file and needs to
 * know what can be done to it.
 */
export function bareGuide(cwd: string): string {
  const files = existsSync(cwd) ? readdirSync(cwd).filter((f) => f.endsWith(FILE_SUFFIX)) : [];

  if (files.length === 0) {
    return [
      `ath — Athletic Standard, an open file format for your training and recovery data.`,
      ``,
      `There is no ${FILE_SUFFIX} file in this folder, so start with one:`,
      ``,
      `  ath init                       make the file, here`,
      `  ath import <export>            load an Apple Health, WHOOP or Oura export into it`,
      ``,
      `Then you can log what you did, read it back, and predict against it:`,
      ``,
      `  ath log slept badly, about 5 hours`,
      `  ath stats`,
      `  ath predict fran --json`,
      ``,
      `Every command explains itself: \`ath <command> --help\`, with examples.`,
      `The whole list: \`ath --help\`.`,
    ].join("\n");
  }

  const name = files[0]!;
  return [
    `ath — Athletic Standard. Working on ${name} in this folder.`,
    ``,
    `  ath stats                      what is in the file, and where it came from`,
    `  ath records list               stable IDs for the records in the database`,
    `  ath log HRV 61 this morning    write something down`,
    `  ath import <export>            load more device data`,
    `  ath predict fran               predict Fran — needs a model in a bare terminal`,
    `  ath check                      make sure the file is still sound`,
    ``,
    `Every command explains itself: \`ath <command> --help\`, with examples.`,
    `The whole list: \`ath --help\`.`,
  ].join("\n");
}

/** Worked examples, one block per command, with real values rather than placeholders. */
export const EXAMPLES: Record<string, string> = {
  init: `
Examples:

  Answer three questions, then write the file:
    $ ath init

  Skip the questions:
    $ ath init -y --name "Sam Reyes" --birth-year 1991 --sex female

The file is created here, called athlete${FILE_SUFFIX}, with the well-known
benchmarks already defined. Nothing leaves your machine.

For a complete v1 package restore, create an empty destination:
    $ ath init -y --empty
    $ ath import ./saved-package
    $ ath import ./saved-package --yes

The agent skill is written to .agents/skills, which Claude Code, Cursor,
Codex, and Gemini CLI read, and into any of .claude, .cursor, .codex,
.gemini, or .github already here. An agent in this folder then knows how
to read and write the file. Pass --no-skill to skip that.`,

  skill: `
Examples:

  Where the skill is installed, and whether each copy is current:
    $ ath skill

  Upgrade ath, and replace the skill here if one is already installed:
    $ ath --update

  Replace the skill without upgrading ath:
    $ ath skill install

The skill is a folder an agent reads to learn the file and the commands.
ath init writes it to .agents/skills and into any harness folder already
here. Each copy carries its version. When a copy is older than the one
this ath ships, other commands say so once on stderr. \`ath --update\`
replaces an installed copy after upgrading ath. This command replaces
it without upgrading.`,

  check: `
Examples:

  Check the file in this folder:
    $ ath check

  Check one somewhere else:
    $ ath check ~/training/athlete${FILE_SUFFIX}

An error means the database breaks a rule of the format and says which rule.
A warning means something is worth knowing but the database is still valid.
The check covers SQLite integrity, foreign keys, domain rules, and attachments.`,

  import: `
Examples:

  Preview a complete v1 package:
    $ ath import ~/Backups/athletic-standard-export

  Apply it only after the preview has no conflicts:
    $ ath import ~/Backups/athletic-standard-export --yes

The default is always a preview. \`--json\` is also only a preview and
never confirms a write. An empty database adopts the package database ID
and exact revision history. A database with the same ID can accept new
records and later revisions.

An import never deletes a local record merely because the package omits
it. A different database ID is refused because it may belong to another
athlete. Athletic Standard 0.x JSON files are prerelease formats and are
not imported by v1.`,

  export: `
Examples:

  Write a complete portable package:
    $ ath export ./athlete-backup

  Return the command result as JSON:
    $ ath export ./athlete-backup --json

SQLite remains the live record. The exported JSON package is a snapshot
and does not stay synchronized. The package includes complete current
records, retained revisions, deletions, dense samples, and permitted
attachments. The manifest is written last.

The export has no time by default, so two exports of the same database
revision are byte-identical. Supply a fixed --exported-at only when a
manifest time is needed.`,

  log: `
Examples:

  A note, taken exactly as typed. No quotes needed:
    $ ath log slept badly, about 5 hours

  A manual measurement. Name RMSSD or SDNN rather than saying only HRV:
    $ ath log RMSSD 61 ms

  How something felt, with a rating:
    $ ath log sore quads 4/5

  A benchmark result:
    $ ath log Fran in 4:41 rx

  A structured record from an agent. Run \`ath log\`, paste one record
  object or an array, and press Ctrl-D:
    $ ath log
    {"record_type":"goal","data":{"authorship":{...},"title":"Fran under 4:30"}}
    ^D

Plain text creates one record. A structured JSON array creates one
validated batch. The preview shows every stable ID and complete record
before one transaction writes anything. \`--json\` returns that preview
and never confirms a write. Pass \`--yes\` only after approval.`,

  link: `
Examples:

  Find the stable IDs and current result revision:
    $ ath records list --type benchmark_result
    $ ath records list --type workout_session

  Preview the link:
    $ ath link <result-id> <session-id> --revision 1 --json

  Write it after review:
    $ ath link <result-id> <session-id> --revision 1 --yes

The benchmark result owns this link. Linking creates a new revision of
that result. A stale --revision is refused rather than overwriting a
correction made after the preview.`,

  predict: `
Examples:

  Predict Fran. Needs a gateway key in a bare terminal:
    $ ath models
    $ ath predict fran --model qwen/qwen3-235b-a22b

  Save a usual model, then predict without naming it:
    $ ath models --default openai/gpt-oss-120b
    $ ath predict fran

  Evidence for a harness, not a prediction. No key needed:
    $ ath predict fran --json

A prediction needs a model. In Claude Code, Cursor, or Codex the model
is already there: pull evidence with --json, reason, write with
\`ath log\`. In a bare terminal this command calls the model you chose,
prints the number, and asks once before writing.

--as-of hides everything after that day and does not write. Replaying
the past is \`ath backtest\`.`,

  key: `
Examples:

  Save a Vercel or OpenRouter key in the computer's password store:
    $ ath key set vercel
    $ ath key set openrouter

  See which gateway is saved, not the key itself:
    $ ath key

  Delete it:
    $ ath key clear

There is no Athletic Standard subscription. A key is only for calling a
model from a bare terminal. Inside a harness you do not need one.

The key never goes in the athlete file. Agents read that file. If the
password store is missing, the command refuses. It will not write a
plaintext file. A key already in AI_GATEWAY_API_KEY or OPENROUTER_API_KEY
still works, for scripts.`,

  models: `
Examples:

  Every live text model that can reason, including open weights:
    $ ath models

  Save your usual model next to the athlete file:
    $ ath models --default openai/gpt-oss-120b

Needs a gateway key. The list is fetched each time, so a new open-weight
model appears without a new ath. Image, audio, and embedding models are
out. We do not pick a favourite lab for you.`,

  backtest: `
Examples:

  Replay history with the models you name:
    $ ath backtest --model qwen/qwen3-235b-a22b --model openai/gpt-oss-120b

  Every live text model that can reason, after showing the count:
    $ ath backtest --all

A prediction needs a model, so a key is required. No key → refuse. In a
harness, daily predictions use the harness model; this command is for
comparing models on a history you already have.

A result is replayed only when an earlier result on the same benchmark
exists. Evidence is as of the day before. The athlete file is not
written. The report's summary is the later share payload.`,

  share: `
Examples:

  Name the latest backtest report:
    $ ath share

Not built yet. The shareable part of a backtest is the summary in the
report file this names.`,

  grade: `
Examples:

  Find the prediction and result IDs:
    $ ath records list --type prediction
    $ ath records list --type benchmark_result

  Preview the exact link and grade:
    $ ath grade <prediction-id> --result <result-id> --json

  Write after review:
    $ ath grade <prediction-id> --result <result-id> --yes

\`ath grade\` never logs a result. It grades two records that already
exist. It creates a stable prediction-result link when needed and saves
the deterministic arithmetic against exact revisions.

A result inside the prediction's stated range is a hit. Anything else is
a miss. A prediction error analysis is a separate authored record written
through structured \`ath log\`, not through a grade flag.`,

  correct: `
Examples:

  Read the current revision and complete data:
    $ ath records show <record-id> --history --json

  Preview a complete replacement:
    $ ath correct <record-id> --revision 2 --data '{"...":"..."}' --json

  Write after review:
    $ ath correct <record-id> --revision 2 --data '{"...":"..."}' --yes

Correction keeps the stable ID and complete earlier revisions. The
expected revision prevents a stale preview from overwriting newer data.`,

  delete: `
Examples:

  Preview a normal deletion:
    $ ath delete <record-id> --revision 2 --json

  Hide it from current views after review:
    $ ath delete <record-id> --revision 2 --yes

Normal deletion retains the complete record and revision history. Privacy
erasure is a different command because it removes values and cannot be
undone from the database.`,

  stats: `
Examples:

  What is in the file, and where it came from:
    $ ath stats

  As data, for an agent:
    $ ath stats --json

Baselines are listed per source and never averaged across them, because
two devices measuring one thing give two answers. Each carries how many
observations it rests on, over what days, and the rule used.

Each source also lists the devices seen writing under it, with the days
each one covered — which is where a replaced watch becomes visible.

Predictions are counted by who made them, so you can see which agent and
model has actually been right about you.`,

  series: `
Examples:

  One row per day:
    $ ath series heart_rate

  A window, and one device:
    $ ath series hrv_beats --from 2026-08-01 --to 2026-08-31 --source whoop-1

  Every sample, not a daily summary:
    $ ath series heart_rate --raw --from 2026-08-14 --to 2026-08-14

Calendar days use the timezone saved in the athlete profile. Pass a different
one explicitly when needed:
    $ ath series heart_rate --from 2026-08-14 --to 2026-08-14 --timezone America/Denver

Dense samples live in SQLite. This command reads them without replacing the
provider summaries that may describe the same workout or night. \`ath stats\`
lists which series exist.`,

  records: `
Examples:

  Find stable IDs for all current records:
    $ ath records list

  List only predictions:
    $ ath records list --type prediction

  Show one record and its complete revision timeline:
    $ ath records show 01890f3e-e7d3-7cc0-88c8-000000000010 --history

  Show the records it links to and the records that link to it:
    $ ath records show 01890f3e-e7d3-7cc0-88c8-000000000010 --links

\`ath records list\` is for finding stable IDs. \`ath records show\` reads one
logical record, a named revision, its history, and its links. Use \`ath stats\`
for the fixed overview and \`ath series\` for dense samples.`,
};

/** The line under the grouped command list on bare `ath --help`. */
export const HELP_FOOTER = `
Every command has examples of its own: \`ath <command> --help\`.

The database is yours and it is local. SQLite is the live record. JSON exports
are portable snapshots and do not stay synchronized. Device measurements and
things you reported yourself are kept apart on purpose.

A prediction needs a model. In a harness the model is already there.
There is no Athletic Standard subscription.

Not an app, not a coach, not medical advice.`;
