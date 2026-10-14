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

## D100. v1 storage names state what each row means

The v1 Zod and SQL names use the full record meaning. Legacy v0.4.3 field names stay
only at the import boundary.

The core tables and fields are:

- `database_metadata`, with `athleticstandard_version` for the live format version.
- `database_state.current_revision` for the database-wide revision.
- `records.record_id`, `record_type`, and `first_known_at` for every athlete-owned
  record.
- `record_revisions.became_known_at` for historical visibility and
  `previous_revision` for its prior revision.
- `record_deletions` for normal deletions.
- `record_erasure_markers` for the optional value-free marker left after privacy
  erasure removes the parent record.
- `record_import_keys.match_key_method` and `match_key` for repeat-import matching.
- `record_import_keys.input_athleticstandard_version` for the version read from an
  imported Athletic Standard document. It is null for provider files that are not
  Athletic Standard documents.

Measured storage uses `measurement_samples`, `legacy_sample_groups`,
`legacy_sample_files`, `point_measurement_calculations`,
`workout_session_segments`, and `provider_reporting_periods`. Standard values use
`standard_value` and `standard_unit`. Point-like records use
`input_timestamp_text`, `input_utc_offset_text`, and `input_timestamp_basis`.
Sessions use matching `input_start_timestamp_*` and `input_end_timestamp_*` fields.
Athlete reports use `input_reported_at_*`. Reconstructed samples use
`timestamp_base_text`, `input_utc_offset_text`, and
`timestamp_reconstruction_method`.

Athlete statements use `athlete_reports`, not `soft_signals`. Structured nutrition
uses `reported_nutrition_items` for the athlete's claims and
`nutrition_model_interpretations` for model output.

Prediction storage uses `prediction_result_links` for the link from a prediction to
a benchmark result. Its `created_by_*` fields name who created the link. Grades point
to a link revision.
`prediction_error_analyses` explains why a prediction differed from the result.
Its claimed causes use `prediction_error_causes`. Possible legacy matches use
`prediction_error_cause_record_candidates`.

These names supersede the proposed v1 names `format_metadata`,
`database_revision`, `record_tombstones`, `records.kind`,
`records.id`, `records.created_at`, `record_revisions.known_at`,
`record_revisions.supersedes_revision`, `record_import_keys.key_recipe`,
`record_import_keys.import_key`, `record_import_keys.source_format_version`,
`soft_signals`, `series_samples`, `series_receipts`, `series_sidecar_receipts`,
`measurement_derivations`, `session_segments`, `provider_periods`, `benchmarks`,
`nutrition_reported_items`, `planned_blocks`, `saved_reviews`,
`nutrition_interpretations`, `prediction_outcomes`, `miss_analyses`,
`miss_analysis_evidence`, and `miss_analysis_evidence_candidates`. Their final names
are defined above and in the v1 build plan. D82 through D99 remain in force for
behavior and data meaning.

Rejected alternatives:

- **Keep short internal names and explain them in documentation.** Rejected because
  the names appear in schemas, migrations, diagnostics, exports, and agent output.
- **Keep `source_format_version`.** Rejected because it does not say whether the
  value is the Athletic Standard version, provider format, package version, or SQL
  schema version.
- **Keep `prediction_outcomes`.** Rejected because the row contains a link to the
  result. The measured result remains in `benchmark_results`.
- **Keep `miss_analysis`.** Rejected because it does not say that the record explains
  a prediction error.

## D101. v1 does not import prerelease 0.x Athletic Standard files

The public v1 CLI refuses Athletic Standard documents whose
`athleticstandard_version` is below `1.0.0`. It explains that prerelease files are
unsupported. It does not convert them automatically or through a hidden best-effort
path.

No real athlete records depend on the 0.x format. Permanent compatibility tables,
IDs, time assumptions, and conflict rules are not justified for repository fixtures
that can be regenerated directly as v1 data.

The active v1 model therefore removes:

