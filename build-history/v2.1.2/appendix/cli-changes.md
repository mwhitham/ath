# Command-line changes in 2.1.2

This file lists every deliberate difference between the 2.1.1 command line and
2.1.2. Anything not listed here keeps its 2.1.1 words, output, and exit status.
The 2.1.1 changes are in [the 2.1.1 list](../../v2.1.1/appendix/cli-changes.md).

## 1. `ath skill install [name...]` (D229)

`ath skill install` takes optional skill names. With no names it works as
before, and it now also updates an unedited calculation skill (D231). With names
it installs only those skills, including a coaching skill such as `crossfit` or
`hyrox`, which is otherwise never installed. A name the program does not ship is
refused and nothing is written. The help for `ath skill` and `ath skill install`
describes this. Test: `tests/cli_inventory.rs` checks the new words; the full
text is in the generated `skills/athletic-standard/references/cli.md`.

## 2. `ath skill` names each copy's kind and marks an edited calculation

`ath skill --json` gives each copy a `kind` (`tool`, `calculation`, or
`coaching`) and an `edited` flag. A coaching skill that is not installed has the
state `available`. The text output says when a calculation copy is edited and
kept, and how to install a coaching skill.

## 3. `ath apply` names the operation and what it read (D230)

The `ath apply --json` object gains `operation`, and, when they apply, `inputs`
(the other records the method read), `exclusions`, and `method_notice` (for an
operation replaced in 2.1.2). The text output adds `compared with:`, `left out:`,
and `note:` lines. Every recorded 2.1.0 result still matches on the 2.1.1
calculation files: `tests/calculate_golden.rs` and `tests/cli_golden.rs`.

## 4. `ath apply` refuses a range calculation and a coaching skill (D243)

`ath apply baseline --record <id>` printed a null result with "unknown
calculation mean". It now exits with status 1 and prints the `ath calculate`
command to run. `ath apply hyrox --record <id>` says that `hyrox` is a coaching
skill, not a calculation. The tool skill keeps its 2.1.0 refusal.

## 5. `ath calculate` writes receipt version 2 (D244)

`calculation_version` is `ath-runtime-v2`, and results gain the fields D244
lists. Every recorded 2.1.0 receipt still matches once those fields are set
aside: `tests/calculate_golden.rs`, `tests/cli_golden.rs`, and
`tests/calculate_range_v2.rs` for the additions.

## 6. Two messages say when `ath` is older than what it reads (D250)

A database written by a newer `ath` was refused with "Database SQL schema N is newer
than supported schema 6" and no next step. The message keeps that wording and adds
"A newer ath wrote this database. Update ath to open it, for example with ath
--update." The check is `tests/migration.rs`.

A calculation skill whose `use:` names an operation this `ath` does not have still
returns a null result. Its `missing` entry keeps "unknown calculation X" and adds
"this skill file may need a newer ath (ath --update)". The check is
`tests/calculate.rs`.
