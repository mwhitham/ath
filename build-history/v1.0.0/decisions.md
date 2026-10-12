# Athletic Standard v1.0.0 — Decision Log

Decisions for this version continue after D81 in
[v0.4.0](../v0.4.0/decisions.md). Earlier decision files stay unchanged. A decision
below says which earlier choice it supersedes and which part remains in force.

## D82. SQLite is the live local record; JSON is the complete portable package

The live record is one `athlete.ath.sqlite` database. The CLI reads and writes that
database directly. JSON remains a complete import and export format, but an exported
package changes the database only after an explicit import.

This supersedes D4's no-database rule. It also supersedes D25, D36, D40, and D43 for
live dense-series storage. Samples live in SQLite rather than daily sidecar files.
An exported JSON package still uses generated series files so another implementation
can inspect and validate it without SQLite.

The local-first part of D4 remains. The database needs no server, account, network
connection, or subscription. D5's hosted ingestion work remains outside the core
package.

Rejected alternatives:

- **Keep JSON as the live record and use SQLite as a cache.** Rejected because two
  writable copies can disagree, and every write would need a recovery rule for both.
- **Make SQLite an optional second live format.** Rejected because every command and
  test would need two storage implementations with identical transaction behavior.
- **Move the live record to a hosted database.** Rejected because offline local use is
  a required property of the open package.

## D83. Zod describes the records; SQLite and JSON encode the same records

The Zod domain schemas remain the public definition of athlete-owned records. SQL
tables enforce storage relationships. The JSON schemas define the portable package.
Neither encoding adds athlete-owned fields that the other cannot represent.

The JSON package includes current records, retained revisions, tombstones, source
facts, provider periods, plans, saved agent work, and permitted attachments. It
excludes keys, connector grants, model preferences, unsaved backtest reports,
accounts, billing, and service operations.

Rejected alternatives:

- **Make the SQL schema the only public definition.** Rejected because JSON import,
  export, and package validation still need a format independent of one database
  engine.
- **Export only current values.** Rejected because revision and deletion history is
  athlete-owned data and is needed for restore, synchronization, and historical
  evidence.
- **Put operational settings into the package.** Rejected because a model preference
  or gateway key is not part of the athlete record.

## D84. New records use UUIDv7; legacy records use deterministic UUIDv5

Every athlete-owned record has a stable ID and a per-record revision. New records use
UUIDv7. Its time-ordered prefix improves SQLite index locality. It is not used as the
record's event time.

Legacy v0.4.3 records use UUIDv5 with an Athletic Standard namespace. The name input
is the complete old reference listed in the migration appendix. If two old records
share that reference, the first keeps the base UUID and each later collision adds its
original position to the UUIDv5 input. The same legacy package therefore produces the
same IDs on every import.

Links name record IDs. Evidence names both the ID and revision. A correction creates
a revision and keeps the ID.

This supersedes D48 and D64 where they reject record IDs and use `(source,start)` or
`(benchmark,recorded_at)` as links. Those pairs remain migration inputs, not live
v1 links.

Rejected alternatives:

- **Keep compound links.** Rejected because a correction can change one part of the
  compound value and break every reference to it.
- **Use content hashes as IDs.** Rejected because correcting content would change the
  ID.
- **Use UUIDv4 for new records.** It would be stable, but UUIDv7 gives better insertion
  order for large SQLite indexes.
- **Use an array position to separate old duplicates.** Rejected as the whole ID.
  Position is used only when the old fields cannot distinguish two records.

## D85. Measured, athlete-reported, and authored records stay separate

Measured records require a source foreign key. Athlete-reported records have no
source column. Authored records name who wrote the claim and the evidence used.

A manual source marks a hand-entered measurement or result. It never makes that row
device-measured and never enters a device baseline. An athlete's meal description,
photo, feeling, or note remains athlete-reported even when a model interprets it.
The interpretation is a linked authored record.

This keeps D2 and the two-tier wall. The new authored class prevents plans,
predictions, reviews, and model interpretations from being forced into either
measurement or self-report.