- Deterministic UUIDv5 IDs for 0.x records.
- `legacy_sample_groups` and `legacy_sample_files`.
- `prediction_error_cause_record_candidates`.
- `record_import_keys.input_athleticstandard_version`.
- Assumed historical `became_known_at` values.
- Conversion of old timestamp-based links and embedded prediction fields.
- The `unknown` author type that existed only for unrecoverable legacy authors.
- The field-by-field v0.4.3 migration gate and its release tests.

New records use UUIDv7. A v1 package restore preserves the stable IDs, revisions, and
times already present in that package. A provider import uses
`record_import_keys` to find an earlier provider row and assigns UUIDv7 only when no
match exists.

A complete v1 package includes provider import keys and value-free erasure markers.
This allows a restored database to recognize a later provider correction and resolve
references to erased records.

The v1 package, manifest, and generated measurement-series files continue to use
`athleticstandard_version`. The live database continues to use
`database_metadata.athleticstandard_version`. Provider files have their own
route-specific formats and do not create an Athletic Standard version field per
record.

`records.created_at` keeps its ordinary meaning: when the Athletic Standard record
was created. `record_revisions.became_known_at` records when each revision became
available. New records normally use the same instant for both. Restored v1 packages
preserve both values.

The 0.4.3 npm package remains available for its released behavior. The repository's
0.x fixtures are development data and are regenerated as v1 fixtures instead of
migrated.

This supersedes D84's UUIDv5 rule, D98's legacy time assumptions, and the
legacy-only parts of D99 and D100. Their rules for UUIDv7, v1 import and export,
prediction-result links, and clear current names remain.
D101 also supersedes D100's `records.first_known_at` name and its rejection of
`records.created_at`.

Rejected alternatives:

- **Ship the 0.x converter in case somebody needs it.** Rejected because it adds
  permanent schema and test work without an athlete record to preserve.
- **Use a best-effort importer.** Rejected because silently losing samples, links, or
  source details is worse than a clear refusal.
- **Keep the migration appendix as an implementation gate.** Rejected because the
  appendix now documents a design that v1 will not implement.

## D102. The portable package uses typed record envelopes and separate sample files

Each current record in portable JSON has `record_id`, `record_type`, `created_at`,
`current_revision`, `is_deleted`, and a typed `data` object. Each retained revision
has the same `record_type` and complete typed `data`, plus its revision number,
previous revision, knowledge time, knowledge-time basis, and change author.

`athlete.ath.json` contains every current record and retained revision except dense
`measurement_sample` records. It also contains deletion records, erasure markers,
provider import keys, and references to generated measurement-series files.

Generated measurement-series files contain the current sample records and their
revisions, deletion records, and provider import keys. Each file covers one source,
quantity, and bounded time range. This keeps the main document inspectable without
dropping stable sample IDs or sample history. Erasure markers remain in
`athlete.ath.json` because privacy erasure may remove the source and quantity needed
to place a marker in one series file.

Authored record data uses one shared `authorship` object. It names the author,
authoring time, evidence, content version, and status. Plan records use plain fields
for their title, objective, dates, session blocks, completion state, and optional
link to a completed workout. These fields store a plan; they do not authorize the
CLI or agent to create one without athlete confirmation.

Provider reporting periods use these distinct names:

- `whoop_physiological_cycle`.
- `oura_activity_day`.
- `oura_sleep_day`.
- `oura_calendar_day`.
- `oura_day_unspecified`.
- `apple_activity_summary_day`.

The Zod schemas define the complete field rules. SQL may flatten nested JSON objects
into named columns and child tables, but it must reconstruct the same typed `data`
object without loss.

Rejected alternatives:

- **Put every sample in `athlete.ath.json`.** Rejected because dense streams would
  make the main document too large to inspect while duplicating the purpose of the
  generated series files.
- **Export compact value arrays without record IDs or revisions.** Rejected because
  the package could not restore corrections, deletion history, or provider import
  matching.
