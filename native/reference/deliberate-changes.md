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

## Packages and attachments

28. **A merge can add a deleted record or delete a record.** 2.1.0 set a
    record's deleted flag before it inserted the deletion row, so the
    database rule refused the merge with "records.is_deleted must match
    record_deletions" and nothing was imported. The deletion row is now
    written first, in the same transaction. Test: `package_import.rs`
    (`a_merge_adds_deleted_records_and_deletions`).

29. **Merge compares records by logical value.** Key order inside supplied
    fields, extensions, and other JSON maps is ignored, and numbers compare as
    JavaScript numbers. 2.1.0 compared JSON text. A package sorts its keys and
    the database keeps the order a record was written in, so importing a
    database's own export reported "record revision history differs" for
    such records. A matching record is not written, so its stored bytes stay
    as they were. Test: `package_import.rs`
    (`reordered_supplied_fields_match_and_keep_the_local_bytes`,
    `a_database_s_own_export_imports_without_conflicts`).

30. **Export refuses an attachment reached through a symbolic link.** The
    file, or a folder on its path, may not be a link, and the path must be a
    regular file. 2.1.0 followed the link and copied whatever it named when
    the size and hash happened to match. Reason: spec section 9.2 file
    safety, as the other attachment operations already required. Test:
    `package_export.rs` (`an_attachment_reached_through_a_symbolic_link_is_refused`).

31. **A resolved conflict keeps the chosen side's extensions.** An imported
    choice writes the package record's extensions. A corrected choice keeps
    the current local extensions, because `--data` holds the data object only
    and cannot carry extensions; this matches `ath correct`, which also keeps
    extensions. A local choice writes nothing. 2.1.0 dropped extensions from
    every resolved revision. Test: `package_import.rs`
    (`a_resolved_conflict_keeps_the_chosen_side_s_extensions`).

32. **`conflicts show` without a conflict file says how to make one:** `no
    temporary import conflicts; run ath import <package> to create a
    preview`. 2.1.0 printed a raw ENOENT message (the quirk listed in
    `command-behavior.md`). Test: `package_import.rs`
    (`a_missing_conflict_file_says_how_to_create_one`).

33. **An invalid conflict file names the first problem in a short phrase**
    after `temporary import conflict file is invalid:`. 2.1.0 printed the
    validator's JSON issue list there. The file is refused in the same cases.
    Test: `package::conflicts` unit test.

34. **Package JSON that is not UTF-8 is refused.** 2.1.0 replaced invalid
    bytes with U+FFFD and imported the result. Lone surrogates and numbers
    beyond the double range are refused as entries 2 and 3 describe. Text
    after `package JSON is invalid: <path>:` comes from the Rust JSON reader.
    Reason: a replacement would change supplied data silently. Test:
    `package_import.rs` (`malformed_deep_oversized_and_linked_documents_are_refused`),
    `serialization_vectors.rs`.

35. **Attachment files are recoverable across a stopped process.** New
    attachment files (`log`, `correct`, and package import) are recorded as
    pending, written, and synchronized before their records commit, and the
    commit clears the pending entries. A process that stops first leaves only
    files no record names. Every command that opens a database for writing
    removes them once the process that wrote them has stopped, and keeps any
    file a record names. 2.1.0 removed files only on an error it saw. Reason:
    spec section 9.2. Test: `package::tests` (failure injection),
    `package_attachments.rs`, `cli_edit.rs`.

36. **Package import copies attachment files before its transaction.**
    2.1.0 renamed them into place inside the transaction. The files are now
    in place, recorded as pending, before the records commit, and are removed
    if the import fails. Each file's hash and size are checked again when it
    is copied, and an existing file is never replaced. Test:
    `package_import.rs`, `package::tests`.

37. **Erasure removes attachment files after its commit and reports any it
    could not remove.** 2.1.0 renamed the files aside before the transaction
    and deleted them after it. Now the erasure records each removal as
    pending in the same transaction that erases the records, then removes the
    files and synchronizes the folder. A file that cannot be removed makes the
    command fail with a message that says the records are erased and the
    removal stays pending; `ath check` reports it and the next writable
    command finishes it. `erased_attachment_count` counts the files actually
    removed. Test: `package_erase.rs`, `package::tests`.

38. **A missing attachment during export is named:** `attachment file is
    missing: <path> (<record ID>)`, instead of the operating system's error
    text. Test: `package_export.rs`.

39. **Stored attachment names follow the POSIX extension rule on every
    system.** 2.1.0 used the platform's path rules, so on Windows a
    backslash in a file name split the name. Reason: a stored name must not
    depend on where the record was written. Test: `files::attachments` unit
    tests.

40. **Series files are read in an order that follows `localeCompare` for
    ASCII text** (IDs, quantity names, timestamps, and paths). Text outside
    ASCII sorts by code point. The order sets only which sample IDs fill a
    shortened preview list and which problem is reported first. Test:
    `package::load` unit test.

41. **Export writes each file in its private staging folder once,** without
    a second temporary name, and synchronizes the folder before the one
    rename that publishes it. The published files are the same. Test:
    `package_export.rs`, `package::tests`.

## Garmin, Strava, the Garmin pull, and mapped imports