Rejected alternatives:

- **One signal table with a nullable source.** Rejected because a soft record could
  then claim device provenance by mistake.
- **Treat every model output as an athlete report.** Rejected because the model, not
  the athlete, wrote that claim.
- **Treat manual measurements as device evidence.** Rejected because the value was
  transcribed, not received from the device route.

## D86. Plans and saved agent work are records, but the CLI is not a coach

Training plans, planned sessions, planned blocks, goals, coaching notes, saved
reviews, and nutrition interpretations are athlete-owned records. They round-trip
through JSON and keep their author and evidence.

No earlier decision bans plan storage. D3 limits the reference agent to
benchmark-anchored prediction. It does not limit which athlete-owned records the
format can preserve. This decision therefore expands the format without superseding
D3. Athletic Standard still does not prescribe training, diagnose a condition, or
act without the athlete's confirmation. D41's decision not to build a skill registry
also remains.

A planned session and a completed workout are different records. Completion may link
them. It never turns the plan itself into measured history.

Rejected alternatives:

- **Keep plans in agent chat only.** Rejected because the athlete cannot export,
  revise, or inspect them later.
- **Store plans as workout sessions.** Rejected because intended work is not evidence
  that the work happened.
- **Generate plans automatically in the CLI.** Rejected because storage does not
  authorize coaching.

## D87. Store source summaries and complete samples; calculate cross-record answers

Every accepted sample is stored. Every summary or score reported by a source is also
stored. A provider summary does not replace its supporting samples.

Daily totals, baselines, trends, slopes, coverage, source differences,
correlations, workout drift, and segment comparisons are calculated when requested.
The calculation returns its method, version, inputs, exclusions, coverage, and
result. It does not write a rolling cache into athlete data.

Calculations remain separate by quantity and source unless the named operation is a
source comparison. SDNN and RMSSD never share a calculation. Temperature and
activity-specific distance types also remain separate.

Rejected alternatives:

- **Keep only provider summaries.** Rejected because they cannot answer a later
  question about a workout segment or a different time window.
- **Keep only raw samples.** Rejected because a provider-reported score or nightly
  summary is itself part of the athlete's source record.
- **Persist every daily and monthly rollup.** Rejected because those values are
  reproducible and would need correction whenever an input revision changes.

## D88. Source periods and calendar calculations are different time questions

An absolute instant, a provider-assigned period, and a calendar date are stored as
different facts.

WHOOP defines physiological cycles rather than calendar days. Oura defines separate
Activity, Sleep, and Calendar Days. Apple activity summaries carry a user-perceived
date. Those source periods and dates are preserved with their kind. A date-only
record does not receive an invented midnight timestamp.

Calendar calculations use an explicit IANA timezone such as `America/Chicago`. The
athlete profile may provide the default. That default is a chosen calculation rule,
not proof that every historical event happened there. Without an explicit or saved
timezone, a calendar calculation refuses rather than silently using UTC or the
computer's timezone.

Original timestamp and offset text is retained. An offset is called a recording
offset only when the route documents that meaning. Apple does not document the
timezone meaning of Health export XML offsets, so the importer preserves them
without claiming they identify the recording location or local day.

Supporting research:
[appendix/time-and-provider-periods.md](appendix/time-and-provider-periods.md).

Rejected alternatives:

- **Use the first ten timestamp characters as the local day.** Rejected because a
  rendered offset may not describe where the measurement happened.
- **Use the athlete profile timezone for every stored source date.** Rejected because
  travel and provider-specific periods would be rewritten as something the source
  did not report.
- **Use UTC dates for everything.** Rejected because a UTC day is not the athlete's
  calendar day in most timezones.
- **Call every provider period a day.** Rejected because WHOOP cycles and Oura's
  different day types have different boundaries.

## D89. Keep `ath log` as the general creation command

`ath log` remains the general command for creating a record and the only command for
entering a benchmark result. Its text path keeps the current common workflow. Its
structured path accepts every domain record, including nutrition, plans, goals,
reviews, predictions, and miss analyses. Attachments enter through `log` or
`correct`, where they can be checked and linked to their owner.