- **Mirror SQL tables directly in JSON.** Rejected because child-table layout is a
  storage choice. The portable format should describe complete logical records.

## D103. Normalized facts and package history must be exact

Normalized instants use UTC text ending in `Z`. Input timestamp text and numeric
offset text remain separate source receipts. Each receipt says what the route
documents that offset to mean. An available source value, unit, timestamp, or offset
cannot also carry a missing reason.

A complete package includes every retained revision through each current record's
`current_revision`. The current typed data must equal that final revision. Package
validation refuses missing revisions, broken revision chains, duplicate records,
or provider import keys that point to another record.

The main document owns deletion records and provider import keys for non-sample
records. A measurement-series file owns them for the sample records in that file.
A current sample is placed by its current source, quantity, and time. All retained
revisions of that sample stay with it even when an older revision had another
source, quantity, or time. An erased sample has no value-bearing record left, so its
value-free erasure marker stays in the main document.

Privacy erasure markers use a fixed `reason_code`. They do not accept a free-text
reason. Measured records do not accept athlete notes. A source-provided label or
summary must use its named source field; an athlete comment is a separate
source-free athlete report.

Revision changes, deletions, and links name the actor that performed the action.
They do not require a calculation version. A tool-authored calculated record names
the tool version and calculation version as part of its authorship.

All authored records store ordered evidence through
`authored_record_evidence(authored_record_id, evidence_order,
evidence_record_id, evidence_revision)`. This supersedes the prediction-only
`prediction_evidence` table name. Prediction error causes continue to carry their
own exact evidence reference because the reference is part of each claimed cause.

The planned session owns its optional `completed_workout_session_id` link. A workout
session does not store a second link back to the plan, so the two records cannot
disagree.

Rejected alternatives:

- **Allow offsets on normalized timestamps.** Rejected because SQLite text ordering
  would not reliably match instant ordering.
- **Let a package omit old revisions.** Rejected because restore could not reproduce
  historical evidence or knowledge-time queries.
- **Store free-text erasure reasons.** Rejected because an erasure marker must not
  retain sensitive text.
- **Require calculation metadata for every tool action.** Rejected because deleting
  or linking a record is an action, not an arithmetic result.

## D104. The database handle is opaque and every logical link has a foreign key

The public database handle does not expose a SQL client or an arbitrary transaction
callback. Public callers use domain operations such as `writeRecord`,
`correctRecord`, and the named queries. Internal migration and transaction helpers
remain package-private.

SQL stores every logical record link that integrity and privacy erasure need to
follow. `record_attachment_links` connects athlete reports and nutrition entries to
attachments. `point_measurement_calculation_inputs` connects a calculated
measurement to the exact input revisions. Goals use a foreign key for an optional
benchmark target. Prediction links and grades reference both the expected typed
table and the exact retained revision.

A correction's `became_known_at` cannot precede the prior revision's knowledge time.
A prediction-result link must join records for the same benchmark definition. A
saved grade must copy the scores from its exact linked revisions, and its stored
arithmetic must match the calculation version's deterministic method.

A new local database file is created exclusively with owner-only permissions before
SQLite enables WAL. WAL and shared-memory files also receive owner-only permissions.
The local client applies the five-second timeout to every pooled connection.
Foreign-key enforcement is checked inside every write transaction.

An existing local file is inspected before WAL is enabled. Another nonzero
application ID is refused without changing the file's journal mode. Opening warns
when the database, WAL, shared-memory file, or database directory is readable by
other local users. A database path in a directory writable by other local users is
refused.

Rejected alternatives:

- **Expose the libSQL client for advanced callers.** Rejected because direct SQL can
  bypass revisions, validation, and the two-tier wall.
- **Keep links only in `data_json`.** Rejected because foreign-key checks and privacy
  erasure could not find every dependent record.
- **Enable WAL before checking the file.** Rejected because a refused foreign
  database would be modified merely by inspection.

