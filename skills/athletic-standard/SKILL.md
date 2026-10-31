---
name: athletic-standard
description: Reads and writes an athlete's Athletic Standard SQLite database (.ath.sqlite) with the ath CLI. Applies when someone logs a workout, lift, named benchmark such as Fran, HRV as RMSSD or SDNN, resting heart rate, sleep, soreness, nutrition, or another athlete report; asks what training or recovery data shows; asks for a benchmark prediction or backtest; reports a result to grade; imports Apple Health, WHOOP, Oura, Garmin, or Strava, or an Athletic Standard 2.0.0 or 2.1.0 package; reads an Open Wearables or Junction JSON file through a mapping file; exports a complete package; runs ath apply on one record; or asks about Athletic Standard records, revisions, calculations, deletion, or privacy erasure.
license: MIT
compatibility: Requires the ath CLI (npm package athleticstandard) on PATH. Local database use needs no network. Optional terminal model calls need a configured gateway.
metadata:
  version: "2.1.0"
---

# Athletic Standard

The live local record is one SQLite database named `athlete.ath.sqlite`. `ath` reads
and writes it. A JSON package is an explicit complete snapshot. It is not a second
live copy.

This skill expects Athletic Standard **2.1.0**. A version 1.0.0 or 2.0.0 database
upgrades with `ath migrate`. A 2.0.0 package still imports. Prerelease `0.x` JSON
documents are not imported.

Local use needs no account, network connection, subscription, or hosted service.
Optional bare-terminal prediction and backtest commands can call a model gateway.

The database contains sensitive health data. It is not encrypted by `ath`. Recommend
operating-system disk encryption. Anyone who can read the file can read the athlete
record.

Not an app, not a coach, not medical advice. Describe what the data shows. Do not
prescribe training or diagnose a condition.

## Start here

1. Run `ath stats --json`. It reports current counts, sources, coverage, and
   baselines.
2. Use `ath records list --json` when stable record IDs are needed.
3. Choose a command below. Full options and examples are in
   [references/cli.md](references/cli.md).
4. Pass `--json` for structured output. On commands that require confirmation,
   `--json` is a preview and never acts as `--yes`.
5. Before changing athlete data with `log`, `link`, `correct`, `delete`, `erase`,
   `grade`, or `import`, show the preview and wait for approval. Then run the command
   without `--json` and confirm, or pass `--yes`.
6. Erasure needs a second confirmation. Never supply
   `--confirm-erasure <record-id>` until the full dependency preview has been
   approved.
7. For a prediction, run `ath predict <benchmark> --json`. This returns evidence,
   not a prediction. Reason over it and save a v1 `prediction` record through
   structured `ath log`.

## The two-tier wall

Measured records and athlete reports never mix.

A measured record must have a `source_id`. The database enforces the source
reference. A hand-entered measurement uses the `manual` source. It never counts as a
device measurement or enters a device baseline.

An `athlete_report` or `nutrition_entry` has no source field. It cannot claim that a
device measured the athlete's words, rating, meal, or note.

`intensity` and `Feeling` in `supplied_fields` are labels a source sent. Quote them
only as "the source said". Never use them as evidence. A calculation cannot read
them.

Authored records are separate again. Predictions, grades, plans, goals, reviews, and
model interpretations name their author and exact evidence record revisions.

Use measured records for the predicted number. Athlete reports may change confidence
and may support a later explanation. Keep the distinction explicit.

## Stable IDs and revisions

Every logical record has a UUIDv7 `record_id` and a positive
`current_revision`. Corrections keep the ID and add a complete revision. Evidence
names both the record ID and revision.

Use:

```sh
ath records list --type <record-type> --json
ath records show <record-id> --history --links --json
```

Do not build links from timestamps. Use stable record IDs.

## Commands

Set up:

| Command | Purpose |
|---|---|
| `ath init` | Create `athlete.ath.sqlite`, a manual source, and standard benchmarks |
| `ath init --empty` | Create a revision-zero restore destination |
| `ath skill` | Show installed skill copies |
| `ath skill install` | Add skill files that are not already installed, and replace an older tool skill |
| `ath --update` | Update `ath`, replace an older tool skill, and add missing skill files |
| `ath key` | Manage an optional terminal model-gateway key outside athlete data |
| `ath models` | List gateway models or save a terminal default |
| `ath migrate` | Preview and apply a SQL schema migration after a verified backup (needs `sqlite3` on PATH) |
| `ath repair` | Repair typed-table bytes after npm 1.0.0 imports; use `--from-revision` when typed columns disagree with the revision (needs `sqlite3` on PATH when candidates exist) |

Import, export, and create:

| Command | Purpose |
|---|---|
| `ath import <path>` | Preview a provider export or complete v1 package |
| `ath import <path> --yes` | Apply a conflict-free preview in one transaction |
| `ath import <path> --mapping <name>` | Preview a JSON file read through a mapping file, such as `open-wearables` or `junction` |
| `ath conflicts show` | Show complete local and imported values from a package conflict |
| `ath conflicts resolve <record-id>` | Record a local, imported, or corrected choice without changing athlete data |
| `ath export <directory>` | Write a complete deterministic JSON package |
| `ath log [entry...]` | Create one plain-text record or one structured object or array |
| `ath link <result-id> <session-id> --revision <n>` | Revise a benchmark result to link a workout session |
| `ath correct <record-id>` | Replace complete current data and retain the old revision |
| `ath delete <record-id>` | Hide the current record while retaining history |
| `ath erase <record-id>` | Irreversibly remove values and every stored dependency |