D72's structured-array exception remains. A confirmed JSON array may create several
independently validated records in one transaction. The preview lists every record.

D76's narrow exception remains. Bare `ath predict` may create the prediction that
its chosen model produced after preview and confirmation. A harness still creates
its prediction through structured `ath log`.

Specialized commands may create the tool-authored record that is their direct
result. `ath grade` may create a grade from an existing prediction and result.
`ath delete` may create a tombstone. They do not accept a general caller-supplied
record.

SQLite removes the ability to inspect or correct a live record with a text editor.
The CLI therefore adds:

- `ath records list` and `ath records show` to find IDs and inspect revisions.
- `ath correct` to create a revision of an existing record.
- `ath calculate` to run the named operations from D87.
- `ath export`, `ath conflicts`, `ath delete`, and `ath erase` for the operations
  already required by the storage design.

`ath stats` remains the fixed overview. `ath series` remains the dense-sample view.
`ath predict --json` remains the evidence path for an agent. Separate create
commands for plans, goals, reviews, nutrition, attachments, migration, and backup
are not added.

`ath link` previews a link change and, after confirmation, creates a new revision of
the record that owns the link. It does not update a typed row without revision
history.

`--json` never confirms a write. `ath predict --json` retains D76's evidence response
for agent harnesses.

Rejected alternatives:

- **Add `ath record create`.** Rejected because it duplicates `ath log`.
- **Add one command for every table.** Rejected because an agent already supplies
  typed JSON and the domain schemas validate it.
- **Stretch `stats` into arbitrary calculations and record lookup.** Rejected because
  its current job is a short fixed overview.
- **Allow SQL in the CLI.** Rejected because it bypasses domain validation, revisions,
  and the two-tier wall.

## D90. Logging a result performs an unambiguous grade; `ath grade` never logs

`ath log` is the only command that creates a benchmark result. If exactly one open
prediction matches, the same transaction also writes the versioned grade. The
preview shows both records. No match writes only the result. More than one possible
match writes no grade.

This narrows D72 for linked tool consequences. One plain-text `ath log` still accepts
one athlete entry, and D72's structured array remains available. A result transaction
may also create a linked tool-generated grade. Structured input cannot supply that
grade as a caller-authored record.

`ath grade <prediction-id> --result <record-id>` grades two existing records. It is
used for imported results, ambiguous matches, corrected results, and a new
calculation version. It never creates a result.

The old `ath grade <benchmark> --actual <score>` form is removed. A miss analysis is
a separate authored record written through structured `ath log`.

This supersedes D62 and D65 for the command that writes miss analysis and results.
Their validation and preview rules remain. `ath grade --analysis` is removed; the
structured `ath log` path performs D62's evidence checks before it accepts a miss
analysis. This also narrows D67's special case where `grade` could create or reuse a
result.

Rejected alternatives:

- **Keep two result-entry commands.** Rejected because the same athlete statement
  would have two commands with different side effects.
- **Remove `ath grade` entirely.** Rejected because existing imported or corrected
  records still need an explicit, repeatable grading operation.
- **Choose the latest open prediction when several could match.** Rejected because
  recency does not prove which claim the result tests.

## D91. Import previews; export is complete and byte-deterministic

`ath export` reads one database revision and writes a complete JSON package. With
the same revision and options, every exported byte is the same. Export time is
absent unless the caller supplies it.

`ath import` previews new, matching, changed, conflicting, unknown, and
unrepresentable records. `--yes` confirms only the non-conflicting plan. It never
chooses a conflict or discards an unknown field.

`ath conflicts show` displays both complete values. `ath conflicts resolve` records
an explicit choice of local, imported, or corrected value for that package import.
The final import applies all records and resolutions in one transaction.

An empty destination at database revision zero adopts the source database ID and
database revision from a complete Athletic Standard package. This initializes a
restore, so it does not add another revision. A merge into a nonempty database with
the same database ID increments the local database revision once.