## D105. Normalized UTC timestamps use fixed millisecond precision

Every normalized instant uses `YYYY-MM-DDTHH:mm:ss.SSSZ`. Fixed-width UTC text sorts
in instant order in SQLite and keeps indexed range queries correct. Input timestamp
text remains unchanged in its receipt, including any finer precision or numeric
offset supplied by the route.

Import rounds an absolute source instant to the nearest millisecond only for the
normalized field. It does not rewrite the input receipt. When two source events land
in the same normalized millisecond, their stable record IDs and provider import keys
still keep them separate.

Rejected alternatives:

- **Allow optional fractional seconds.** Rejected because text ordering places
  `00.500Z` before `00Z`, which breaks range and historical queries.
- **Use SQLite date functions in every query.** Rejected because wrapping each
  timestamp column would prevent the existing bounded indexes from serving ordinary
  range queries.

## D106. Empty initialization is an explicit restore destination

`ath init --empty` creates a valid database at database revision zero without an
athlete profile, sources, or benchmark definitions. A complete v1 package can restore
into that database and adopt the package database ID and exact database revision.

Normal `ath init` continues to create the athlete profile, manual source, and standard
benchmark definitions. It is not empty and therefore cannot adopt another database
ID.

Rejected alternatives:

- **Treat a normal initialized database as empty.** Rejected because it already owns
  records and a database revision.
- **Delete seeded records during restore.** Rejected because import must not silently
  remove local records.

## D107. Privacy erasure follows every stored dependency

`ath erase` removes the named record and every current record whose stored link
depends on it. This includes source-owned measurements, exact evidence references,
prediction-result work, plan completion links, model interpretations, and
attachments. The preview lists the complete set before any write.

An attachment and every record that references it form one erasure set. This prevents
the command from deleting an attachment file while leaving another current record
that still claims it exists.

The write requires the expected record revision and the unchanged database revision
from the preview. Non-interactive use requires both `--yes` and
`--confirm-erasure <record-id>`. Interactive use asks for confirmation and then
requires the exact record ID. The transaction removes all current rows and
value-bearing revisions, then leaves one value-free erasure marker per removed
stable ID.

Local attachment files are renamed out of their live names before the database
transaction. A refused transaction restores those names. A successful transaction
removes the renamed files.

Rejected alternatives:

- **Erase only the named row.** Rejected because linked records would retain values
  derived from it or would point at data that no longer exists.
- **Keep attachment files after erasing their records.** Rejected because the erased
  health data would remain readable on disk.
- **Let `--yes` perform erasure by itself.** Rejected because normal write
  confirmation is not strong enough for an irreversible action.

## D108. Each provider route has a versioned matching recipe

Provider imports use `apple_health/v1`, `whoop_csv/v1`, or `oura_csv/v1` as
`record_import_keys.match_key_method`.

The matching fields are:

- A source uses its writer, route, and sensor type.
- A source device uses its source ID, name, manufacturer, model, and hardware.
- A point measurement uses its source ID, quantity, and source time. An Oura
  date-only row uses its provider date instead of an invented instant.
- A sleep or workout session uses its source ID and start time.
- A provider score uses its source ID, metric, and source time or provider date.
- A dense sample uses its source ID, quantity, and normalized sample instant.
- An athlete report or nutrition entry uses its type and reported time.

When those fields do not distinguish two rows, the importer appends their order in
the provider document. An unchanged repeat resolves to the existing records. If one
of those otherwise indistinguishable rows changes, the import reports a conflict
instead of guessing which existing record received the correction.

Provider parsers continue to stream or bound their input as before. Their output is
mapped into v1 records before any database write. One confirmed import writes new
records, corrections, and matching keys in one transaction.

Rejected alternatives:

- **Use normalized values in matching keys.** Rejected because a provider correction
  would create a second record instead of a revision.
- **Use one matching recipe across all routes.** Rejected because Oura date rows,
  WHOOP cycles, and Apple sample streams do not expose the same fields.
