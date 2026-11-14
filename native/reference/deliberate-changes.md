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

## npm client

The JavaScript exports keep their 2.1.0 names, signatures, and result
shapes (`packages/npm/test/public-api.generated.ts`). These are the
differences a caller can see. They are numbered N1 onward so this list does
not collide with the native entries.

N1. **Validation errors from database exports are ZodError objects rebuilt
    from the native issues.** Each issue has `path`, `code`, and `message`.
    Zod's type-specific issue fields and its message wording are not
    reproduced, so `error.message` differs. `instanceof ZodError` and the
    issue paths still work. Reason: native writes validate in Rust only
    (spec section 6). Test: `packages/npm/test/database.test.ts`.

N2. **Each call opens and checks the database file again in its own native
    worker.** A different database at the same path is refused: "The
    database at <path> is not the one this handle opened". A restore through
    `importRecords` updates the handle to the restored database ID. Reads open
    the file read-only. Reason: spec section 11.2. Test: `protocol.test.ts`,
    `database.test.ts`.

N3. **`openDatabase` and `inspectDatabase` report security warnings after the
    open succeeds.** When the open fails, for example because the database
    needs migration, the warnings are not reported.

N4. **`withPreparedAttachmentFiles` writes each file with the write that
    creates its attachment record.** The file and the records commit together
    through the native pending-file rules. 2.1.0 wrote the files before
    calling `write` and removed them when `write` failed. A `write` that
    creates no prepared attachment record now leaves no file behind, and an
    existing file at the final path is never replaced. Test: `database.test.ts`.

N5. **A provider preview names its export and import time, and
    `importProviderRecords` reads the export again.** A preview this client
    did not make, or an export or database that changed after the preview, is
    refused. Edits a caller made to the preview object are not applied.
    `plan.drafts` carry `record_id`, `record_type`, `data`, `match_key_method`,
    and `match_key`; `collision_group`, `existing_record_id`, and
    `changes[].current` are not included.

N6. **Package previews, imports, and series-file reads load the package again
    from `source_path`.** A package whose fingerprint changed is refused with
    "the package changed after it was loaded; load it again". Edits a caller
    made to a `LoadedPackage` in memory are not applied.

N7. **Remote libSQL targets open no connection.** The 2.1.0 refusal messages
    are kept: `URL_SCHEME_NOT_SUPPORTED` for an unsupported scheme, and
    "SQLite foreign-key enforcement could not be enabled" for a supported one.
    The first is a plain `Error` named `LibsqlError` with the same `code`, not
    an instance of the libSQL client's class. An unreachable server gave a
    network error in 2.1.0. `writeV1BacktestReport` refuses a URL instead of
    writing under a folder named after it. See `packages/npm/REMOTE-TARGETS.md`.

N8. **The render exports check the records they are given.** `renderRecord`,
    `renderRecordList`, and raw `renderSeriesView` refuse an object that is not
    a valid current record or revision. 2.1.0 printed whatever it was given.
    `renderDatabaseStats` lists `by_type` in record-type name order, which is
    the order `databaseStats` returns.

N9. **Synchronous helpers run a short native process.** `createUuidV7`,
    `buildMigrationPreview`, `calendarDateInZone`, `deterministicJson`, the
    backup-name helpers, `conflictResolutions`, `addAttachmentsToLogPlan`, and
    the render exports keep their synchronous signatures and results. Each
    call costs a process start.

N10. **`applyPendingDatabaseMigrations` and `migrateDatabase` apply every
     pending step in one transaction,** as native item 17 describes.
     `createVerifiedSqliteBackup` uses the SQLite backup API, as native
     item 19 describes, and backs up the file at `sourcePath`.

N11. **Errors from the native worker carry a non-enumerable `code`,** such as
     `ATH_REFUSED`, `ATH_STALE`, or `ATH_CANCELLED`. Messages are unchanged.

N12. **`SQL_SCHEMA_VERSION` is 5** and the JSON Schema exports describe
     2.1.1. These are version declarations under D208.

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

- A package merge advances the database revision by one even when every
  record already matches and nothing else is written, as in 2.1.0. Test:
  `package_import.rs` (`a_restore_preview_writes_nothing_then_the_restore_keeps_everything`).
- A merge that adds revisions to a record deleted here, when the package does
  not delete it, is refused by the database rule that a deleted flag needs its
  deletion row, as in 2.1.0. Nothing is written. Undeleting a record through a
  merge needs its own decision.
- Package preview lists still collect sample IDs in series-file order before
  sorting, so a shortened list holds the first 1,000 seen, as in 2.1.0.

## Open work

- The command-line changes chosen for 2.1.1 are listed in `cli-changes.md`.
