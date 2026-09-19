# ath command reference

Generated from the CLI's own help by `pnpm generate:skill-cli`. Do not edit by hand.
This is the text `ath <command> --help` prints, for every command, so an agent
can read the whole surface without a round trip.

Every command takes `--json` where output is meant for an agent. Commands that
write show a summary and ask once; `-y` answers yes in advance and `--dry-run`
writes nothing.

## Contents

- [ath](#ath)
- [ath init](#ath-init) — create a new athlete.ath.json here, with the well-known benchmarks defined
- [ath import](#ath-import) — load an Apple Health, WHOOP, or Oura export into the file
- [ath log](#ath-log) — write something down: a workout result, a measurement, or how you felt
- [ath link](#ath-link) — attach a workout result to the device session it happened in
- [ath check](#ath-check) — make sure the file still obeys every rule of the format
- [ath key](#ath-key) — save or show a gateway key — never stored in the athlete file
- [ath key set](#ath-key-set) — save a Vercel or OpenRouter key in the OS password store
- [ath key clear](#ath-key-clear) — delete the saved gateway key from the password store
- [ath models](#ath-models) — list live text models that can reason, or save your usual one
- [ath predict](#ath-predict) — predict a benchmark — needs a model in a bare terminal; --json is evidence
- [ath backtest](#ath-backtest) — replay history with one or more models — writes a report, not the athlete file
- [ath share](#ath-share) — not built yet — names the latest backtest report
- [ath grade](#ath-grade) — record what actually happened, and score the prediction against it
- [ath series](#ath-series) — read a sample series back: one row per day, or the raw samples
- [ath stats](#ath-stats) — what is in the file: counts, date ranges, sources, and baselines

## ath

```
Usage: ath [options] [command]

Athletic Standard — an open, local-first format for training and recovery data
that keeps measured signals apart from self-reported ones.

Options:
  -V, --version                      output the version number
  -h, --help                         display help for command

Set up:
  init [options]                     create a new athlete.ath.json here, with
                                     the well-known benchmarks defined
  key [options]                      save or show a gateway key — never stored
                                     in the athlete file
  models [options]                   list live text models that can reason, or
                                     save your usual one

Get your data in:
  import [options] <path>            load an Apple Health, WHOOP, or Oura export
                                     into the file
  log [options] [entry...]           write something down: a workout result, a
                                     measurement, or how you felt
  link [options] <result> <session>  attach a workout result to the device
                                     session it happened in

Read your data:
  check [options] [file]             make sure the file still obeys every rule
                                     of the format
  series [options] <quantity>        read a sample series back: one row per day,
                                     or the raw samples
  stats [options] [file]             what is in the file: counts, date ranges,
                                     sources, and baselines

Predict and grade:
  predict [options] <benchmark>      predict a benchmark — needs a model in a
                                     bare terminal; --json is evidence
  backtest [options]                 replay history with one or more models —
                                     writes a report, not the athlete file
  share [options]                    not built yet — names the latest backtest
                                     report
  grade [options] <benchmark>        record what actually happened, and score
                                     the prediction against it
```

## ath init

```
Usage: ath init [options]

create a new athlete.ath.json here, with the well-known benchmarks defined

Options:
  --name <name>        your name, so the file says whose it is
  --birth-year <year>  birth year, which some readings are read against
  --sex <sex>          male | female, for the same reason
  --units <units>      metric | imperial — display only; stored values are
                       always metric (default: "metric")
  --file <path>        write somewhere other than the default name (default:
                       "athlete.ath.json")
  -y, --yes            skip the questions and use the flags and defaults
  --no-skill           do not install the agent skill, even if an agent folder
                       is here
  --json               structured output, for an agent rather than a person
  -h, --help           display help for command
```

## ath import

```
Usage: ath import [options] <path>

load an Apple Health, WHOOP, or Oura export into the file

Arguments:
  path           the export: a zip, a folder, an export.xml, or a CSV

Options:
  --file <path>  import into a file other than the one in this folder
  --json         structured output, for an agent rather than a person
  -h, --help     display help for command
```

## ath log

```
Usage: ath log [options] [entry...]

write something down: a workout result, a measurement, or how you felt

Arguments:
  entry                  the entry, unquoted. Leave it off to paste one, ending
                         with Ctrl-D

Options:
  --benchmark <name>     name the workout yourself, instead of naming it after
                         the day
  --date <date>          the day it happened, as YYYY-MM-DD. Leave it off for
                         today
  --scaling <rx|scaled>  whether the workout was done as written, or scaled
  --file <path>          write to a file other than the one in this folder
  -y, --yes              write it without asking — for scripts, and when you are
                         sure
  --again                record a second result on a day that already has one
  --dry-run              show what would be written, and write nothing
  --json                 structured output, for an agent rather than a person
  -h, --help             display help for command
```

## ath link

```
Usage: ath link [options] <result> <session>

attach a workout result to the device session it happened in

Arguments:
  result         the benchmark, e.g. `fran`, or `fran@2026-09-04` when there are
                 several
  session        the session's start: `17:25`, a full timestamp, or
                 `whoop-1@<timestamp>`

Options:
  --file <path>  change a file other than the one in this folder
  --json         structured output, for an agent rather than a person
  -h, --help     display help for command
```

## ath check

```
Usage: ath check [options] [file]

make sure the file still obeys every rule of the format

Arguments:
  file        the file to check (default: the one in this folder)

Options:
  --json      structured output, for an agent rather than a person
  -h, --help  display help for command
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
  --default <name>  save this model next to the athlete file, for the next
                    predict
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
  benchmark         the benchmark to predict, e.g. `fran`. See them all with
                    `ath stats`

Options:
  --model <name>    the model for this run. See them with `ath models`
  --gateway <name>  vercel or openrouter, when both keys are available
  --as-of <date>    pretend it is this day, hiding everything after it. Does not
                    write. Replay the past with ath backtest (YYYY-MM-DD)
  --file <path>     read a file other than the one in this folder
  -y, --yes         write the prediction without asking
  --dry-run         show the prediction, and write nothing
  --json            evidence for a harness, not a prediction
  -h, --help        display help for command
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
Usage: ath grade [options] <benchmark>

record what actually happened, and score the prediction against it

Arguments:
  benchmark              the benchmark that was attempted, e.g. `fran`

Options:
  --actual <score>       what happened: `4:32` for a time, `245` for reps,
                         `100kg` for a load
  --date <date>          the day of the attempt, as YYYY-MM-DD. Leave it off for
                         today
  --scaling <rx|scaled>  whether the workout was done as written, or scaled
  --analysis <json>      the agent's write-up of a miss, after it has read the
                         dossier
  --file <path>          change a file other than the one in this folder
  -y, --yes              write it without asking — for scripts, and when you are
                         sure
  --again                record a second attempt on a day that already has a
                         result
  --dry-run              show what would be written, and write nothing
  --json                 structured output, for an agent rather than a person
  -h, --help             display help for command
```

## ath series

```
Usage: ath series [options] <quantity>

read a sample series back: one row per day, or the raw samples

Arguments:
  quantity       one of: heart_rate, hrv_beats, ecg_beats, hrv_sdnn,
                 respiratory_rate, oxygen_saturation, steps, active_energy,
                 distance_walking_running, distance_cycling, distance_swimming,
                 running_speed, running_power, running_stride_length,
                 running_vertical_oscillation, running_ground_contact_time,
                 physical_effort, basal_energy, exercise_time, flights_climbed,
                 walking_speed, walking_step_length,
                 walking_asymmetry_percentage, time_in_daylight

Options:
  --from <date>  earliest day to include (YYYY-MM-DD)
  --to <date>    latest day to include (YYYY-MM-DD)
  --source <id>  one device only, for when two measured the same thing — `ath
                 stats` lists the ids
  --raw          every sample, rather than one row per day
  --json         structured output, for an agent rather than a person
  --file <path>  read a file other than the one in this folder
  -h, --help     display help for command
```

## ath stats

```
Usage: ath stats [options] [file]

what is in the file: counts, date ranges, sources, and baselines

Arguments:
  file        the file to summarize (default: the one in this folder)

Options:
  --json      structured output, for an agent rather than a person
  -h, --help  display help for command
```