- **Assign changed duplicate rows by their current order.** Rejected because provider
  export order can change independently of record meaning.

## D109. Runtime arithmetic uses named, source-separated operations

`ath calculate` accepts `summary`, `baseline`, `trend`, `coverage`,
`compare-sources`, `correlation`, and `workout-drift`. It does not accept SQL.

Summary and baseline results are separate for each source. A source comparison needs
two named source IDs. Correlation pairs same-calendar-day means and names the IANA
timezone. Coverage uses calendar dates and also requires an explicit or saved IANA
timezone. Workout drift compares equal-duration halves of one named workout session.

Every result names the calculation version, method, quantity, unit, sources, range,
timezone when used, input record revisions, and exclusions. A receipt lists at most
1,000 record references and states when that list is shortened. Arithmetic refuses
more than 100,000 input records until the caller narrows the time range. It never
returns a result calculated from a silently shortened input.

Manual sources are excluded. One request uses one quantity, so RMSSD and SDNN,
distinct temperature quantities, and activity-specific distance quantities cannot
be combined by accident.

Rejected alternatives:

- **Expose arbitrary SQL.** Rejected because it can bypass source separation and the
  two-tier wall.
- **Pool sources for a larger sample.** Rejected because two devices measuring one
  quantity provide two answers.
- **Silently stop at an input limit.** Rejected because a partial baseline would look
  complete.

## D110. Provider summaries are records separate from scores and raw samples

`provider_summary` stores a total, duration, or other summary reported by a source
when it does not belong to one sleep or workout session. It requires a source and
keeps the source value, normalized value, provider period, and source time facts.
Its provider-import key uses the source ID, summary type, and source time or provider
date. This extends D108 without changing its existing matching rules.

This closes a gap in D87. Apple activity summaries are the first required example.
Active energy, exercise time, and stand hours are summaries for an Apple activity
day. They are not raw samples and they are not scores.

Rejected alternatives:

- **Store summaries as `provider_score`.** Rejected because a duration or energy
  total is not a composite score and has no score scale.
- **Store summaries as point measurements.** Rejected because a day total is not a
  measurement taken at one instant.
- **Recalculate and discard the source summary.** Rejected because the source's
  reported answer is part of the athlete record even when raw samples also exist.

## D111. Provider-period arithmetic is an explicit calculation

`ath calculate provider-period-summary` groups values by the exact provider period
stored on each record. It requires `--period-type`. It does not turn an Oura day,
WHOOP cycle, or Apple activity-summary day into a calendar day.

The command accepts `--record-kind` so a measurement, provider score, provider
summary, and summary nested in a session cannot be mixed because they share a name.
The default kind is `measurement`. Provider scores and provider summaries that carry
an absolute instant may also use the existing summary, baseline, trend, comparison,
and correlation operations. Date-only values use the provider-period operation.

Results stay separate by source and provider period. Each result names the period
type, complete stored period, record revisions, unit, exclusions, and arithmetic
method. `--from` and `--to` compare the provider date when one exists. Otherwise they
compare the provider period start instant. No IANA timezone is needed because this
operation does not choose calendar boundaries.

Rejected alternatives:

- **Treat a provider date as UTC midnight.** Rejected because the source supplied a
  date or physiological period, not that instant.
- **Include provider scores and summaries in measurement calculations by name.**
  Rejected because a score, a reported total, and a sample can use similar names
  while answering different questions.
- **Make provider periods a calendar grouping option.** Rejected because that would
  make two different time rules look interchangeable.

## D112. A changed row in an indistinguishable provider group is a conflict

This decision supersedes the collision-group matching rule in D97. It confirms the
later rule in D108.

An unchanged repeated provider export still matches each row by its route key and
collision position. If any row changes inside a group whose matching fields are
otherwise identical, the import reports a conflict. Document order cannot prove
which existing record received the correction. A collision position is safe for an
unchanged repeat, but it is not evidence about row history after content changes.