These routes ran in the 2.1.0 helper, `ath-core`, which wrote through its own
store. They now run through the one import operation (spec section 8). The
comparison in `native/tests/route_support/keyed.rs` names each normalization
below. Tests: `import_garmin.rs` and `import_mapping.rs`.

42. **One import is one transaction and advances the database revision once.**
    The helper wrote each Garmin, Strava, and pull record in its own
    transaction and advanced the revision once per record. A failure partway
    left the earlier records and the source written. A mapped import was
    already one transaction (D189) but advanced the revision once per row and
    once per new source. Reason: spec section 8 requires one operation whose
    source creation, records, keys, and revision change commit together.

43. **A Garmin or Strava preview reads the database and checks every record.**
    The 2.1.0 preview read only the files, so it showed no deleted records and
    no record the write would refuse. A record the 2.1.0 write refused, such as
    a GPX time without an offset, now fails the preview with the same message
    (entry 9).

44. **A repeated, unchanged Garmin, Strava, or pull import matches its
    records.** The helper compared the new record with the stored one
    including `received_at`, so every repeat wrote a revision of every record.
    The stored `received_at` is now copied onto the new record before the
    compare, as D182 does for mapped imports. Reason: spec section 4.3,
    "Repeating an unchanged import matches existing records."

45. **A Garmin, Strava, or pull key that names a deleted record leaves that
    record out.** The result's `left_out` says `<label>: the stored record <id>
    is deleted`, as mapped imports already did (D182). 2.1.0 stopped with
    `record <id> was not found: Query returned no rows` after writing the
    records before it.

46. **A deleted Garmin, Strava, or pull source leaves its records out**, with
    `<label>: its source <id> is deleted`, as D190 does for mapped imports.
    2.1.0 wrote new records under the deleted source.

47. **One key twice in one Garmin, Strava, or pull input repeats or refuses.**
    The same data names the record again in `record_ids`, as 2.1.0 did.
    Different data refuse the import with `provider export gives one import
    key two different records: <key>`. 2.1.0 merged the second into the first
    as another revision. A key that names another record type also refuses
    the import; 2.1.0 merged the data into the other record.

48. **A revision from these routes keeps the record's extensions.** The helper
    wrote every revision without extensions, so a later import dropped
    extensions a person had added.

49. **`changed_by.tool` is `ath`.** The helper named itself `ath-core`. That
    program no longer exists.

50. **Stored JSON for these routes uses the published field order and
    JavaScript number text.** The helper wrote Rust's formatting, such as
    `40.0`, in its own field order. Reason: one record encoder (spec section 7).
    The comparison reads both as JSON.

51. **A symbolic link inside a Garmin or Strava folder is refused, and
    archives get the provider archive checks.** The helper followed links and
    read archives with no checks or limits. Each file is now limited to
    `MAX_SHARED_FILE_BYTES`, a mapped input to `MAX_MAPPED_INPUT_BYTES`, and a
    pull response to `MAX_PULL_RESPONSE_BYTES` (the helper's HTTP client
    already stopped at that size, with its own message). The pull now waits at
    most 30 seconds to connect and 120 seconds to read.

52. **A pull result lists `left_out` when something was left out.** 2.1.0
    printed no `left_out` field for a pull. It appears only with entries 45
    and 46.

53. **A mapping section with more than one unknown setting names the first in
    file order.** 2.1.0 named whichever one its hash map listed first.

54. **A mapped import's preview no longer writes and rolls back.** It stages
    the plan as every other route does. The outputs are the same; the golden
    comparison covers the deleted-source and deleted-record cases of D189 and
    D190.

55. **A Garmin, Strava, or pull revision is marked `provider_import`.** The
    2.1.0 helper marked a revision from these routes `local_write`, as if the
    athlete had made it. A provider supplied the data, so the revision's
    `became_known_at_basis` now says so, as WHOOP, Oura, and mapped imports
    already did. Test: `import_garmin.rs`.

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

- A package merge advances the database revision by one even when every
  record already matches and nothing else is written, as in 2.1.0. Test:
  `package_import.rs` (`a_restore_preview_writes_nothing_then_the_restore_keeps_everything`).
- A merge that adds revisions to a record deleted here, when the package does
  not delete it, is refused by the database rule that a deleted flag needs its
  deletion row, as in 2.1.0. Nothing is written. Undeleting a record through a
  merge needs its own decision.
- Package preview lists still collect sample IDs in series-file order before
  sorting, so a shortened list holds the first 1,000 seen, as in 2.1.0.

- Garmin's own `startTimeGMT` form without an offset (`2026-01-02 15:00:00`)
  stops a Garmin import, as in 2.1.0. Reading it as UTC would accept data that
  2.1.0 refused, so it waits for a separate decision.
- Strava is detected by any file name ending in `activities.csv`, but only a
  file named exactly `activities.csv` is read, as in 2.1.0.
- An Oura bedtime that ends exactly at midnight UTC becomes a day with no time,
  as in 2.1.0.
- Garmin, Strava, pull, and mapped imports still refuse a supplied field named
  `__proto__`, as D191 requires. Change 4 keeps that key everywhere else.

## Open work

- The command-line changes chosen for 2.1.1 are listed in `cli-changes.md`.
