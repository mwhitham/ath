# Athletic Standard skills

The `ath` executable carries 27 [Agent Skills](https://agentskills.io). Each is a
folder under [`skills/`](../skills/) with a `SKILL.md`. Agents such as Claude Code,
Cursor, Codex, and Gemini CLI read them from `.agents/skills/`.

There are three kinds (decision D228):

- **Tool.** `athletic-standard` finds, reads, logs, imports, and exports records.
- **Calculation.** 24 skills, each running one reproducible calculation with
  `ath apply` or `ath calculate`. Each states its method, minimum data, how to read
  the result, and its limits, and holds a worked example that the test suite runs.
- **Coaching.** `crossfit` and `hyrox` assess, plan, adapt, and review training for
  one sport. Their advice is saved as authored records with the evidence it used.

The record and the tool skill describe what the data shows; they do not prescribe
training. Advice comes only from a coaching skill that the athlete chose to install.
No skill gives medical advice.

## Install

With `ath`:

```sh
ath init                      # creates the database and installs the tool and calculation skills
ath skill install             # adds missing skills; updates an unedited calculation skill
ath skill install hyrox       # installs a coaching skill, which is installed only when named
ath skill                     # shows each copy, its kind, and whether it was edited
```

`ath skill install` replaces an older tool or coaching skill. It replaces an
installed calculation skill only when its `SKILL.md` matches a file an earlier
release shipped, so a person's edits are never lost (D231). An edited calculation
copy keeps running its own method; `ath skill` marks it edited.

With the [skills CLI](https://skills.sh/docs/cli), from a local checkout:

```sh
DISABLE_TELEMETRY=1 DO_NOT_TRACK=1 npx skills add ./skills --skill hyrox
```

Set both variables for any install you do not want counted. The skills CLI sends
anonymous install telemetry to skills.sh by default, and skills.sh lists skills from
that telemetry.

The skills CLI puts each agent's copy in that agent's folder: `.agents/skills/` for
Codex and similar agents, `.claude/skills/` for Claude Code. `ath apply` runs the
calculation in `.agents/skills/<name>/SKILL.md` when it exists, else the copy built
into `ath`. A copy in `.claude/skills/` is read by the agent as instructions; an
edit there does not change what `ath apply` calculates. Run `ath skill install` to
put the calculation files where `ath apply` reads them.

## The skills

### Tool

| Skill | Use it to |
|---|---|
| `athletic-standard` | Read, log, correct, delete, erase, import, export, predict, and grade. |

### Coaching

| Skill | Use it to |
|---|---|
| `crossfit` | Assess an athlete, program strength, gymnastics, and conditioning, scale workouts to keep the intended stimulus, fit work around class, compare benchmarks, prepare for the Open, and review a week. |
| `hyrox` | Plan a race build, progress running and stations, analyse a result from its splits, set a pacing budget, prepare Doubles and Relay teams, and review a race. Rules come from the named season's rulebooks (2026/27). |

### Recovery calculations

| Skill | Answers |
|---|---|
| `hrv-change` | Is this day's RMSSD or SDNN different from the athlete's recent normal? |
| `resting-heart-rate-change` | Is this day's resting heart rate different from normal? |
| `sleep-change` | Was last night's sleep shorter or longer than usual? |
| `resting-heart-rate` | What was the lowest sustained heart rate during the night? |
| `heart-rate-recovery` | How far did heart rate fall in 60 s after the effort? |

### Sleep calculations

| Skill | Answers |
|---|---|
| `sleep-duration` | How long did the athlete sleep? |
| `sleep-efficiency` | What share of time in bed was sleep? |
| `sleep-stages` | How was the night split between stages? |
| `sleep-interruptions` | How often was sleep interrupted? |
| `sleep-timing` | When did sleep start, peak, and end on the local clock? |
| `sleep-regularity` | How regular has sleep been over two weeks? |

### Training calculations

| Skill | Answers |
|---|---|
| `training-load-effort` | What was the session-RPE load of a session? |
| `training-load-heart-rate` | What was the heart-rate training load (Banister TRIMP)? |
| `heart-rate-zones` | How long was spent in each heart-rate zone? |
| `strength-volume` | What was the volume load, per movement? |
| `workout-drift` | Did heart rate drift upward in a steady session? |
| `hyrox-race-splits` | Where did a HYROX race's time go? |

### Summaries

| Skill | Answers |
|---|---|
| `summary` | Count, mean, minimum, and maximum over a period, per device. |
| `baseline` | The athlete's normal and its spread, by day. |
| `trend` | Is a reading going up or down, and how clearly? |
| `coverage` | Which days have readings, and which are missing? |
| `compare-sources` | How far apart are two devices? |
| `correlation` | Do two readings move together? |
| `provider-period-summary` | Values per Oura day or WHOOP cycle. |

## What was verified

| Check | How |
|---|---|
| Calculation methods | `tests/calculate_methods.rs` and `tests/calculate_range_v2.rs` compare each method with values worked out separately, including every failure found in the 2.1.1 audit. |
| Old methods kept | `tests/calculate_golden.rs` runs every recorded 2.1.0 case on the 2.1.1 files and gets the same answers. |
| Skill text matches the program | `tests/skill_examples.rs` runs every worked example and checks every skill's shape, links, and version. |
| Coaching record templates | `tests/skill_examples.rs` validates every `ath-record-example` against the record model. |
| Installation | `tests/skill_install.rs` and `tests/assets_skill.rs`. |
| Agent behaviour | Fresh-session runs are recorded in the [2.1.2 agent evaluation](../build-history/v2.1.2/appendix/agent-evaluation.md). |
| Coaching quality | Independent review by qualified CrossFit and HYROX coaches is required before release and is listed as outstanding in the [2.1.2 progress](../build-history/v2.1.2/progress.md) until it is done. |

## Publishing

`skills.sh.json` at the repository root groups the skills.sh page into the sections
above. Nothing in this repository publishes a skill or submits it to skills.sh. The
maintainer does that by hand. The steps are in
[`docs/publishing-skills.md`](publishing-skills.md).

## Contributing a coaching skill

Running, cycling, and other sports are welcome as coaching skills. They must meet
[`docs/coaching-skill-standard.md`](coaching-skill-standard.md).
