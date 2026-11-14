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

### Garmin, Strava, the Garmin pull, and mapped imports

These routes ran in the 2.1.0 helper, `ath-core`, which wrote through its own
store. They now run through the one import operation (spec section 8). The
comparison in `native/tests/route_support/keyed.rs` names each normalization
below. Tests: `import_garmin.rs` and `import_mapping.rs`.

17. **One import is one transaction and advances the database revision once.**
    The helper wrote each Garmin, Strava, and pull record in its own
    transaction and advanced the revision once per record. A failure partway
    left the earlier records and the source written. A mapped import was
    already one transaction (D189) but advanced the revision once per row and
    once per new source. Reason: spec section 8 requires one operation whose
    source creation, records, keys, and revision change commit together.

18. **A Garmin or Strava preview reads the database and checks every record.**
    The 2.1.0 preview read only the files, so it showed no deleted records and
    no record the write would refuse. A record the 2.1.0 write refused, such as
    a GPX time without an offset, now fails the preview with the same message
    (entry 9).

19. **A repeated, unchanged Garmin, Strava, or pull import matches its
    records.** The helper compared the new record with the stored one
    including `received_at`, so every repeat wrote a revision of every record.
    The stored `received_at` is now copied onto the new record before the
    compare, as D182 does for mapped imports. Reason: spec section 4.3,
    "Repeating an unchanged import matches existing records."

20. **A Garmin, Strava, or pull key that names a deleted record leaves that
    record out.** The result's `left_out` says `<label>: the stored record <id>
    is deleted`, as mapped imports already did (D182). 2.1.0 stopped with
    `record <id> was not found: Query returned no rows` after writing the
    records before it.

21. **A deleted Garmin, Strava, or pull source leaves its records out**, with
    `<label>: its source <id> is deleted`, as D190 does for mapped imports.
    2.1.0 wrote new records under the deleted source.

22. **One key twice in one Garmin, Strava, or pull input repeats or refuses.**
    The same data names the record again in `record_ids`, as 2.1.0 did.
    Different data refuse the import with `provider export gives one import
    key two different records: <key>`. 2.1.0 merged the second into the first
    as another revision. A key that names another record type also refuses
    the import; 2.1.0 merged the data into the other record.

23. **A revision from these routes keeps the record's extensions.** The helper
    wrote every revision without extensions, so a later import dropped
    extensions a person had added.

24. **`changed_by.tool` is `ath`.** The helper named itself `ath-core`. That
    program no longer exists.

25. **Stored JSON for these routes uses the published field order and
    JavaScript number text.** The helper wrote Rust's formatting, such as
    `40.0`, in its own field order. Reason: one record encoder (spec section 7).
    The comparison reads both as JSON.

26. **A symbolic link inside a Garmin or Strava folder is refused, and
    archives get the provider archive checks.** The helper followed links and
    read archives with no checks or limits. Each file is now limited to
    `MAX_SHARED_FILE_BYTES`, a mapped input to `MAX_MAPPED_INPUT_BYTES`, and a
    pull response to `MAX_PULL_RESPONSE_BYTES` (the helper's HTTP client
    already stopped at that size, with its own message). The pull now waits at
    most 30 seconds to connect and 120 seconds to read.

27. **A pull result lists `left_out` when something was left out.** 2.1.0
    printed no `left_out` field for a pull. It appears only with entries 20
    and 21.

28. **A mapping section with more than one unknown setting names the first in
    file order.** 2.1.0 named whichever one its hash map listed first.

29. **A mapped import's preview no longer writes and rolls back.** It stages
    the plan as every other route does. The outputs are the same; the golden
    comparison covers the deleted-source and deleted-record cases of D189 and
    D190.

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
- An Oura bedtime that ends at exactly `T00:00:00Z` is stored as an
  `oura_day_unspecified` day, not as an instant, even on a sleep row, as in
  2.1.0.
- Garmin, Strava, the Garmin pull, and mapped imports still refuse a supplied
  field named `__proto__` or with an empty name (D191), with the 2.1.0
  message, although entry 4 keeps such keys elsewhere.
- A Garmin or Strava revision is merged over the stored record and names the
  basis `local_write`, as the helper's `revise` wrote it (D182 kept that merge
  for the built-in routes).
- A Garmin activity's `startTimeGMT` must carry an offset. Garmin's own
  `2026-01-02 15:00:00` form stops the import with `could not read time`, as
  in 2.1.0.
- A Strava export is recognized by any name ending in `activities.csv`, but
  only a file named exactly `activities.csv` is read, as in 2.1.0.

## Open work

- The command-line changes chosen for 2.1.1 are listed in `cli-changes.md`.
