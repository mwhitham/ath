# Command-line changes in the native 2.1.1 executable

This file lists every deliberate difference between the 2.1.0 command line
and the native `ath`. Rule IDs refer to [command-behavior.md](command-behavior.md).
Anything not listed here keeps its 2.1.0 words, output, and exit status.
Tests compare the native executable with
[command-inventory.json](command-inventory.json).

## Deliberate changes

### 1. A refusal is printed once (quirk S4)

2.1.0 printed some refusals twice on stderr. This happened when `fail()` ran
inside an action whose `catch` called `fail()` again, as in `log`, `delete`,
`erase`, and `records show`. The native executable prints each refusal once,
as `ath: <message>`. The exit status is still 1.

Reason: the second line repeats the first and carries no information. A
reader or agent that counts error lines would misread one refusal as two.
Handlers now return the failure, and one place prints it.

### 2. `ath init` refuses when it cannot ask its questions

Without `--yes` or `--json`, `ath init` asks four profile questions on
standard input. In 2.1.0, when standard input was not a terminal, the process
stopped after a question with exit status 0 and created no file. The native
executable refuses before asking, with exit status 1:

```text
ath: there is no terminal to ask on, so nothing was created. Pass --yes to skip the questions.
```

When standard input is a terminal but ends before every question is answered
(for example, Ctrl-D at a prompt), 2.1.0 also stopped with exit status 0. The
native executable refuses with exit status 1:

```text
ath: the questions were not answered, so nothing was created. Pass --yes to skip the questions.
```

Reason: exit status 0 told a script that the database was created when it
was not. The wording follows the other non-interactive refusal, `there is no
terminal to ask on, so nothing was written. Pass --yes after reviewing the
preview.` The answers still come from standard input, as in 2.1.0.

### 3. `ATH_GATEWAY_BASE_URL` points the model gateways at a local test server

`ath models`, `ath predict`, and `ath backtest` send gateway requests to the
root named by `ATH_GATEWAY_BASE_URL` when it is set. Only an `http` root on
`127.0.0.1`, `localhost`, or `[::1]` is accepted. Any other value is refused
before a key is read:

```text
ath: ATH_GATEWAY_BASE_URL must name a test server on this computer, such as http://127.0.0.1:8080; nothing was sent
```

Reason: spec section 10 requires tests to use local model servers, and the
executable-level tests run the program as a separate process. 2.1.0 had no
way to change its gateway roots. `ath pull garmin --base-url` already served
the same purpose for the Garmin connection. The loopback rule keeps a stray
value from sending a key to another computer.

### 4. A birth year that is not a number is refused in one line

`ath init --birth-year soon` refused in 2.1.0 with the validator's JSON
issue list inside the message. The native executable names the field and the
reason on one line, and still keeps nothing:

```text
ath: could not create a valid v1 database: data.birth_year: Invalid input: expected number, received NaN. Nothing was kept.
```

Reason: every other validation refusal prints `<path>: <message>`. The exit
status is still 1.

### 5. `ath skill install` cannot lack its shipped skill

2.1.0 refused with `the skill is not here to copy: expected it at <path>`
when its package had no skill folder. The native executable carries the
skill, calculation, and mapping files inside itself (spec section 11.1), so
this refusal no longer exists.

### 6. `ath migrate --help` no longer asks for `sqlite3`

The 2.1.0 help for `ath migrate` ended with this paragraph:

```text
Requires the `sqlite3` command-line tool on PATH for verified backups (same as
`ath repair` when repair candidates exist).
```

The native executable makes and verifies its backups with SQLite's own backup
interface, in the same process (spec section 7). It needs no `sqlite3`
program, so the paragraph is removed. Nothing else in the help changes. The
installed command reference is regenerated from the new help.

Reason: help that names a requirement the program does not have sends a person
to install software for nothing. Tests: `tests/cli_inventory.rs` and
`tests/cli_executable.rs` expect the 2.1.0 help without that paragraph.

## Comparison with 2.1.0

`tools/reference/cli-golden.ts` runs the 2.1.0 command line on
synthetic scenarios and writes `tests/golden/cli/`. The test
`tests/cli_golden.rs` runs the native executable on the same
scenarios. Each golden file lists its normalization rules. The expected
side also applies these listed differences: change 1 above, the native
migration step 5 (see Version), and `deliberate-changes.md` entries 5, 25,
32, 42, and 44.

## Version

`--version`, `-V`, and every version shown by the command line report
`2.1.1`. This follows D152's single version and is not a behavior change.

The native program writes SQL schema step 5 (D211). `ath migrate` previews
list step 5 and name 5 as the target, `ath check --json` reports
`sql_schema_version` 5, and a command that needs a migration says `must be
upgraded to 5`. A 2.1.0 database at step 4 needs `ath migrate` before a
write-capable command runs; read-only commands open it unchanged.
