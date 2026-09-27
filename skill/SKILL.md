---
name: athletic-standard
description: Reads and writes an athlete's Athletic Standard file (.ath.json) with the ath CLI. Applies when someone logs a workout, a lift, a benchmark result, HRV, resting heart rate, sleep, soreness, or how they feel; asks what their training or recovery data shows; asks for a prediction on a benchmark such as Fran or a 5k; reports a result to grade a prediction against; wants to backtest predictions on past results; or imports an Apple Health, WHOOP, or Oura export. Also applies to any question about a .ath.json file or the ath command.
license: Apache-2.0
compatibility: Requires the ath CLI (npm package athleticstandard) on PATH. No network needed.
metadata:
  version: "0.4.1"
---

# Athletic Standard

An Athletic Standard file is one JSON document holding one athlete's training and
recovery data. It sits on the athlete's own machine. `ath` is the command that reads
and writes it.

This skill expects format version **0.4.1**. `ath check` prints the version of the
file in front of you. A file on an older version still loads; a newer one may hold
fields described nowhere here, so say so rather than guessing at them.

Not an app, not a coach, not medical advice. Describe what the data shows and what it
does not. Do not prescribe training or diagnose anything.

## Start here

1. Run `ath stats --json`. It says what is in the file, which device wrote each
   measurement, over what days, and how many observations each average rests on. Run
   it every time rather than working from an earlier run. The file changes underneath
   you.
2. Pick the command from the table below. Full options and examples for every one are
   in [references/cli.md](references/cli.md).
3. Pass `--json`. Every command that produces output for you has it.
4. Before any command that writes (`log`, `link`, `grade`, `import`), show the athlete
   what will be written and wait for a yes. Pass `-y` only after they have agreed.
5. For a prediction: run `ath predict <benchmark> --json` to get the evidence, write
   the prediction yourself from it, then save it with `ath log`. You are the model, so
   no key and no gateway are needed.

## The one rule the format is built on

Measured signals and self-reported signals never mix. A measurement came off a device
and names the source that wrote it. A self-reported entry is what the athlete said,
and it structurally cannot name a device.

The consequence for you: a predicted number comes from measured signals. What the
athlete reported about themselves can widen or narrow your confidence and can explain
a result afterwards, but it does not move the number.

## What `ath stats` tells you that decides what you can say

- **Sources.** Each app or device is its own source. Two of them measuring the same
  quantity give two answers and are never averaged together. If they disagree, say
  which one you used and by how much they differ.
- **Coverage.** Every number the tool prints carries the count it rests on, the days
  it covers, how many of those days actually have data, and the rule used. A mean
  over 90 nights and a mean over 4 look identical until you read that line.

## The commands

Set up:

| Command | What it does |
|---|---|
| `ath init` | create a new athlete file here, with the well-known benchmarks defined |
| `ath skill` | show where this skill is installed and whether it is current; `ath skill install` refreshes it |
| `ath key` | save or show a gateway key for terminal use; never stored in the athlete file |
| `ath models` | list live text models that can reason, or save the usual one |

Get data in:

| Command | What it does |
|---|---|
| `ath import <path>` | load an Apple Health, WHOOP, or Oura export into the file |
| `ath log [entry...]` | write a workout result, a measurement, how they felt, or a prediction |
| `ath link <result> <session>` | attach a workout result to the device session it happened in |

Read:

| Command | What it does |
|---|---|
| `ath check [file]` | whether the file still obeys every rule of the format |
| `ath series <quantity>` | a sample stream back, one row per day or every raw sample |
| `ath stats [file]` | what is in the file: counts, date ranges, sources, and baselines |

Predict and grade:

| Command | What it does |
|---|---|
| `ath predict <benchmark> --json` | the evidence for a prediction; the prediction is yours to write |
| `ath grade <benchmark> --actual <score>` | record what happened and score the prediction against it |
| `ath backtest` | replay past results through one or more models; writes a report, not the athlete file |
| `ath share` | not built yet; names the latest backtest report |

Full options, arguments, and examples for each: [references/cli.md](references/cli.md).

`key`, `models`, `backtest`, and `share` are for a person at a terminal. They call a
model through a gateway the person paid for. Inside a harness you are the model, so
you do not use them. Bare `ath predict` (without `--json`) also calls a gateway; in a
harness always pass `--json`.

Common flags: `--file <path>` points at a file other than the one in this folder.
`--dry-run` shows what a write would do and writes nothing. `--as-of <date>` makes
`predict` see only what was known on that day.

## What you must hold, because no tool can

- **Recorded activity is not actual activity.** A day with no workout session recorded
  is a day with no recording. It may or may not have been a rest day. Say which you
  mean.
- **Association is not cause.** Two things moving together is a thing to notice, not a
  reason. When you name a cause, say what would have to be true for it to be one.
- **Every claim traces to a row.** Cite the date and the value. If you cannot point at
  the row, do not make the claim.
- **Name what is missing.** Gaps change an answer as much as the data does. `ath
  predict` names its own gaps; carry them into what you say.
- **A number the athlete typed is not a device reading.** `ath log` files it under the
  manual source. Never present it as if a device measured it.

## What you do not need to hold, because the tools do

Units are converted at import and stored canonically. Duplicate records are
reconciled by type, timestamp, and source. Averages are never pooled across sources.
Sleep is stored as actual sleep and as time in bed, separately, and every command
that prints one says which it printed. SDNN and RMSSD are separate measurements and
never share a baseline.

Do not re-implement any of that. If a command seems to be getting it wrong, that is a
bug in the command, not something to work around in prose.

## Open these when the question needs them

- [references/cli.md](references/cli.md) — every command, its options, and its
  examples, as `ath <command> --help` prints them.
- [references/format.md](references/format.md) — the record types, their fields, and
  their units.
- [references/logging.md](references/logging.md) — how to turn what someone said into
  a record, and how to name a workout they did not name.
- [references/predicting.md](references/predicting.md) — how to read the evidence
  package and write a prediction worth grading.
- [references/grading.md](references/grading.md) — what to do with a hit, and what to
  do with a miss.
