# Deliberate behavior changes in 2.1.1

The native rebuild keeps 2.1.0 behavior unless this list names a change.
Each entry states the change, the reason, and the test that proves it. Spec
section 13.1 requires every difference from the reference program to be listed
here; an unlisted difference is a bug.

## Record format and validation

1. **Version 2.1.1 is accepted.** The record-version enum gains `2.1.1`, so
   messages that list accepted versions name it. Reason: D208 selects 2.1.1.
   Test: `native/tests/model_vectors.rs` skips the 28 version-message vectors.

2. **Lone UTF-16 surrogate escapes are refused when JSON is read.** JavaScript
   accepts text such as `"\ud800"`. Rust text cannot hold a lone surrogate, so
   the reader returns an error instead of guessing a replacement. Reason: a
   replacement would change supplied data silently. Test: `json_vectors.rs`.

3. **Numbers outside the double range are refused when JSON is read.**
   JavaScript reads `1e400` as Infinity and writes it back as `null`. The
   reader refuses the text. Reason: writing `null` would change a value
   silently. Test: `json_vectors.rs`, and 31 skipped model vectors.

4. **A `__proto__` key is kept and checked like any other key.** The 2.1.0
   validator dropped a `__proto__` key and everything under it from supplied
   fields and extensions. That hid a nested `source_id` from the rule that
   athlete-reported records cannot claim a source. The rebuild keeps the key,
   stores it, and applies every rule to it. Reason: the two-tier wall must not
   depend on a JavaScript object quirk. Test: `model_rules.rs`.

## Runtime services

5. **`ath share` finds the report `ath backtest` writes.** 2.1.0 looked for
   `backtest-<date>T<HHMMSS>.json`, but backtest writes
   `backtest-<date>T<HHMMSSmmm>Z.json`, so share never found a report. Both
   names are now accepted. Test: `runtime_share.rs`.

6. **A password-store refusal ends with one period.** 2.1.0 printed the tool's
   own period and then another. Test: `runtime_keyring.rs`.

7. **`ath --update` hands its install plan to the npm launcher.** The native
   program never runs a package manager itself; it returns a checked program
   and argument list that the launcher runs after the native process exits.
   Global npm, pnpm, and bun installs run without the launcher get the exact
   command to run. Cargo and archive installs get precise instructions. Reason:
   spec section 11.3. Test: `runtime_update.rs`.

## Provider imports

8. **A WHOOP cycle timezone written only in letters, such as `PST`, skips the
   row.** The 2.1.0 helper read it as UTC, which moved every time in the row by
   hours, and it cut long digit strings such as `+12345` to `+12:34`. The rule
   `tests/csv.test.ts` already tested is now applied: the row is skipped with
   an example. Test: `import_whoop.rs`.

9. **An invalid candidate fails the preview.** In 2.1.0 the preview succeeded
   and the confirmed write failed. Test: `import_whoop.rs`.

10. **New refusals for misuse.** A preview applied to a different database, or
    a candidate that names a source its reader never declared, is refused.

11. **Unusual characters no longer crash a reader.** A non-ASCII character at
    a fixed time position makes the row unreadable instead of stopping the
    program.

12. **Folders are read in byte order on every system.** Node used byte order
    on Unix but not necessarily on Windows. A stable order keeps previews the
    same everywhere.

13. **Corrupt zip archives report the Rust zip library's error text.** The
    refusal and exit status are unchanged.

14. **Garmin JSON detection reads the first 32 KB, not the whole file.** The
    detected route is the same; large files no longer load fully to detect.

15. **Skip reasons sort with JavaScript `localeCompare` rules for ASCII
    text.** Other characters sort by code point.

16. **New records name tool version 2.1.1 in `changed_by`.**

## Database maintenance

17. **All migration steps run in one transaction.** A failure leaves the file
    at its starting step, never half migrated. Test: `migration.rs`.

18. **A change after the migration or repair backup is a stale refusal.**
    2.1.0 told the person to restore the backup, which would have lost the
    other writer's work. The new refusal asks for a new preview and backup.
    Test: `migration.rs`.