Rejected alternatives:

- **Import immediately and report afterwards.** Rejected because a large package can
  make thousands of changes that cannot be checked before they land.
- **Use last-write-wins.** Rejected because clocks and receipt times do not establish
  which health record is correct.
- **Add export time by default.** Rejected because it would make two exports of the
  same revision differ for no athlete-data reason.

## D92. Delete keeps history; erase removes value-bearing history

`ath delete` writes a tombstone and leaves revisions available. It is the normal way
to remove a mistaken or unwanted current record.

`ath erase` is the privacy operation. It shows dependent records and attachments,
requires a second explicit confirmation, and removes current values, value-bearing
revisions, samples, attachments, and derived records that depend on the erased
record. It may retain only a minimal tombstone containing no health value, free
text, source detail, or attachment hash.

An erase cannot promise forensic removal from old exports, filesystem snapshots,
backups, SQLite journals, or storage-device wear leveling. The command and
documentation say that plainly.

Rejected alternatives:

- **Make delete remove revision history.** Rejected because an ordinary correction
  should remain auditable.
- **Call a tombstone privacy erasure.** Rejected because the sensitive values would
  still exist in revisions.
- **Cascade silently.** Rejected because the person must see every dependent record
  and attachment before an irreversible operation.

## D93. Use raw parameterized SQL through `@libsql/client`

The first database implementation uses the current stable `@libsql/client` if it
passes platform and performance tests. Local SQLite and a future hosted libSQL
adapter use the same domain operations.

Migrations are ordinary, parameterized SQL. There is no ORM. The database module
exposes record and query operations, not a general SQL handle to the CLI.

Local connections enable foreign keys, a five-second busy timeout, and WAL. The
format metadata table is authoritative for local and hosted adapters. Local header
pragmas provide additional checks only.

Rejected alternatives:

- **Add an ORM before the query patterns exist.** Rejected because it would add a
  second schema language before repeated code shows a need.
- **Use a local-only SQLite API.** Rejected because the planned hosted adapter would
  then need different transaction and query code.
- **Expose database statements to commands.** Rejected because commands should use
  the same validation and revision rules as package consumers.

## D94. Imports and athlete files are untrusted local input

Provider exports, JSON packages, archives, attachments, photos, and existing
databases are treated as untrusted. The implementation sets byte, file-count,
record-count, nesting, and expanded-archive limits. It streams large inputs and
rejects path traversal, absolute archive paths, links, mismatched file types, and
unsupported images.

Database files, attachment directories, temporary work, and exports use owner-only
permissions where the operating system supports them. The CLI refuses to write
health data through a symbolic link and warns before using an existing path readable
by other local users.

The database is not encrypted by `ath`. Documentation recommends operating-system
disk encryption and says that anyone who can read the files can read the athlete
record.

Rejected alternatives:

- **Trust local files because the user selected them.** Rejected because downloaded
  archives and photos can still be malformed or hostile.
- **Invent application-level encryption in this release.** Rejected because key
  storage and recovery would become a separate security system. The operating system
  already provides disk encryption and account permissions.

## D95. Measure large databases before setting release limits

Deterministic fixtures cover 100,000, 3 million, and 30 million records. Benchmarks
record hardware, operating system, database settings, seed, repetitions,
percentiles, cache method, query plans, database size, and memory.

The first measured run sets release limits. No latency claim is written before that
run. If the largest fixture fails, indexes, bounded queries, prepared batches, text
storage, chunking, and complete-sample compression are tested in that order before a
second database engine is considered.

Rejected alternatives:

- **Publish an estimated latency target.** Rejected because it would be a number with
  no measured database behind it.
- **Run 30 million records in every pull request.** Rejected because ordinary CI
  needs a fast deterministic fixture. Large runs belong in the release job.
- **Add another database immediately.** Rejected because query and storage changes
  must be tested first.

## D96. Release 1.0.0 directly after 0.4.3