Rejected alternatives:

- **Apply the correction to the same collision position.** Rejected because a
  provider may reorder equal rows between exports.
- **Choose whichever old row leaves the most unchanged matches.** Rejected because
  identical old rows can still leave more than one equally valid history.

## D113. A package may contain 100 million record and history objects

The package cap is 100 million current-record, revision, deletion, and import-key
objects. The 30-million-record release fixture needs at least 60 million objects
because each current record has a first revision. The remaining room covers normal
corrections, deletions, and import keys without removing a hard refusal limit.

Each JSON file remains limited to 512 MiB. Dense samples remain split into bounded
series files and are read one file at a time. The larger object count does not allow
one unbounded JSON document.

Rejected alternatives:

- **Keep the 50-million-object cap.** Rejected because a valid 30-million-record
  database could not complete the required export and restore release test.
- **Remove the object cap.** Rejected because package input remains untrusted.

## D114. Require measured release runs at 100,000 and 3 million records

This decision supersedes the required 30-million-record run in D95 and section 13
of the v1.0.0 plan.

Normal release CI measures 100,000 records. The separate large release job measures
3 million records. The measured 3-million-record fixture is 5.6 GB, which already
exceeds a 1 GB real record covering 15 years. A complete 30-million-record fixture
is 58 GB and mostly measures hours of full-copy disk work during export, check, and
restore.

Large-data failures remain covered by focused tests. Workout calculations must apply
session time bounds before their input limit. Historical evidence queries must select
only the record types they use and must use the record-type index. The generator may
still create larger databases for optional investigation, but they are not a release
gate.

The 100-million-object package refusal limit from D113 remains unchanged. It is an
input safety limit, not a required fixture size.

Rejected alternatives:

- **Keep 30 million records as a release gate.** Rejected because it is far beyond
  the expected local record and makes release proof depend mainly on disk-copy time.
- **Test only 100,000 records.** Rejected because the 3-million-record run is large
  enough to expose unsafe query bounds and remains practical as a separate release
  job.

## D115. Split internal workspace packages before publishing them

The repository first splits pure logic, storage, and importers into private workspace
packages behind the existing `athleticstandard` package. The root package bundles
their code and keeps the current package name, CLI, and supported entry points.
Scoped `@ath/*` packages are published only after a concrete need and packed-package
tests prove they work outside the workspace.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Publish every workspace package immediately.** Rejected because it creates more
  versioned public APIs before their boundaries have been exercised.
- **Keep one source package forever.** Rejected because pure logic, storage, and
  importers need enforceable internal boundaries.

## D116. Direct third-party SQLite writes are supported and SQLite enforces critical rules

Builders may write Athletic Standard databases directly with SQLite. They do not
need the Node library, CLI, a local service, or a maintained language binding.
Every client must enable `PRAGMA foreign_keys = ON`.

Critical correctness rules also use database constraints and triggers. The schema
uses `STRICT` tables, `CHECK`, `NOT NULL`, `UNIQUE`, foreign keys, and SQLite JSON
functions. Triggers enforce the two-tier wall, record and revision agreement,
revision and deletion rules, current-revision pointers, and protected links even
when application code is bypassed. Conformance tests execute direct SQL rather than
assuming the Node operations are the only writer.

D104 still keeps the JavaScript database handle opaque and keeps arbitrary SQL out
of the JavaScript API. This decision supersedes any reading of D104 that direct SQL
writes by another implementation are unsupported. It also strengthens D83: SQLite
is a public encoding with enforced write rules, while Zod remains the logical record
definition.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Require every writer to use the Node package.** Rejected because the SQLite file
  is the cross-language foundation.
- **Document rules without enforcing them in SQLite.** Rejected because another
  writer could then create a file the official reader cannot safely interpret.
- **Rely on foreign keys alone.** Rejected because foreign-key enforcement is a
  per-connection setting and does not express every critical cross-table rule.

