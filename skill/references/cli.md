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
- [ath import](#ath-import) — preview and import a v1 package or Apple Health, WHOOP, or Oura export
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
  --update                                    install the latest ath with the tool that installed it, and replace the agent skill if one is already in this folder
  -h, --help                                  display help for command

Set up:
  init [options]                              create a new athlete.ath.sqlite here, with the well-known benchmarks defined
  skill [options]                             where the agent skill is installed, and whether each copy is current
  migrate [options] [file]                    preview and apply a SQL schema migration after a verified backup
  key [options]                               save or show a gateway key — never stored in the athlete file
  models [options]                            list live text models that can reason, or save your usual one

Get your data in:
  import [options] <path>                     preview and import a v1 package or Apple Health, WHOOP, or Oura export
  conflicts                                   review and resolve the temporary conflicts from a package import
  export [options] <directory>                write a complete deterministic Athletic Standard JSON package
  log [options] [entry...]                    write one plain-text record or one validated structured batch
  link [options] <result-id> <session-id>     attach a benchmark result to the workout session it happened in
  correct [options] <record-id>               replace one record's complete data and retain its prior revision
  delete [options] <record-id>                hide one current record while retaining its complete history
  erase [options] <record-id>                 permanently remove one record and every record that depends on it

Read your data:
  check [options] [file]                      make sure the file still obeys every rule of the format
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
```

## ath import

```
Usage: ath import [options] <path>

preview and import a v1 package or Apple Health, WHOOP, or Oura export

Arguments:
  path           v1 package or supported provider export

Options:
  --file <path>  import into a database other than the one in this folder
  -y, --yes      apply a conflict-free preview
  --json         structured preview; never confirms the write
  -h, --help     display help for command
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
```

## ath share

```
Usage: ath share [options]

not built yet — names the latest backtest report

Options:
  --file <path>  look next to a file other than the one in this folder
  --json         structured output, for an agent rather than a person
  -h, --help     display help for command
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
