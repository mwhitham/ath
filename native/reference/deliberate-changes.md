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

### Apple Health

17. **A GPX time without a UTC offset is left out of route summaries.** 2.1.0
    read such a time in the computer's timezone, so a workout's splits
    depended on the machine. The retained baseline `04-apple-mismatch.json`
    was recorded in UTC and opens the first split five hours early (17,881.6
    seconds); the native split is 1.6 seconds. The stored route already left
    that point out. The Apple golden files are recorded with this rule. Test:
    `import_apple_behavior.rs`.

18. **An Apple Health export of any size can be read.** 2.1.0 returned the
    helper's whole result through a 64 MiB buffer, so a large export failed.
    The native reader streams `export.xml` and keeps records that must wait
    for later files in a private temporary file. One million samples import
    in about 45 MiB of memory. Test: `import_apple_large.rs`, run by hand.

19. **A series sample whose time cannot be placed refuses the preview.** A
    date such as `2026-08-01 25:00:00 -0700` passed 2.1.0's shape check. It
    was then stored at the time of that day's first sample, or the import
    stopped with JavaScript's `Invalid time value` when it sorted first. The
    native preview names the text, as 2.1.0 already did for a point
    measurement. Test: `import_apple_behavior.rs`.

20. **Route time text must be ISO 8601.** A GPX time is read by the rules in
    `instant.rs`. 2.1.0's `Date.parse` also accepted other forms, such as
    `Sun Aug 02 2026 08:03:00 GMT-0500`. A point with such a time is counted as
    a point with no usable time. GPX files write ISO 8601.

21. **Route distances use one portable calculation.** 2.1.0 measured routes
    with V8's sine, cosine, and arcsine. On 64-bit ARM, V8's fdlibm used fused
    multiply-add steps; on x86-64 it did not, so the last bit could differ
    between machines. The native code repeats the 64-bit ARM steps on every
    platform, which matches the recorded golden values bit for bit. A split
    could differ from a 2.1.0 x86-64 result only when it sits within one unit
    in the last place of a rounding boundary. For angles beyond 2^19 * pi/2
    radians (a coordinate beyond about 47 million degrees), the platform's own
    function is used. Test: `haversine_distances_match_v8_bit_for_bit`.

22. **The XML inside a zip is measured while it expands.** 2.1.0 trusted the
    archive's declared size for `export.xml`. The native reader refuses an
    entry that expands past its declared size or the 16 GiB limit, as spec
    section 8 requires for every archive entry.

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
- Apple workout splits store their time under the summary type
  `sleep_duration`, because 2.1.0 mapped every `duration_s` to that name.
- An Apple workout's `totalEnergyBurned` is stored as kilocalories without
  conversion, whatever unit the export names, as in 2.1.0.
- An Apple record with an empty `unit` attribute, or beats whose RMSSD rounds
  to zero, cannot be stored. 2.1.0 refused the whole write; the native preview
  refuses it (change 9).
- A route's calculated elevation gain is stored with an input value equal to
  the standard value, as 2.1.0 stored it.
- Apple dates are read by the 2.1.0 helper's rules: chrono for workout and
  sleep windows, JavaScript `Date.parse` for stored instants, each where 2.1.0
  used it.

## Open work

- The command-line changes chosen for 2.1.1 are listed in `cli-changes.md`.