## D117. Official runtimes execute only reviewed code compiled into releases

Official Athletic Standard and the hosted product do not load third-party importer
or calculation code at runtime. Code approved for official use is reviewed, tested,
and compiled into a release. Forks may choose another loading model.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Load code from the community catalog.** Rejected because catalog metadata and
  compatibility tests are not a runtime trust boundary.
- **Forbid plugin loading in forks.** Rejected because forks control their own
  runtime and risk policy.

## D118. v1 records have a namespaced extension envelope

Every v1 record may carry an optional, namespaced, size-limited `extensions`
envelope. Extension values inherit the parent record's measured,
athlete-reported, or authored class. An extension cannot add a source to an athlete
report or remove the source requirement from measured data.

All v1 readers, SQLite writes, JSON import, and JSON export preserve extension
values. A minimal pass-through implementation is sufficient for v1.0.0. It must land
before v1.0.0 is published to npm; npm still publishes 0.4.3. This partially
supersedes D91 for the defined envelope: unknown namespaced values inside a valid
envelope are preserved rather than discarded. D91's unknown-field refusal remains
in force outside the envelope.

Separate linked extension record types may be added later without replacing the
envelope. An extension becomes an official field only after repeated use, stable
meaning across builders, clear tier rules, and a separately approved format change.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Wait until after v1.0.0.** Rejected because strict v1 readers would otherwise
  need a format change before builders have any safe extension point.
- **Allow arbitrary unknown top-level fields.** Rejected because strict validation
  would no longer distinguish an extension from a typo or unsupported format.
- **Start with separate linked extension records only.** Rejected because a
  pass-through envelope is the smaller useful v1 foundation.

## D119. Community catalog inclusion means tests passed, not endorsement

An entry appears in `community-catalog.json` only after its manifest and synthetic
contract tests pass. Inclusion does not mean the project recommends its training
advice, accepts medical claims, approves its security for every use, or approves it
for the hosted product. Hosted promotion is a separate review.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Treat catalog inclusion as project approval.** Rejected because compatibility
  checks cannot establish coaching quality or suitability for every athlete.
- **Promote every catalog entry into the hosted product.** Rejected because hosted
  use has a separate risk and product review.

## D120. Named local community Skills may be installed explicitly

A community Skill may be installed only when the person or agent names it. The
official bundled Skill remains the default. Read commands never install or refresh
a Skill. The installer never fetches a remote Skill and never installs the whole
catalog.

This supersedes D41's decision not to build community distribution. D41's concern
about implied endorsement remains and is answered by D119. D79 remains in force for
the official bundled Skill and its current install locations.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Auto-install compatible Skills.** Rejected because a read or setup command
  should not silently add agent instructions.
- **Fetch Skills from catalog URLs.** Rejected because remote fetching adds a code
  and content supply chain that the local repository does not need.
- **Keep community installation entirely out of the CLI.** Rejected because an
  explicit local install removes repeated manual copying without implying approval.

## D121. The public foundation stays thin until a builder needs more

The v1 public foundation is the SQLite file, public specification, conformance
vectors, existing CLI, and official Skills. It does not add a local HTTP service, a
local MCP server, a maintained Swift library or example, a Rust core, or another
maintained language port now.

D7 remains in force: CLI plus Skill is the agent interface, and MCP is not added.
A future Rust core may ship inside the npm package behind existing APIs. Any future
service or port must start from a concrete builder need and pass the same conformance
vectors.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Build every likely client surface before demand.** Rejected because each surface
  adds maintenance without removing a demonstrated blocker.
- **Forbid future ports.** Rejected because shared vectors are intended to let
  builders add the smallest implementation they need.

## D122. Contributions use DCO sign-off, MIT, and no CLA

Every contribution requires Developer Certificate of Origin sign-off. The repository
keeps the MIT license and does not require a contributor license agreement.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Add a contributor license agreement.** Rejected because no identified legal
  need justifies a second contribution agreement.