`ath log --attachment <path>` adds PNG, JPEG, WebP, or PDF files to exactly one
athlete report or nutrition entry. `ath correct --attachment <path>` can add one to
an existing report or nutrition entry. Preview first. `--json` never copies a file.

Read and calculate:

| Command | Purpose |
|---|---|
| `ath check [file]` | Check SQLite integrity, schema versions, foreign keys, revisions, relationships, domain rules, sessions, and attachments |
| `ath stats [file]` | Show the fixed overview |
| `ath records list` | Find current stable IDs in stable order |
| `ath records show <record-id>` | Read a current record, revision, history, and links |
| `ath series <quantity>` | Read dense samples from SQLite |
| `ath calculate <operation> <quantity>` | Run named arithmetic without writing athlete data |
| `ath apply <calculation>` | Run one calculation skill file on one record |
| `ath pull garmin` | Read one day from Garmin's personal connection |

The calculation operations are `summary`, `baseline`, `trend`, `coverage`,
`compare-sources`, `correlation`, `workout-drift`, and
`provider-period-summary`. Use `--record-kind` to choose a measurement, provider
score, provider summary, or summary nested in a session. Measurements are the
default. The provider-period operation also requires `--period-type`. It groups by
the stored Oura day, WHOOP cycle, or Apple activity-summary day without turning that
period into a calendar day.

Results remain separate by source. Manual sources are excluded. RMSSD and SDNN are
different quantities and never share a calculation. A receipt lists at most 1,000
input revisions and says when the list was shortened. Arithmetic refuses more than
100,000 inputs until the request is narrowed.

`ath apply <name>` runs the skill with that name, such as `heart-rate-zones`.
Each calculation is its own skill. The program reads the installed `SKILL.md`.
Editing that file changes the result. An older copy of this tool skill is
replaced by `ath skill install`, so an agent does not keep reading old
instructions. A calculation skill that is already installed is not replaced.
Point `--record` at one record, then run
it again on another record to see which fields were read. A day total uses the
record you selected.

Predict and grade:

| Command | Purpose |
|---|---|
| `ath predict <benchmark> --json` | Return evidence for a harness |
| `ath predict <benchmark>` | Optional terminal model call that may save after confirmation |
| `ath grade <prediction-id> --result <record-id>` | Grade one existing prediction against one existing result |
| `ath backtest` | Replay history and write a report, without changing athlete data |
| `ath share` | Not built; names the latest backtest report |

`ath grade` never logs a result. A result is entered only through `ath log`. Explicit
grading creates or reuses the exact prediction-result link and writes a deterministic
grade. If the result record was corrected, grading revises the existing link so
older grades keep the earlier exact link revision.

## Import and export

Provider import always starts with a preview. Review new, matching, changed, skipped,
and conflicting records. A package preview instead reports unrepresentable and
unknown records. Run again with `--yes` only when the preview is acceptable. A
conflict is never resolved by guessing.

Provider rows use route-specific import keys. Repeating unchanged data is a match.
Changed content with the same key creates a revision.

A JSON file from Open Wearables or Junction is read with `--mapping open-wearables`
or `--mapping junction`. A mapping file lists their field and our field. A mapped
field becomes the typed value. Every other field stays in `supplied_fields` under
the name it arrived with. The result lists each object that was left out and why.
A person can add a mapping file in `.agents/mappings/`. A mapping that would turn a
personal report into a measured record, or rename a generic HRV field to RMSSD or
SDNN, is refused before anything is written.

For a v1 package conflict, run `ath conflicts show`. Record one explicit choice for
each record with `ath conflicts resolve`: `local`, `imported`, or `corrected`.
These commands change only a temporary conflict file. Rerun the same package import
with `--yes` after every conflict is resolved. A changed package or database makes
the choices stale.

`ath export` writes:

```text
athlete.ath.json
series/*.ath-series.json
attachments/*
export-manifest.json
```

The package contains current records, retained revisions, normal deletions, erasure
markers, provider import keys, generated sample files, and permitted attachments.
The manifest contains hashes, byte counts, record counts, the source database ID,
and the database revision.

Export time is absent by default. The same database revision and options produce the
same bytes. The package does not stay synchronized with SQLite.

## Delete and erase

`ath delete` is reversible through retained history. It writes a deletion record and
hides the current record.

`ath erase` is for privacy removal. It removes values, revisions, samples, dependent
records, and attachment files. It may leave only value-free erasure markers. It
cannot remove copies already present in exports, backups, snapshots, journals, or
storage-device history.

The erasure preview records the database revision and complete dependency set. The
write is refused if either changes. Interactive use asks for confirmation and then
requires the exact record ID. Non-interactive use needs both `--yes` and
`--confirm-erasure <record-id>`. `--json` never erases.

## What still needs judgment

- No recorded workout means no recording. It does not prove a rest day.
- Association does not prove cause.
- Cite dates, values, source IDs, record IDs, and revisions.
- State missing coverage and conflicting sources.
- A hand-entered value is not a device reading.
- Provider scores may support a claim, but they are source-reported composites.
- RMSSD and SDNN are different statistics.

## Open these when needed

- [references/cli.md](references/cli.md) — generated command options and examples.
- [references/format.md](references/format.md) — record envelopes, record types, and
  package layout.
- [references/logging.md](references/logging.md) — v1 structured logging and
  benchmark naming.
- [references/predicting.md](references/predicting.md) — prediction evidence and the
  v1 prediction record.
- [references/grading.md](references/grading.md) — explicit grading and prediction
  error analysis.