The current published package and format are 0.4.3. There is no 0.5.0 or 0.6.0 build
to release first. The unfinished `ath share` work remains deferred.

Changing the live record from JSON plus sidecars to SQLite changes how every
athlete-data command reads and writes. The public version rule says a breaking change
requires a major version. The next format and npm package version is therefore
1.0.0, not 0.5.0 or 0.6.0.

Rejected alternatives:

- **Release the storage change as 0.5.0 or 0.6.0.** Rejected because minor versions
  may only add optional fields under the existing version rule.
- **Build `ath share` first.** Rejected because sharing is unrelated to the local
  storage migration and remains outside the approved public scope.
- **Backfill missing GitHub releases before work starts.** Rejected as a release
  bookkeeping task, not a dependency of the format change.

## D97. Provider import keys are separate from record IDs

A UUIDv7 says which Athletic Standard record this is. It does not say whether a row
in a later provider export is the same record. Each imported record therefore keeps
a versioned import key in `record_import_keys`.

The importer uses a provider record ID when the route supplies one. Otherwise it
builds a route-specific key from the record kind, source, source time, provider
dimensions, and a collision ordinal when the route supplies indistinguishable
duplicates. The ordinal follows the provider document's stable record order. The key
recipe is versioned and tested with route fixtures.

The same import key and same logical content is a match. The same key with changed
logical content is a correction and creates a revision. A UUID is assigned only
when no import key already resolves to a record.

Within a collision group, unchanged complete records are matched before ordinals are
considered. If a later export reorders the group and the remaining changed records
cannot be paired one-to-one, the import reports a conflict instead of assigning a
correction to an arbitrary record.

Rejected alternatives:

- **Use UUIDv7 alone.** Rejected because another import would generate another UUID
  for the same provider row.
- **Put every value into the import key.** Rejected because a provider correction
  would look like a new record instead of a revision.
- **Use one key recipe for every provider.** Rejected because routes expose different
  stable fields and have different duplicate cases.

## D98. Legacy historical queries use marked time assumptions

v0.4.3 records do not say when the file first learned them. Giving every migrated
revision the migration time would make all old data disappear from a historical
backtest.

Revision 1 therefore uses the old event, report, session, prediction, or result time
described in the migration appendix. It also records
`known_at_basis: legacy_event_time_assumption` or the more specific basis named
there. This preserves the old `--as-of` behavior without claiming the assumed time
was observed as an ingestion fact. Query receipts disclose the basis.

Rejected alternatives:

- **Use the migration time.** Rejected because a replay of any date before migration
  would see no migrated history.
- **Leave `known_at` null.** Rejected because an as-of query could not decide whether
  to include the revision.
- **Call the event time exact knowledge time.** Rejected because the old format never
  recorded when the row was first written.

## D99. A prediction outcome link is separate from its grades

A prediction outcome is the link between one prediction revision and one benchmark
result revision. It is an athlete-owned record in `prediction_outcomes`. A saved
grade points to an outcome rather than copying the result link.

This represents every valid v0.4.3 state. The old schema allowed `actual` to exist
without `grade`. It also lets the tool save a new calculation version without
rewriting which attempt the prediction concerned. Correcting the result creates a
new outcome revision; an older grade keeps its original outcome revision.

Logging an unambiguous result may create the outcome and its first grade in the same
transaction. `ath grade` may create a missing outcome before it writes the grade.
The outcome is an internal linked consequence, not another result-entry command.

This supersedes D64's copied `prediction.actual` value for live v1 records. D64's
rule that a prediction must resolve to the result it was graded against remains.
It also supersedes D90's two-record description. The result transaction previews and
writes the outcome before the grade when that link is new.

Rejected alternatives:

- **Store the result link only on a grade.** Rejected because v0.4.3 permits an
  actual result link without a grade.
- **Store the current result ID directly on the prediction.** Rejected because a
  corrected result would rewrite the prediction and could make an old grade appear
  to use a revision it never saw.
- **Copy the result value into every grade without a record link.** Rejected because
  the copied score could no longer be checked against the measured result.