- **Require no contribution attestation.** Rejected because DCO sign-off provides a
  lightweight record that contributors may submit their work.

## D123. Critical public contracts have explicit owners

`@mwhitham` owns `spec/`, `src/db/migrations/`, `src/v1/schema.ts`, `schema/`,
`.github/workflows/release-*.yml`, and `community/policy.json`. Other areas may be
reviewed by any maintainer and have no catch-all required owner.

This is a narrow exception to the repository rule against individual names. A
GitHub owner handle is allowed only in `CODEOWNERS` and in decision or plan text
that explains `CODEOWNERS`. The repository is personal and has no team handle, so
the required owner must be a real account. No other exception is created.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Require one owner for every file.** Rejected because it needlessly blocks normal
  maintenance outside the public contracts and release controls.
- **Use no protected owners.** Rejected because format, migration, schema, release,
  and community policy changes need explicit review.

## D124. Database migrations always preview, back up, and ask

Agents and scripts use explicit `ath migrate`: first preview, then confirm. An
interactive database open may show the same preview and ask the person to confirm.
A noninteractive open never migrates. Every route creates and verifies a SQLite-safe
backup before changing the database. A package update alone never rewrites a file.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Migrate silently on open.** Rejected because opening an athlete record should
  not make an unannounced irreversible storage change.
- **Require only manual file copying.** Rejected because copying an open WAL database
  is unsafe and does not prove the backup is usable.
- **Support only a separate migration command.** Rejected because an interactive
  person may reasonably confirm the same safe flow while opening the file.

## D125. Stable views are public reads and a small table set is public for writes

Versioned SQL views are the stable direct-read contract. A small documented set of
tables is the stable direct-write contract. The SQL specification names their
columns, required pragmas, transaction order, constraints, triggers, and version
policy. Every other table is an internal encoding detail and may change through an
ordered migration.

D104 still governs the named JavaScript domain API. This decision adds a separate
public SQL contract for non-JavaScript and direct SQLite builders.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Make every table stable.** Rejected because internal indexes, ledgers, and
  normalized encodings need room to change through migrations.
- **Publish example queries only.** Rejected because copied queries do not provide a
  versioned read contract.
- **Allow reads but no direct writes.** Rejected by D116; direct builders are a
  supported use.

## D126. Deprecated stable APIs remain until the next major release

A stable public TypeScript API may be deprecated with a warning and documented
replacement. It remains available for the rest of the current major version. The
next major npm release may remove it.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Remove stable APIs in a minor release.** Rejected because builders need a
  predictable migration boundary.
- **Keep every stable API forever.** Rejected because major releases exist to remove
  obsolete contracts after warning.

## D127. Approved community items ship in the main package first

A community Skill, importer, or calculation approved for official use ships in the
main `athleticstandard` package. The hosted product can adopt it by updating its
exact package pin after a separate hosted review. Separate public packages are
deferred until size, release, or dependency needs justify them.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Publish each approved item as a separate package now.** Rejected because it adds
  release and compatibility work before a concrete need.
- **Copy approved code into the hosted repository.** Rejected because the public
  exact-pinned package should remain the shared reviewed implementation.
- **Treat catalog inclusion as package approval.** Rejected because D119 separates
  passing public tests from official and hosted approval.

## D128. The core extension API uses semantic version 1.0.0

`CORE_API_VERSION` starts at `1.0.0` and follows semantic versioning. Community
manifests use `coreRange` to state compatibility with importer, calculation, Skill,
and extension interfaces. This version is independent of
`athleticstandard_version`, `sql_schema_version`, and unrelated CLI patch releases.

This follows the [contribution readiness plan](appendix/contribution-readiness.md).

Rejected alternatives:

- **Use the npm package version as the core interface version.** Rejected because CLI
  and packaging changes do not necessarily change extension compatibility.
- **Use an unversioned interface.** Rejected because community compatibility would
  become guesswork after the first interface change.
