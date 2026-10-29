# ath command reference

Generated from the CLI's own help by `pnpm generate:skill-cli`. Do not edit by hand.
This is the text `ath <command> --help` prints, for every command, so an agent
can read the whole surface without a round trip.

Every command takes `--json` where output is meant for an agent. Commands that
write show a summary and ask once; `-y` answers yes in advance. Where `--dry-run`
is offered, it shows the write and writes nothing.

## Contents

- [ath](#ath)
- [ath init](#ath-init) — create a new athlete.ath.sqlite here, with the well-known benchmarks defined
- [ath skill](#ath-skill) — where the agent skill is installed, and whether each copy is current
- [ath skill install](#ath-skill-install) — copy the skill into .agents/skills and every harness folder here
- [ath migrate](#ath-migrate) — preview and apply a SQL schema migration after a verified backup
- [ath repair](#ath-repair) — repair typed-table JSON bytes left by npm 1.0.0 provider imports
- [ath import](#ath-import) — preview and import a package or an Apple Health, WHOOP, Oura, Garmin, or Strava export
- [ath conflicts](#ath-conflicts) — review and resolve the temporary conflicts from a package import
- [ath conflicts show](#ath-conflicts-show) — show complete local and imported values from the current conflict preview
- [ath conflicts resolve](#ath-conflicts-resolve) — choose the local, imported, or a corrected value for one conflict
- [ath export](#ath-export) — write a complete deterministic Athletic Standard JSON package
- [ath log](#ath-log) — write one plain-text record or one validated structured batch
- [ath link](#ath-link) — attach a benchmark result to the workout session it happened in
- [ath correct](#ath-correct) — replace one record's complete data and retain its prior revision
- [ath delete](#ath-delete) — hide one current record while retaining its complete history
- [ath erase](#ath-erase) — permanently remove one record and every record that depends on it
- [ath check](#ath-check) — make sure the file still obeys every rule of the format
- [ath key](#ath-key) — save or show a gateway key — never stored in the athlete file
- [ath key set](#ath-key-set) — save a Vercel or OpenRouter key in the OS password store
- [ath key clear](#ath-key-clear) — delete the saved gateway key from the password store
- [ath models](#ath-models) — list live text models that can reason, or save your usual one
- [ath predict](#ath-predict) — predict a benchmark — needs a model in a bare terminal; --json is evidence
- [ath backtest](#ath-backtest) — replay history with one or more models — writes a report, not the athlete file
- [ath share](#ath-share) — not built yet — names the latest backtest report
- [ath grade](#ath-grade) — grade an existing prediction against an existing benchmark result
- [ath apply](#ath-apply) — run one calculation skill file on one record
- [ath pull](#ath-pull) — read one day from Garmin's personal connection
- [ath calculate](#ath-calculate) — run a named calculation without writing athlete data
- [ath series](#ath-series) — read a sample series back: one row per day, or the raw samples
- [ath stats](#ath-stats) — what is in the file: counts, date ranges, sources, and baselines
- [ath records](#ath-records) — find stable record IDs, inspect one record, and read its revision history
- [ath records list](#ath-records-list) — list current records in stable order
- [ath records show](#ath-records-show) — show one record, one revision, its history, or its links

## ath

```
Usage: ath [options] [command]

Athletic Standard — an open, local-first format for training and recovery data
that keeps measured signals apart from self-reported ones.

Options:
  -V, --version                               output the version number
  --update                                    install the latest ath with the tool that installed it, replace an older tool skill, and add skill files that are not already here
  -h, --help                                  display help for command

Set up:
  init [options]                              create a new athlete.ath.sqlite here, with the well-known benchmarks defined
  skill [options]                             where the agent skill is installed, and whether each copy is current
  migrate [options] [file]                    preview and apply a SQL schema migration after a verified backup
  repair [options] [file]                     repair typed-table JSON bytes left by npm 1.0.0 provider imports
  key [options]                               save or show a gateway key — never stored in the athlete file
  models [options]                            list live text models that can reason, or save your usual one

Get your data in:
  import [options] <path>                     preview and import a package or an Apple Health, WHOOP, Oura, Garmin, or Strava export
  conflicts                                   review and resolve the temporary conflicts from a package import
  export [options] <directory>                write a complete deterministic Athletic Standard JSON package
  log [options] [entry...]                    write one plain-text record or one validated structured batch
  link [options] <result-id> <session-id>     attach a benchmark result to the workout session it happened in
  correct [options] <record-id>               replace one record's complete data and retain its prior revision
  delete [options] <record-id>                hide one current record while retaining its complete history
  erase [options] <record-id>                 permanently remove one record and every record that depends on it
  pull [options] <source>                     read one day from Garmin's personal connection

Read your data:
  check [options] [file]                      make sure the file still obeys every rule of the format
  apply [options] <calculation>               run one calculation skill file on one record
  calculate [options] <operation> <quantity>  run a named calculation without writing athlete data
  series [options] <quantity>                 read a sample series back: one row per day, or the raw samples
  stats [options] [file]                      what is in the file: counts, date ranges, sources, and baselines
  records                                     find stable record IDs, inspect one record, and read its revision history

Predict and grade:
  predict [options] <benchmark>               predict a benchmark — needs a model in a bare terminal; --json is evidence
  backtest [options]                          replay history with one or more models — writes a report, not the athlete file
  share [options]                             not built yet — names the latest backtest report
  grade [options] <prediction-id>             grade an existing prediction against an existing benchmark result
```

## ath init

```
Usage: ath init [options]

create a new athlete.ath.sqlite here, with the well-known benchmarks defined

Options:
  --name <name>        your name, so the file says whose it is
  --birth-year <year>  birth year, which some readings are read against
  --sex <sex>          male | female, for the same reason
  --units <units>      metric | imperial — display only; stored values are
                       always metric (default: "metric")
  --timezone <name>    default IANA timezone for calendar views, such as
                       America/Chicago
  --file <path>        write somewhere other than the default name (default:
                       "athlete.ath.sqlite")
  -y, --yes            skip the questions and use the flags and defaults
  --empty              create no athlete records, for restoring a v1 package
  --no-skill           do not install the agent skill, even if an agent folder
                       is here
  --json               structured output, for an agent rather than a person
  -h, --help           display help for command


Examples:

  Answer three questions, then write the file:
    $ ath init

  Skip the questions:
    $ ath init -y --name "Sam Reyes" --birth-year 1991 --sex female

The file is created here, called athlete.ath.sqlite, with the well-known
benchmarks already defined. Nothing leaves your machine.

For a complete v1 package restore, create an empty destination:
    $ ath init -y --empty
    $ ath import ./saved-package
    $ ath import ./saved-package --yes

The agent skill is written to .agents/skills, which Claude Code, Cursor,
Codex, and Gemini CLI read, and into any of .claude, .cursor, .codex,
.gemini, or .github already here. An agent in this folder then knows how
to read and write the file. Pass --no-skill to skip that.
```

## ath skill

```
Usage: ath skill [options] [command]

where the agent skill is installed, and whether each copy is current

Options:
  --json             structured output, for an agent rather than a person
  -h, --help         display help for command

Commands:
  install [options]  copy the skill into .agents/skills and every harness folder
                     here


Examples:

  Where the skill is installed:
    $ ath skill

  Upgrade ath. An older tool skill is replaced. A calculation skill stays:
    $ ath --update

  Add skill files that are not already here, and replace an older tool skill:
    $ ath skill install

The skill is a folder an agent reads to learn the file and the commands.
ath init writes it to .agents/skills and into any harness folder already
here. An older copy of the tool skill is replaced, so the agent does not
keep reading the old instructions. A calculation skill that is already
there is not replaced.
```

## ath skill install

```
Usage: ath skill install [options]

copy the skill into .agents/skills and every harness folder here

Options:
  --json      structured output, for an agent rather than a person
  -h, --help  display help for command
```

## ath migrate

```
Usage: ath migrate [options] [file]

preview and apply a SQL schema migration after a verified backup

Arguments:
  file                               the database to migrate (default: the one
                                     in this folder)

Options:
  --file <path>                      migrate a database other than the one in
                                     this folder
  --confirm-migration <database-id>  repeat the database ID exactly for
                                     non-interactive use
  -y, --yes                          apply the preview after reviewing it
  --dry-run                          show the preview and write nothing
  --json                             structured preview; never confirms the
                                     migration
  -h, --help                         display help for command


Examples:

  Preview the SQL schema migration plan:
    $ ath migrate

  Apply after reviewing the preview:
    $ ath migrate -y --confirm-migration <database-id>

  Return the preview as JSON for an agent:
    $ ath migrate --json

Migration never runs silently. A verified SQLite backup is created before the
first schema write. Non-interactive commands refuse an outdated database and
point here.

Requires the `sqlite3` command-line tool on PATH for verified backups (same as
`ath repair` when repair candidates exist).
```

## ath repair

```
Usage: ath repair [options] [file]

repair typed-table JSON bytes left by npm 1.0.0 provider imports

Arguments:
  file                                    the database to repair (default: the one in this folder)

Options:
  --file <path>                           repair a database other than the one in this folder
  -y, --yes                               apply the repair after reviewing the preview
  --from-revision                         copy typed-table JSON from the current revision when bytes disagree in meaning
  --confirm-revision-align <database-id>  repeat the database ID exactly when using --from-revision
  --dry-run                               show how many records would change and write nothing
  --json                                  structured preview only
  -h, --help                              display help for command


Examples:

  Preview provider-encoding repair candidates:
    $ ath repair

  Apply key-order-only repair after npm 1.0.0 provider imports:
    $ ath repair -y

  Rebuild typed tables from the current revision when direct SQL changed values:
    $ ath repair -y --from-revision --confirm-revision-align <database-id>

Each confirmed repair creates a new verified SQLite backup. Failed attempts leave
that backup on disk for restore; retry creates another backup.
```

## ath import

```
Usage: ath import [options] <path>

preview and import a package or an Apple Health, WHOOP, Oura, Garmin, or Strava
export

Arguments:
  path           v1 package or supported provider export

Options:
  --file <path>  import into a database other than the one in this folder
  -y, --yes      apply a conflict-free preview
  --json         structured preview; never confirms the write
  -h, --help     display help for command


Examples:

  Preview a complete v1 package:
    $ ath import ~/Backups/athletic-standard-export

  Apply it only after the preview has no conflicts:
    $ ath import ~/Backups/athletic-standard-export --yes

  Preview a provider export:
    $ ath import ~/Downloads/apple_health_export.zip
    $ ath import ~/Downloads/whoop-export
    $ ath import ~/Downloads/oura-export

The default is always a preview. `--json` is also only a preview and
never confirms a write. Provider rows keep separate Apple Health, WHOOP,
and Oura source routes. Repeating an unchanged export reports matching
records. A corrected provider row creates a retained revision.

For a v1 package, an empty database adopts the package database ID and
exact revision history. A database with the same ID can accept new
records and later revisions. Import never deletes a local record merely
because a package omits it. A different database ID is refused because
it may belong to another athlete. Unknown fields and records that v1 cannot
represent are listed in the preview and block the write. No flag discards them.
Athletic Standard 0.x JSON files are
prerelease formats and are not imported by v1.
```

## ath conflicts

```
Usage: ath conflicts [options] [command]

review and resolve the temporary conflicts from a package import

Options:
  -h, --help                     display help for command

Commands:
  show [options] [record-id]     show complete local and imported values from
                                 the current conflict preview
  resolve [options] <record-id>  choose the local, imported, or a corrected
                                 value for one conflict
  help [command]                 display help for command


Examples:

  Show complete local and imported values for every current conflict:
    $ ath conflicts show

  Keep the local record:
    $ ath conflicts resolve <record-id> --choice local

  Accept the imported record as a new local revision:
    $ ath conflicts resolve <record-id> --choice imported

  Supply a complete corrected data object as a new local revision:
    $ ath conflicts resolve <record-id> --choice corrected \
        --data '{"...":"..."}'

These commands change only a temporary conflict file. They do not change athlete
data. After every conflict has a choice, rerun the exact package import with
--yes. A changed package or database makes every saved choice stale.
```

## ath conflicts show

```
Usage: ath conflicts show [options] [record-id]

show complete local and imported values from the current conflict preview

Arguments:
  record-id      show one conflicting stable record ID

Options:
  --file <path>  read conflicts for a database other than the one in this folder
  --json         structured output
  -h, --help     display help for command
```

## ath conflicts resolve

```
Usage: ath conflicts resolve [options] <record-id>

choose the local, imported, or a corrected value for one conflict

Arguments:
  record-id          conflicting stable record ID

Options:
  --choice <choice>  local, imported, or corrected
  --data <json>      complete corrected data object; required with corrected
  --file <path>      resolve conflicts for a database other than the one in this
                     folder
  --json             structured output
  -h, --help         display help for command
```

## ath export

```
Usage: ath export [options] <directory>

write a complete deterministic Athletic Standard JSON package

Arguments:
  directory                  new directory to create for the package

Options:
  --exported-at <timestamp>  include this exact UTC export time in the manifest
  --file <path>              export a database other than the one in this folder
  --json                     structured command result
  -h, --help                 display help for command


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
manifest time is needed.
```

## ath log

```
Usage: ath log [options] [entry...]

write one plain-text record or one validated structured batch

Arguments:
  entry                the entry, unquoted. Leave it off to paste one, ending
                       with Ctrl-D

Options:
  --benchmark <name>   name the workout yourself, instead of naming it after the
                       day
  --at <timestamp>     exact UTC event time with milliseconds; defaults to now
  --agent <name>       name the agent making a structured write
  --attachment <path>  attach a PNG, JPEG, WebP, or PDF; pass more than once for
                       several files (default: [])
  --file <path>        write to a file other than the one in this folder
  -y, --yes            write it without asking — for scripts, and when you are
                       sure
  --dry-run            show what would be written, and write nothing
  --json               structured preview for an agent; never confirms the write
  -h, --help           display help for command


Examples:

  A note, taken exactly as typed. No quotes needed:
    $ ath log slept badly, about 5 hours

  A manual measurement. Name RMSSD or SDNN rather than saying only HRV:
    $ ath log RMSSD 61 ms

  How something felt, with a rating:
    $ ath log sore quads 4/5

  A benchmark result:
    $ ath log Fran in 4:41 rx

  A photo attached to one athlete report:
    $ ath log Meal photo --attachment ./receipt.png

  A structured record from an agent. Run `ath log`, paste one record
  object or an array, and press Ctrl-D:
    $ ath log
    {"record_type":"goal","data":{"authorship":{...},"title":"Fran under 4:30"}}
    ^D

Plain text creates one record. A structured JSON array creates one
validated batch. The preview shows every stable ID and complete record
before one transaction writes anything. `--json` returns that preview
and never confirms a write. Pass `--yes` only after approval.

--attachment accepts PNG, JPEG, WebP, or PDF files up to 25 MiB. Pass it
more than once for several files. Exactly one athlete report or nutrition
entry in the batch owns them. No file is copied during a JSON preview.
```

## ath link

```
Usage: ath link [options] <result-id> <session-id>

attach a benchmark result to the workout session it happened in

Arguments:
  result-id            stable benchmark-result record ID
  session-id           stable workout-session record ID

Options:
  --revision <number>  current result revision from the preview
  --file <path>        change a database other than the one in this folder
  -y, --yes            write without asking after reviewing the preview
  --dry-run            show what would change and write nothing
  --json               structured preview; never confirms the write
  -h, --help           display help for command


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
correction made after the preview.
```

## ath correct

```
Usage: ath correct [options] <record-id>

replace one record's complete data and retain its prior revision

Arguments:
  record-id            stable record ID

Options:
  --revision <number>  current revision from the preview
  --data <json>        complete replacement for the record's data object
  --agent <name>       name the agent making the correction
  --attachment <path>  attach a PNG, JPEG, WebP, or PDF; pass more than once for
                       several files (default: [])
  --file <path>        change a database other than the one in this folder
  -y, --yes            write without asking after reviewing the preview
  --dry-run            show what would change and write nothing
  --json               structured preview; never confirms the write
  -h, --help           display help for command


Examples:

  Read the current revision and complete data:
    $ ath records show <record-id> --history --json

  Preview a complete replacement:
    $ ath correct <record-id> --revision 2 --data '{"...":"..."}' --json

  Write after review:
    $ ath correct <record-id> --revision 2 --data '{"...":"..."}' --yes

Correction keeps the stable ID and complete earlier revisions. The
expected revision prevents a stale preview from overwriting newer data.
Use --attachment while correcting an athlete report or nutrition entry to
create and link a supported file in the same database revision.
```

## ath delete

```
Usage: ath delete [options] <record-id>

hide one current record while retaining its complete history

Arguments:
  record-id            stable record ID

Options:
  --revision <number>  current revision from the preview
  --file <path>        change a database other than the one in this folder
  -y, --yes            write without asking after reviewing the preview
  --dry-run            show what would change and write nothing
  --json               structured preview; never confirms the write
  -h, --help           display help for command


Examples:

  Preview a normal deletion:
    $ ath delete <record-id> --revision 2 --json

  Hide it from current views after review:
    $ ath delete <record-id> --revision 2 --yes

Normal deletion retains the complete record and revision history. Privacy
erasure is a different command because it removes values and cannot be
undone from the database.
```

## ath erase

```
Usage: ath erase [options] <record-id>

permanently remove one record and every record that depends on it

Arguments:
  record-id                      stable record ID to erase

Options:
  --revision <number>            current revision from the preview
  --reason <code>                one of: athlete_request, legal_requirement,
                                 recorded_in_error, other (default:
                                 "athlete_request")
  --confirm-erasure <record-id>  second confirmation: repeat the record ID
                                 exactly
  --file <path>                  change a database other than the one in this
                                 folder
  -y, --yes                      accept the first confirmation after reviewing
                                 the preview
  --dry-run                      show everything that would be erased and write
                                 nothing
  --json                         structured preview; never confirms the erasure
  -h, --help                     display help for command


Examples:

  Preview the record, every dependent record, and each attachment:
    $ ath erase <record-id> --revision 2 --json

  Permanently erase after reviewing that preview:
    $ ath erase <record-id> --revision 2 --yes \
        --confirm-erasure <record-id>

Erasure removes the current record, its revision history, records that
depend on it, and their attachment files. It leaves only value-free
markers saying which stable IDs were erased.

This cannot be undone from the database. Scripts need both --yes and an
exact --confirm-erasure value. Interactive use asks the same two
questions. `--json` and `--dry-run` never erase anything.
```

## ath check

```
Usage: ath check [options] [file]

make sure the file still obeys every rule of the format

Arguments:
  file               the file to check (default: the one in this folder)

Options:
  --file <path>      check a database other than the one in this folder
  --validate-export  also build and validate a temporary complete JSON package
  --json             structured output, for an agent rather than a person
  -h, --help         display help for command


Examples:

  Check the file in this folder:
    $ ath check

  Check one somewhere else:
    $ ath check ~/training/athlete.ath.sqlite

An error means the database breaks a rule of the format and says which rule.
A warning means something is worth knowing but the database is still valid.
The check covers SQLite integrity, schema versions, foreign keys, domain rules,
record relationships, revision chains, session ordering, and attachments.

Also build and validate a temporary complete JSON package:
    $ ath check --validate-export
```

## ath key

```
Usage: ath key [options] [command]

save or show a gateway key — never stored in the athlete file

Options:
  --json           structured output, for an agent rather than a person
  -h, --help       display help for command

Commands:
  set <gateway>    save a Vercel or OpenRouter key in the OS password store
  clear [gateway]  delete the saved gateway key from the password store


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
still works, for scripts.
```

## ath key set

```
Usage: ath key set [options] <gateway>

save a Vercel or OpenRouter key in the OS password store

Arguments:
  gateway     vercel or openrouter

Options:
  -h, --help  display help for command
```

## ath key clear

```
Usage: ath key clear [options] [gateway]

delete the saved gateway key from the password store

Arguments:
  gateway     vercel or openrouter; leave off to clear both

Options:
  -h, --help  display help for command
```

## ath models

```
Usage: ath models [options]

list live text models that can reason, or save your usual one

Options:
  --default <name>  save this model in athlete.ath.model next to the database,
                    for the next predict
  --gateway <name>  vercel or openrouter, when both keys are available
  --file <path>     the athlete file whose default to save
  --json            structured output, for an agent rather than a person
  -h, --help        display help for command


Examples:

  Every live text model that can reason, including open weights:
    $ ath models

  Save your usual model in athlete.ath.model next to athlete.ath.sqlite:
    $ ath models --default openai/gpt-oss-120b

Needs a gateway key. The list is fetched each time, so a new open-weight
model appears without a new ath. Image, audio, and embedding models are
out. We do not pick a favourite lab for you.
```

## ath predict

```
Usage: ath predict [options] <benchmark>

predict a benchmark — needs a model in a bare terminal; --json is evidence

Arguments:
  benchmark          the benchmark to predict, e.g. `fran`. See them all with
                     `ath stats`

Options:
  --model <name>     the model for this run. See them with `ath models`
  --gateway <name>   vercel or openrouter, when both keys are available
  --as-of <time>     historical day (YYYY-MM-DD) or exact UTC timestamp
  --timezone <name>  IANA timezone when --as-of is a calendar day
  --file <path>      read a file other than the one in this folder
  -y, --yes          write the prediction without asking
  --dry-run          show the prediction, and write nothing
  --json             evidence for a harness, not a prediction
  -h, --help         display help for command


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
`ath log`. In a bare terminal this command calls the model you chose,
prints the number, and asks once before writing.

--as-of hides everything after that day and does not write. Replaying
the past is `ath backtest`.
```

## ath backtest

```
Usage: ath backtest [options]

replay history with one or more models — writes a report, not the athlete file

Options:
  -m, --model <name>  a model to run; pass more than once to compare (default:
                      [])
  --all               run every live text model that can reason, after showing
                      the count
  --gateway <name>    vercel or openrouter, when both keys are available
  --file <path>       read a file other than the one in this folder
  -y, --yes           do not ask before a long --all run
  --json              structured output, for an agent rather than a person
  -h, --help          display help for command


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
written. The report's summary is the later share payload.
```

## ath share

```
Usage: ath share [options]

not built yet — names the latest backtest report

Options:
  --file <path>  look next to a file other than the one in this folder
  --json         structured output, for an agent rather than a person
  -h, --help     display help for command


Examples:

  Name the latest backtest report:
    $ ath share

Not built yet. The shareable part of a backtest is the summary in the
report file this names.
```

## ath grade

```
Usage: ath grade [options] <prediction-id>

grade an existing prediction against an existing benchmark result

Arguments:
  prediction-id                    stable prediction record ID

Options:
  --result <record-id>             stable benchmark-result record ID
  --calculation-version <version>  grade calculation version (default:
                                   "prediction-grade-v1")
  --file <path>                    change a database other than the one in this
                                   folder
  -y, --yes                        write without asking after reviewing the
                                   preview
  --dry-run                        show what would be written, and write nothing
  --json                           structured preview; never confirms the write
  -h, --help                       display help for command


Examples:

  Find the prediction and result IDs:
    $ ath records list --type prediction
    $ ath records list --type benchmark_result

  Preview the exact link and grade:
    $ ath grade <prediction-id> --result <result-id> --json

  Write after review:
    $ ath grade <prediction-id> --result <result-id> --yes

`ath grade` never logs a result. It grades two records that already
exist. It creates a stable prediction-result link when needed and saves
the deterministic arithmetic against exact revisions.

A result inside the prediction's stated range is a hit. Anything else is
a miss. A prediction error analysis is a separate authored record written
through structured `ath log`, not through a grade flag.
```

## ath apply

```
Usage: ath apply [options] <calculation>

run one calculation skill file on one record

Arguments:
  calculation    calculation file name, such as heart-rate-zones

Options:
  --record <id>  the one record to use
  --file <path>  read a database other than the one in this folder
  --json         structured result
  -h, --help     display help for command


Examples:

  Run the shipped heart-rate zones calculation on one workout:
    $ ath apply heart-rate-zones --record 018f6b2a-6c3a-7c2e-8f1a-6a0d9e2b7c31

  Run the same calculation on a different device's record:
    $ ath apply heart-rate-zones --record 018f6b2a-6c3a-7c2e-8f1a-6a0d9e2b7c32

The calculation file lives in the skill folder. Edit that file to change
the zones. The result names the file, the record, and the fields it read.
```

## ath pull

```
Usage: ath pull [options] <source>

read one day from Garmin's personal connection

Arguments:
  source            garmin

Options:
  --date <day>      YYYY-MM-DD
  --base-url <url>  personal API root, for a test server
  --file <path>     write into a database other than the one in this folder
  --json            structured result
  -h, --help        display help for command


Examples:

  Read one day from Garmin after the key is saved:
    $ ath key set garmin
    $ ath pull garmin --date 2026-01-02

The key stays in the password store. It is not written into the athlete file.
```

## ath calculate

```
Usage: ath calculate [options] <operation> <quantity>

run a named calculation without writing athlete data

Arguments:
  operation                    one of: summary, baseline, trend, coverage,
                               compare-sources, correlation, workout-drift,
                               provider-period-summary
  quantity                     measurement quantity, provider metric, or summary
                               type

Options:
  --record-kind <kind>         one of: measurement, provider-score,
                               provider-summary, session-summary (default:
                               "measurement")
  --period-type <type>         provider period type; required for
                               provider-period-summary
  --source <id>                use one source only
  --compare-source <id>        second source for compare-sources or correlation
  --other-quantity <quantity>  second quantity for correlation
  --session <record-id>        workout session for workout-drift
  --from <time>                earliest day (YYYY-MM-DD) or exact UTC timestamp
  --to <time>                  latest day (YYYY-MM-DD) or exact UTC timestamp
  --timezone <name>            IANA timezone for calendar operations
  --group <period>             none, day, week, or month (default: "none")
  --file <path>                read a database other than the one in this folder
  --json                       structured calculation receipt
  -h, --help                   display help for command


Examples:

  A 90-day baseline, kept separate for each source:
    $ ath calculate baseline hrv_rmssd --from 2026-07-01 --to 2026-09-28

  Daily totals in an explicit calendar timezone:
    $ ath calculate summary steps --group day --from 2026-09-01 \
        --to 2026-09-30 --timezone America/Chicago

  Compare two sources without pooling them:
    $ ath calculate compare-sources resting_heart_rate \
        --source <source-id> --compare-source <other-source-id>

  Correlate same-day means:
    $ ath calculate correlation hrv_rmssd --other-quantity resting_heart_rate \
        --source <source-id> --timezone America/Chicago

  Baseline WHOOP recovery scores that have an absolute source time:
    $ ath calculate baseline recovery --record-kind provider-score

  Summarize Oura readiness by Oura's own assigned day:
    $ ath calculate provider-period-summary readiness \
        --record-kind provider-score --period-type oura_day_unspecified

Named operations are summary, baseline, trend, coverage, compare-sources,
correlation, workout-drift, and provider-period-summary. The provider-period
operation requires --period-type and never turns that period into a calendar day.

Record kinds are measurement, provider-score, provider-summary, and
session-summary. Measurement is the default. Naming the kind keeps measurements,
scores, and source-reported totals separate even when their names are similar.

The result names its method, exact record revisions, exclusions, sources, date
range, and timezone or provider period when one was used.

Manual sources never enter these device calculations. Different sources,
RMSSD and SDNN, temperature types, and distance types are never pooled.
```

## ath series

```
Usage: ath series [options] <quantity>

read a sample series back: one row per day, or the raw samples

Arguments:
  quantity           one of: heart_rate, hrv_beats, ecg_beats, hrv_sdnn,
                     respiratory_rate, oxygen_saturation, steps, active_energy,
                     distance_walking_running, distance_cycling,
                     distance_swimming, running_speed, running_power,
                     running_stride_length, running_vertical_oscillation,
                     running_ground_contact_time, physical_effort, basal_energy,
                     exercise_time, flights_climbed, walking_speed,
                     walking_step_length, walking_asymmetry_percentage,
                     time_in_daylight

Options:
  --from <time>      earliest day (YYYY-MM-DD) or exact UTC timestamp
  --to <time>        latest day (YYYY-MM-DD) or exact UTC timestamp
  --source <id>      one source only, for when two measured the same thing —
                     `ath stats` lists the ids
  --timezone <name>  IANA timezone for calendar days; uses the athlete profile
                     when omitted
  --limit <count>    maximum samples to read (default: 10000)
  --raw              every sample, rather than one row per day
  --json             structured output, for an agent rather than a person
  --file <path>      read a file other than the one in this folder
  -h, --help         display help for command


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
provider summaries that may describe the same workout or night. `ath stats`
lists which series exist.
```

## ath stats

```
Usage: ath stats [options] [file]

what is in the file: counts, date ranges, sources, and baselines

Arguments:
  file           the file to summarize (default: the one in this folder)

Options:
  --file <path>  summarize a database other than the one in this folder
  --json         structured output, for an agent rather than a person
  -h, --help     display help for command


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
model has actually been right about you.
```

## ath records

```
Usage: ath records [options] [command]

find stable record IDs, inspect one record, and read its revision history

Options:
  -h, --help                  display help for command

Commands:
  list [options]              list current records in stable order
  show [options] <record-id>  show one record, one revision, its history, or its
                              links
  help [command]              display help for command


Examples:

  Find stable IDs for all current records:
    $ ath records list

  List only predictions:
    $ ath records list --type prediction

  Show one record and its complete revision timeline:
    $ ath records show 01890f3e-e7d3-7cc0-88c8-000000000010 --history

  Show the records it links to and the records that link to it:
    $ ath records show 01890f3e-e7d3-7cc0-88c8-000000000010 --links

`ath records list` is for finding stable IDs. `ath records show` reads one
logical record, a named revision, its history, and its links. Use `ath stats`
for the fixed overview and `ath series` for dense samples.
```

## ath records list

```
Usage: ath records list [options]

list current records in stable order

Options:
  --type <record-type>  show one record type
  --include-deleted     include records hidden by a normal deletion
  --limit <count>       maximum records to return (default: 100)
  --file <path>         read a database other than the one in this folder
  --json                structured output, for an agent rather than a person
  -h, --help            display help for command
```

## ath records show

```
Usage: ath records show [options] <record-id>

show one record, one revision, its history, or its links

Arguments:
  record-id            stable UUIDv7 record ID

Options:
  --revision <number>  show this retained revision
  --history            include the revision timeline
  --links              include incoming and outgoing record links
  --file <path>        read a database other than the one in this folder
  --json               structured output, for an agent rather than a person
  -h, --help           display help for command
```