19. **A verified backup is one self-contained file.** It is switched to
    rollback-journal mode, and verification also compares the database
    revision. Test: `backup.rs`.

20. **`ath check` reads one snapshot and reports more problems instead of
    stopping.** Damaged extensions or unreadable JSON are issues; 2.1.0
    stopped with an error. Foreign-key findings are sorted, because SQLite
    versions list them in different orders. Pending attachment file work is
    a `pending_file_operation` warning. A missing attachment reports the
    system's own file error text. Test: `check_repair.rs`.

21. **A read-only open of a WAL database in a folder SQLite cannot use is
    refused clearly.** 2.1.0 said the metadata table was missing. The file is
    not changed. Test: `read_policy.rs`.

## Prediction and backtest

22. **A backtest replay cannot see the result it predicts.** 2.1.0 set the
    cutoff one millisecond before the result's latest revision became known.
    A corrected result then showed the model its own first revision as a prior
    result, and a result logged late let the model see later events. The
    cutoff is now one millisecond before the earlier of the result's event
    time and the time the database first learned of it, for both event and
    knowledge filters, as `predict --as-of` does. The report field
    `replays[].as_of` changes for corrected or late-logged results. Reason:
    spec section 10. Test: `prediction.rs` (corrected outcome, late import).

23. **A model reply cannot set `authorship` or `benchmark_definition_id`.**
    2.1.0 merged the reply over those fields, so a model could claim athlete
    authorship or move its prediction to another benchmark. The refusal reads
    `model prediction response cannot set <field>; ath sets the authorship and
    benchmark`. Test: `prediction.rs`.

24. **A non-numeric predicted score in a backtest is a model failure.** 2.1.0
    converted it to a number.

25. **`log --json` lists record data in published field order,** the order
    the stored record uses. 2.1.0 used the order the planner built.

26. **Text after "not valid JSON:" comes from the Rust JSON reader.** The
    prefix and exit status are unchanged.

27. **A calendar range or `--as-of` day where midnight does not exist** (for
    example in America/Santiago) fails with the message the 2.1.0 search gave.

## Behavior kept on purpose

These look odd but stay, because changing them would change accepted data or
stored bytes:

- Issue order, paths, and wording match the 2.1.0 validator, including the
  extension issue paths that split an extension ID on its dots.
- A current record must equal its last retained revision as JSON text, so key
  order inside supplied fields matters.
- Provider import keys are compared after joining method and key with U+0000.
- Time-order refinements read timestamps as `Date.parse` does, so a malformed
  sleep or workout end still reports the order rule.
- Fixed UTC offsets such as `+05:00` are accepted as a profile timezone, as the
  2.1.0 validator accepted them.
- On macOS, `ath key set` still passes the key to the `security` tool as a
  command argument, as 2.1.0 did. Another local process could read it while the
  tool runs. Changing the call needs a test on a real Keychain, so it is listed
  as an open question for the owner.

- Import source lookup uses `LIMIT 1` without `ORDER BY`, as in 2.1.0.
- A WHOOP time with a space before its offset fails the whole import, as in
  2.1.0.
- A single CSV with a nonstandard file name imports nothing and is reported as
  an unrecognized file, as in 2.1.0.

- A child JSON row that cannot be parsed is reported as "record_json is not
  valid JSON", as in 2.1.0.

- `hrv-change`, `resting-heart-rate-change`, and `sleep-change` describe
  "earlier records" but average every other same-source record, including
  later ones. Spec section 10 forbids changing a calculation method in this
  rebuild. This needs a separate format decision.
- `heart-rate-recovery` looks for a sample 60 seconds after the last sample,
  so it always reports the field missing.
- Range calculations reject a timezone name in the wrong letter case; calendar
  views accept it.
- `correlation` against an empty comparison source refuses with
  "incompatible units:".
- Measurements that have only a provider period never appear in prediction
  evidence.
- A backtest stops when evidence cannot be built for one replay.

## Open work

- The command-line changes chosen for 2.1.1 are listed in `cli-changes.md`.
