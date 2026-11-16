# Upgrading to 2.1.2

Version 2.1.2 repairs the calculation skills, adds session effort and strength-set
fields to the record, and adds the CrossFit and HYROX coaching skills. The plan and
its decisions are in [build-history/v2.1.2](../build-history/v2.1.2/spec.md).

## A database

A 2.1.1 database is at SQL schema step 5; 2.1.2 writes step 6. Run `ath migrate`. It
shows a preview, makes a verified backup, and asks before it writes. Step 6 adds one
nullable column, `athlete_reports.workout_session_id`, and records version 2.1.2.
It changes no existing row. A 2.1.1 program refuses a 2.1.2 database; to go back,
restore the backup.

Version 2.1.2 adds optional fields, which the published version rule reserves for a
minor version. This is a recorded exception (D227): every 2.1.1 record and package
is valid 2.1.2 data, and nothing is removed or changed.

## Records

- An athlete report can be a `session_effort`: the athlete's rating of a whole
  session, with `rating`, `rating_scale`, and `duration_seconds`.
- An athlete report can name the workout it describes with `workout_session_id`.
  Erasing the workout erases the reports that name it.
- A strength set can name its `movement` and `load_basis` (`total` or
  `per_implement` with `implement_count`).

## Calculations

Several methods were wrong in 2.1.1. They are repaired under new operation names
(D230):

| Skill | What changed |
|---|---|
| `hrv-change`, `resting-heart-rate-change` | Compare the day with earlier days only, in a 28-day window, by daily mean; SDNN samples now work. |
| `sleep-change` | Compares main sleeps only; naps are left out. |
| `heart-rate-recovery` | Measures from the end of effort; it could never succeed before. |
| `sleep-regularity` | Uses the Sleep Regularity Index; identical nights now score 100. |
| `strength-volume` | Converts pounds and counts per-implement loads. |
| `training-load-effort` | Reads only `session_effort` reports. |
| `training-load-heart-rate` | Uses Banister's sex-specific weighting and covered time only. |
| `heart-rate-zones` | Credits only covered time. |
| `resting-heart-rate` | The lowest 5-minute mean; on a sleep session it reads that night's samples. |
| `sleep-efficiency`, `sleep-stages`, `sleep-timing` | Check the denominator, keep awake minutes apart, and give local clock times. |

`ath skill install` (and `ath --update`) replaces a calculation skill you never
edited with the 2.1.2 file. A copy you edited is kept and still runs its old method.
Its result then carries a `method_notice` naming the replacement, and `ath skill`
marks it edited. To adopt the new method, compare your edits with the shipped file
and copy them across.

`ath apply` results now name their `operation`, the other records read (`inputs`),
and what was left out (`exclusions`). `ath apply` refuses a range calculation, such
as `baseline`, and prints the `ath calculate` command to run.

`ath calculate` receipts are `ath-runtime-v2`. Existing fields keep their meaning;
new fields are listed in D244.

## Skills

The coaching skills are installed only when you name them:

```sh
ath skill install crossfit
ath skill install hyrox
```

[docs/skills.md](skills.md) lists every skill.

## JavaScript callers

The Zod schemas accept the new optional fields. `CalculationResult.calculation_version`
is typed as `"ath-runtime-v1" | "ath-runtime-v2"`. No export was added or removed.

## The full list

Every deliberate command-line change is in
[cli-changes.md](../build-history/v2.1.2/appendix/cli-changes.md).
