# What a skill does when `ath` cannot run

This study supports D250. It was done on 2026-11-19, after the 2.1.2 skills were
built and before any were published.

## 1. The problem

Every skill runs `ath`. Skills reach an agent in two ways:

- **Through `ath`.** `ath init` and `ath skill install` copy the skills, so `ath`
  is present and its version matches them.
- **Through the skills CLI.** `npx skills add` copies only the text, and any subset
  can be installed. The skills CLI has no way to declare that a skill needs a
  program or another skill (its README, read on 2026-11-19).

The gap belongs to the second way. There, three states can occur:

| State | What the agent sees | What can explain it |
|---|---|---|
| `ath` is not installed | The shell's "command not found" | Only the skill text. `ath` never runs. |
| `ath` is too old | Whatever the old program prints | Only the skill text. A released program cannot be changed. |
| `ath` runs but finds no database | `ath`'s own message | `ath` itself |

## 2. Facts that shaped the design

- **npm's `latest` is 1.1.0.** The registry lists 0.1.0 to 1.1.0 for
  `athleticstandard`. Someone following the README today installs 1.1.0. It has no
  `ath apply --record`, and it refuses a 2.1.2 database with "Unsupported Athletic
  Standard database version: 2.1.2". Until 2.1.2 is published, "too old" is the
  likely state.
- **An unrelated npm package is named `ath`.** It is version 1.0.0, with no
  description and no command. An agent that guesses `npm install -g ath` installs
  the wrong package. So the text must name `athleticstandard`.
- **`compatibility` is not enough.** The Agent Skills specification defines it as
  "Max 500 characters. Indicates environment requirements (intended product, system
  packages, network access, etc.)", and notes "Most skills do not need the
  `compatibility` field" (agentskills.io/specification). Claude Code "accepts the
  field but doesn't act on it" (code.claude.com/docs/en/skills). Cursor's skill
  fields do not include it. The body is what every agent reads.
- **Anthropic's guidance.**
  - Under "Avoid assuming tools are installed": "Don't assume packages are
    available". Its checklist includes "Required packages listed in instructions
    and verified as available" (Skill authoring best practices).
  - For Claude Code: "Global package installation discouraged: Skills should only
    install packages locally to avoid interfering with the user's computer" (Agent
    Skills overview, runtime constraints).
  - The same guide says to "Build evaluations first": find the gap, set a baseline,
    and "Write minimal instructions". This study follows that order.
- **Published skills that wrap a command-line program.** Firebase, Cloudflare,
  Supabase, and Vercel skills check the program's version first and give an install
  command. None of them asks before installing. This summary comes from a research
  agent's reading of their `SKILL.md` files on 2026-11-19. It was not re-checked
  line by line.
- **The SQL contract allows reading.** `spec/sql/README.md` publishes stable
  `sql_v1_` read views for other programs. So the skill does not say that reading
  the database is wrong in general. It says that when a skill's answer needs
  records or a calculation and `ath` cannot run, the agent stops. The calculations
  and their checks, such as the overlap refusal in `hyrox_splits`, run only in `ath`.

## 3. The design and why

Each fact goes where the agent will meet it, and each has one source.

- What only the text can say goes in every skill: how to check `ath`, the real
  package name, asking first, and stopping instead of working around `ath`. Any
  skill may be installed alone, so a pointer to the tool skill is not enough.
- That text is generated from `assets::skill::prerequisite_section`. The
  generated-file check fails if a copy differs, and the version comes from the
  release constant. This follows the pattern already used for
  `skills/athletic-standard/references/cli.md`.
- What `ath` can see goes in `ath`'s messages. The no-database message already
  names `--file` and `ath init`. Two messages were extended: a database from a newer
  `ath`, and a skill operation this `ath` does not have
  ([cli-changes.md](cli-changes.md), item 6).

## 4. Testing the gap before and after

The method, the criteria (written before any run), and every result are in the
[agent evaluation](agent-evaluation.md), under "Setup cases". In short:

| State | Before (today's text) | After (D250 text) |
|---|---|---|
| `ath` not installed | Failed. Read the database with `sqlite3` and added the splits by hand. Never named the package. | 2 of 2 passed. Stopped, named `athleticstandard`, warned about the npm `ath` package, and asked before installing. |
| `ath` 1.1.0 | Failed. Saw the old version and offered `ath --update`, but then read the database with `sqlite3` and did the splits by hand. | 2 of 2 passed. Stopped, offered `ath --update` or the npm install, and asked first. |
| No database in the folder | Passed. Found `record/` and used it. | 2 of 2 passed. Checked the version, then used `--file`. |

Because the no-database baseline passed, its message was left unchanged.

## 5. Limits

- One baseline run and two later runs per state, all with one model. This shows the
  text can change the behaviour, not how often it does across models.
- In six runs the agent read the test wrapper that stood in for `ath`. In the 1.1.0
  state that wrapper showed that `--update` was blocked. The baseline agent read it
  too, and it also did not run `--update`. The change that the new text made, not
  reading the database by hand, is not something the wrapper blocked.
- The `npx -y athleticstandard@<version>` form was not run. It cannot resolve until
  2.1.2 is published. `docs/publishing-skills.md` now asks for that check before the
  skills are listed.

## 6. Updating from the versions people have installed (D253)

This section was added on 2026-11-19, once the maintainer set 2.1.2 to be released
that day. No 2.x version had reached npm, so everyone with `ath` has 0.x or 1.x.

What the published packages do, read from each tarball on npm:

| Versions | `--update` | After the update |
|---|---|---|
| 0.1.0 to 0.4.1 | none | `npm install -g` copies no skill files. |
| 0.4.3, 1.0.0, 1.1.0 | runs `npm install -g athleticstandard@latest` | Copies its own `skill/` folder into the project. The install has just replaced the package, and 2.1.2 ships `skills/`, so nothing is copied. |

0.x used prerelease JSON records, which 2.x does not import (D101). A 1.x database
upgrades with `ath migrate`.

Runs, with the criteria written first. All used the `hyrox` and `hyrox-race-splits`
skills and the HYROX database:

| Case | Setup | Result |
|---|---|---|
| u1 | Simulated 1.1.0 whose `--update` succeeds; the athlete allows updates. D250 text. | Updated, checked the version again, and answered with `apply hyrox-race-splits`. No `ath skill install`, which made no difference here: the skills came from the skills CLI and were already 2.1.2 text. |
| u2 | Simulated 0.4.1 with no `--update`; the athlete allows updates. D250 text. | After "unknown option", ran `npm install -g athleticstandard@latest` by itself, which is the right package because the D250 text names it. Checked the version again and answered correctly. So the D250 text needs no fallback line. |
| o1 | A folder set up by `ath init` 1.1.0, `ath` already 2.1.2. The athlete allows updates. | Without the notice and with it: both ran `ath skill`, saw the 1.1.0 copy, ran `ath skill install`, and answered with the calculation. The request's invitation may have prompted the check. |
| o2 | The same folder; the athlete asks only "Where did I lose time in my HYROX race on 18 October?" | Without the notice: never learned that the skill was old, never found `hyrox-race-splits`, and added the splits itself. With the notice: saw it on the first command, ran `ath skill install`, and answered with the calculation. It ran the install without asking, and said it should have asked. |

The D250 section cannot reach an agent in an old folder, because that agent reads the
old tool skill. The notice in `ath` can. That is why D253 is a program change and not
a skill change. One run per case; one model.
