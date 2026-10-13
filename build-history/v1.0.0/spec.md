# Athletic Standard v1.0.0 — SQLite Build Plan

Status: approved build plan. D101 removes the prerelease-file migration gate.
Runtime implementation begins with the v1 domain schemas.

The decisions for this version begin with D82 in `decisions.md`. They explicitly
supersede the earlier no-database and live-sidecar choices. They also replace the
compound links in D48 and D64 with stable UUID record links. No earlier decision
banned plan storage. Earlier decision files remain unchanged.

This version changes the live local record from JSON plus sidecar files to SQLite.
It keeps JSON as a complete import and export format.

The change is a major version because it changes how every command finds, reads, and
writes an athlete record. Athletic Standard `0.x` files are prerelease formats and
are not imported by v1.

## 1. Outcomes

v1.0.0 must provide:

1. One SQLite database for one athlete.
2. The existing two-tier wall as database structure and validation.
3. Stable IDs and revisions for every record that can be corrected or synchronized.
4. Storage for every accepted wearable sample and every summary reported by a source.
5. Runtime calculations over the recorded data.
6. Complete, deterministic JSON import and export.
7. The same CLI and Skill workflow where the storage change does not require a command
   change.
8. A database model that uses portable SQLite features and can be adapted to libSQL.
9. Honest date handling that keeps provider reporting periods separate from
    calendar days and never invents a timestamp for a date-only record.

The open package remains useful without an account, network connection, or hosted
service.

## 2. Scope boundary

This public build includes:

- The SQLite schema and migrations.
- The domain schemas that describe athlete-owned records.
- Local database discovery, opening, transactions, and checks.
- Importers for v1 Athletic Standard JSON packages and supported provider exports.
- Complete JSON export.
- CLI commands that read and write SQLite.
- Runtime calculations, evidence building, prediction, grading, and backtesting.
- The local Agent Skill and its generated command reference.
- Deterministic fixtures and performance tests.

This public build does not include:

- Accounts, billing, or cloud storage.
- A hosted OAuth broker.
- A remote MCP server.
- Continuous wearable synchronization.
- Mobile applications.
- Public athlete profiles.
- Messaging or external actions.

Those systems depend on the public data model but belong in a separate private
service.

## 3. One logical model, two encodings

The Zod domain schemas remain normative. They describe athlete-owned records without
depending on SQLite or JSON.

The same logical record has two supported encodings:

| Encoding | Purpose |
|---|---|
| SQLite | Live local record used by the CLI and local agents |
| JSON package | Portable import, export, inspection, and independent backup |

SQLite is the only live copy. Exported JSON does not update automatically. A changed
JSON package affects SQLite only after an explicit import with a preview and
confirmation.

The JSON package must contain every athlete-owned record. It excludes secrets,
connector grants, billing, and service operations.

## 4. Local file layout

The default live file is:

```text
athlete.ath.sqlite
```

Attachments remain ordinary sibling files:

```text
athlete.ath.sqlite
attachments/
  <opaque-id>.<extension>
```

Dense samples move into SQLite. The live record no longer needs `series/` sidecar
files.

An exported package contains:

```text
athlete.ath.json
series/
  <generated Athletic Standard series files>
attachments/
  <files referenced by exported records>
export-manifest.json
```

`export-manifest.json` records file hashes, record counts, the source database ID, and
the database revision. It records an export time only when the caller supplies one.
It contains no account or connector secrets.

## 5. SQLite database rules

Every local connection must run:

```sql
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
```

Local files also use WAL mode. A new file receives SQLite application ID
`0x41544831` (`ATH1`). `ath check` refuses a local SQLite file with another nonzero
application ID.

`database_metadata` is the sole metadata table. It records:

- `application_marker`.
- `sql_schema_version`.
- `athleticstandard_version`.
- `database_id`.
- `created_at`.

Local SQLite mirrors the SQL schema version into `PRAGMA user_version` as an
additional check. Hosted libSQL adapters must use `database_metadata`; they cannot
depend on header pragmas or local connection settings for format or migration
versions. Every local or hosted connection must still enable and verify foreign-key
enforcement before use.

Every write command uses a transaction. A failed import, correction, grade, or plan
write leaves the prior database unchanged. Do not copy an open WAL database as a
backup. Use `ath export` or SQLite's online backup operation. A closed, checkpointed
database may be copied.

## 6. Record classes

The database keeps three classes separate.

### 6.1 Measured records

Measured records require a source. The database enforces the source reference with a
foreign key. Only a device or verified import source counts as device-measured hard
evidence.

A hand-entered measurement or benchmark result uses a source whose
`sources.source_type` is `manual`, as required by D45. It is always labeled as
manually recorded. It never enters a device baseline or counts as device-measured
evidence. Feelings, journal answers, meals, and notes remain source-free athlete
reports.

Examples:

- Point measurements.
- Dense sample series.
- Sleep sessions.
- Workout sessions.
- Benchmark results.
- Provider summaries.
- Provider scores.

### 6.2 Athlete-reported records

Athlete-reported records have no source column. This keeps the two-tier wall
structural.

Examples:

- Sleep quality.
- Soreness.
- Stress.
- Mood.
- Energy.
- Meals and nutrition.
- Free notes.

A model may interpret text, voice, or a photo. The record remains athlete-reported
when the athlete supplied the underlying claim.

### 6.3 Authored records

Plans, predictions, reviews, and recommendations are claims written by a person or
agent. They are not measurements and they are not athlete reports.

Each authored record names:

- Its author type.
- The agent and model when applicable.
- Its creation time.
- The evidence references it used.
- Its version and current status.

Author types are `athlete`, `agent`, and `tool`. Agent and model are required only
for an agent author. Tool-generated records name the tool, tool version, and
calculation version.

## 7. Required database tables

The first SQL migration should use ordinary SQL without an ORM.

### 7.1 Database and athlete

```text
database_metadata
database_state
athlete_profile
records
record_revisions
record_deletions
record_erasure_markers
record_import_keys
```

`database_state.current_revision` is a monotonically increasing integer. Every
committed athlete change increments it once.

`records` is the parent for every athlete-owned record. It contains the stable record
ID in `record_id`, its `record_type`, `current_revision`, `created_at`, and
`is_deleted`. Typed tables use `record_id` as their primary key and foreign key.
Evidence references `(record_id, revision)` through this parent structure.

Ordered segments, point-measurement calculation details, nutrition items, planned
session blocks, and similar child rows are part of their parent record state. They
use the parent ID and an order or child key. They do not get independent record IDs
unless another record can link to or revise them independently. Correcting a child
creates a revision of its parent.

The stable `records.record_type` values are:

```text
athlete_profile
source
source_device
point_measurement
measurement_sample
sleep_session
workout_session
benchmark_result
provider_summary
provider_score
athlete_report
nutrition_entry
attachment
benchmark_definition
prediction
prediction_result_link
prediction_grade
prediction_error_analysis
training_plan
planned_session
coaching_note
saved_training_review
goal
nutrition_model_interpretation
```

`point_measurement_calculations`, `workout_session_segments`,
`benchmark_result_segments`,
`provider_reporting_periods`, `reported_nutrition_items`, `authored_record_evidence`,
`prediction_error_causes`, `planned_session_blocks`,
`point_measurement_calculation_inputs`, and `record_attachment_links` are child
rows. They do not have their own `record_type`.

Source devices are independently revisioned records. Their imported coverage windows
and descriptive details may be corrected without replacing the source that owns
them.

New records use UUIDv7. The time component improves index locality but is not an
event timestamp and is never used as one. Restoring a v1 package preserves the IDs
and revisions already present in the package.

`record_import_keys` maps a provider row to its stable record ID. Its main fields are
`record_id`, `match_key_method`, `match_key`, and `imported_at`. A provider record ID
is preferred when the route supplies one. Otherwise the route defines a versioned
key from stable source fields. UUIDv7 is never used to decide whether a later
provider import contains an existing record.

`athlete_profile` may hold an IANA timezone such as `America/Chicago`. It is the
athlete's chosen default for calendar calculations. It is not evidence that every
historical record happened in that timezone.

### 7.2 Sources

```text
sources
source_devices
```

A source is unique by `data_writer` and `data_route`. A named `sensor_type` also
splits the source
when its measurement method requires a separate baseline, such as Apple Watch optical
HRV and Apple Watch ECG beat timing. WHOOP through Apple Health and WHOOP CSV always
have different source IDs. Hardware model and serial details are descriptive. They do
not split a source when the data writer, route, and sensor type stay unchanged.

The source record retains:

- Data writer.
- Vendor.
- Device and sensor details when supplied.
- Data route, such as an import file or connector.

### 7.3 Measured data

```text
point_measurements
point_measurement_calculations
measurement_samples
sleep_sessions
workout_sessions
workout_session_segments
benchmark_results
benchmark_result_segments
provider_summaries
provider_scores
provider_reporting_periods
```

`provider_summaries` stores source-reported totals and durations that do not belong
to one sleep or workout session. `provider_scores` stores composite scores such as
WHOOP recovery or Oura readiness. D110 keeps these meanings separate.

`measurement_samples` stores every accepted sample. Its main index is:

```text
(quantity, source_id, recorded_at)
```

Measured child rows use explicit parent columns:

- `point_measurement_calculations.point_measurement_id`.
- `workout_session_segments.workout_session_id`.
- `benchmark_result_segments.benchmark_result_id`.
- `provider_reporting_periods.owner_record_id`.

Every measured row also carries:

- A stable record ID.
- Source ID.
- A normalized UTC instant or interval with fixed millisecond precision when the
  source supplies one.
- Every input timestamp and offset text exposed by the route.
- A provider-assigned date or period when the source supplies one.
- The documented meaning of each source time field when the route defines it.
- A missing reason when a time, offset, date, or period is unavailable.
- Original unit and value when the route exposes them.
- `standard_unit` and `standard_value`.
- Provider record ID when supplied.
- `received_at` and `received_at_basis`.
- Current revision.

A date-only source record remains date-only. Import does not invent midnight UTC or
any other instant. A numeric offset is preserved as source text. It is treated as a
recording offset only when the route documents that meaning.

Provider reporting periods are not renamed as calendar days. WHOOP physiological
cycles, Oura activity days, Oura sleep days, Oura calendar days, and Apple
activity-summary days retain distinct period types. A record may have both an
absolute instant and a provider reporting period.

Every revision stores the complete record state in `record_json`, when it
`became_known_at`, the `became_known_at_basis`, the actor that made the change, and
its `previous_revision`. The current typed row is updated in the same transaction.
An as-of query considers only revisions that became known by its cutoff. New local
and provider writes use the time the database learned the revision. A v1 package
restore preserves the recorded time and basis.
A correction cannot use a `became_known_at` earlier than its prior revision.

A row in `record_deletions` names `record_id`, `deleted_at`, the author, and the
deleted `record_revision`. `record_id` references `records.record_id`, and
`(record_id, record_revision)` references
`record_revisions(record_id, revision)`. The row hides the record from current
queries while preserving the historical event until the retention rule permits
removal.

Normal correction and deletion use revisions and deletion records. Privacy erasure
is a different operation. It removes the current typed row, all value-bearing
revisions, samples, attachments, and derived records that depend on the erased
record. It may also remove the parent `records` row. It may leave one row in
`record_erasure_markers` with only `record_id`, `erased_at`, and a fixed
`reason_code`. That table does not reference `records`, because the parent may no
longer exist. The marker contains no record type, health value, event time, free
text, source detail, revision, or attachment hash. An evidence reference checks
erasure markers after the parent lookup fails. It resolves as `erased`; it never
falls through to another record.

### 7.4 Athlete reports

```text
athlete_reports
nutrition_entries
reported_nutrition_items
attachments
```

`athlete_reports` and nutrition tables have no device source column.
`reported_nutrition_items.nutrition_entry_id` links each reported item to its
athlete-owned nutrition entry.

Structured nutrition must support:

- Original athlete description.
- Foods, amounts, and nutrients stated by the athlete.
- Attachment receipt for a photo.

Missing meals never mean zero intake.

The athlete's description, photo, and stated values remain the athlete report.
Model-inferred foods and nutrients are a linked authored record in
`nutrition_model_interpretations`. It names the model, model version, confidence,
evidence attachment or text, and creation time. Neither the report nor its
interpretation may carry a device source.
`nutrition_model_interpretations.nutrition_entry_id` links it to the athlete's
nutrition entry.

### 7.5 Benchmarks and predictions

```text
benchmark_definitions
predictions
authored_record_evidence
prediction_result_links
prediction_grades
prediction_error_analyses
prediction_error_causes
```

The prediction ledger remains a first-class part of the format. Evidence references
name stable record IDs and revisions.

`prediction_result_links` links one prediction revision to one benchmark-result
revision. It exists separately from a grade because the result may be linked before
arithmetic is saved, and the same link may be graded again with a newer calculation
version. Correcting the linked result creates a link revision. Its `created_by_*`
fields name who created the link without describing that person or tool as the
author of the prediction or measured result.

`prediction_error_causes` stores each claimed cause and its exact evidence reference.

`authored_record_evidence.authored_record_id`,
`prediction_error_causes.prediction_error_analysis_id` name their parent records.
The general evidence table stores ordered evidence for predictions, plans, reviews,
goals, coaching notes, grades, and nutrition interpretations.
Prediction-result links also reference the typed prediction and benchmark-result
tables. A saved grade is checked against the exact linked revisions and deterministic
grade arithmetic before it is written.

Grades are calculated at runtime. A saved grade is an authored record of what the
tool calculated at that time. It names the calculation version and inputs.

### 7.6 Plans and saved agent work

```text
training_plans
planned_sessions
planned_session_blocks
coaching_notes
saved_training_reviews
goals
nutrition_model_interpretations
```

Plans are athlete-owned data and must round-trip through JSON.
`planned_session_blocks.planned_session_id` links each block to its planned session.

A planned session remains separate from a completed workout session. Completion may
set `planned_sessions.completed_workout_session_id` without rewriting the plan as a
measured workout. The workout does not store a second link back to the plan.

## 8. Recorded summaries and runtime calculations

The database stores summaries supplied by a wearable or provider because those are
records from that source.

Examples:

- Oura nightly HRV.
- WHOOP recovery and strain.
- A provider-reported sleep duration.
- A source-reported workout average heart rate.

The database does not keep service-generated rolling tables merely to speed up normal
queries.

These values are calculated when requested:

- Daily, weekly, and monthly totals.
- Personal baselines.
- Trends and slopes.
- Coverage and missing days.
- Differences between sources.
- Correlations.
- Workout drift and segment comparisons.

The agent chooses the question, sources, dates, and level of detail. Deterministic
code performs arithmetic and returns the method, inputs, exclusions, and result.

Calendar calculations use an explicit IANA timezone. The athlete profile timezone is
the default when one is saved. Without either, a calendar calculation refuses and
asks for a timezone. It does not silently use UTC or the computer's current timezone.
The result names the timezone it used.

Provider-reporting-period calculations are separate operations. An Oura Activity Day may run
from 04:00 to 04:00, an Oura Sleep Day from 18:00 to 18:00, and a WHOOP cycle follows
WHOOP's physiological boundary rather than midnight. A provider reporting period is
never silently combined with a midnight-to-midnight calendar total.

The source facts remain unchanged when a caller asks for another timezone. The
calculation changes its calendar boundaries and records that choice in its receipt.
When a request asks for the recording-local day and the source did not provide enough
information, the result reports the unknown records instead of guessing.

Raw series remain available. A provider summary must not replace or hide the samples
that support a different question.

Calculations group by quantity and source. Multiple sources return separate results
unless the requested operation is a named source comparison. SDNN and RMSSD never
share a calculation. Distinct temperature types and activity-specific distance types
also remain separate.

A device baseline includes only sources that qualify as device-measured. A manual
source may be queried in its own labeled result, but it never enters a device
baseline or device-measured evidence.

An instant-bounded query needs no calendar timezone. A date-only bound on `series`,
`predict`, `backtest`, `stats`, or `calculate` uses the explicit or saved IANA
timezone before it becomes an instant range. A provider-reporting-period operation
uses the provider reporting period instead and says so.

## 9. JSON import and export

### 9.1 Export

Add:

```text
ath export <directory> [--json]
```

`--json` controls the machine-readable command result, as it does for other
commands. The exported package is already JSON. A future single-document stdout mode
would use a separate `--stdout` option.

The export process:

1. Opens one stable read transaction.
2. Captures the database revision.
3. Reads current athlete-owned records, retained revisions, deletion records,
   erasure markers, and provider import keys.
4. Writes deterministic JSON with stable ordering.
5. Writes generated series files and permitted attachments.
6. Validates the result against the public schemas.
7. Writes the manifest last.

The document and manifest have normative Zod schemas. A separate deterministic JSON
writer defines key order, number text, Unicode encoding, line endings, timestamp
serialization, revision records, deletion records, erasure markers, and provider
import keys. Zod validates values but does not define their byte encoding. Every
referenced attachment must be exported or the export fails.

The exported `athlete.ath.json` document, `export-manifest.json`, and every generated
Athletic Standard series file use the top-level field
`athleticstandard_version`. The live database stores its format version in
`database_metadata.athleticstandard_version`.

The manifest has an optional `exported_at`. It is absent by default. A caller may
supply a fixed value explicitly. Two exports from the same database revision and the
same explicit options are byte-identical. Golden byte fixtures enforce this.

### 9.2 Import

`ath import` detects an Athletic Standard JSON document, package, or supported
provider export.

Provider imports match their versioned `record_import_keys`. The `match_key` uses the
provider record ID when one exists. A route without one uses its documented source
time and provider dimensions, plus a deterministic collision position only for
otherwise indistinguishable rows. That position is the collision ordinal in the
provider document's stable record order. The same key and content is a match. The same key
with changed content creates a revision. A new key creates a UUIDv7 record.

Within a collision group, complete unchanged rows are matched first. If changed rows
cannot then be paired one-to-one, import reports a conflict. It does not assign a
provider correction by guess.

This replaces the 0.4.x provider merge rule. The old rule skipped a row when its
source and time key already existed and replaced a series group. The v1 preview
instead reports each row as matching, changed, or conflicting, and retains revisions
for corrections.

Before writing, it reports:

- New records.
- Matching records.
- Changed records.
- Conflicts.
- Records that cannot be represented.
- Unknown fields from a newer format.

The default is a preview. Confirmation or `--yes` applies the import in one
transaction.

Confirmation does not resolve a conflict. Import matches database ID, stable record
ID, and revision. It refuses unresolved conflicts and writes nothing. A separate
conflict-resolution command shows both complete records and requires an explicit
choice of local, imported, or a newly corrected value.

The workflow is:

1. `ath import` creates a read-only preview and a temporary conflict set.
2. `ath conflicts show` and `ath conflicts resolve` record choices in that temporary
   set without changing athlete records.
3. `ath import --yes` rechecks the source package and local database revision, then
   applies the non-conflicting rows and all resolutions in one transaction.

If the package bytes or local database revision changed, the temporary resolutions
are stale and import requires a new preview.

Import confirmation may set the athlete profile's default IANA timezone when it is
unset. Import never infers that value from source offsets or the importing computer.

An empty destination at database revision zero adopts the source database ID and
database revision from an Athletic Standard package. This is initialization from a
complete snapshot, so the restore does not add a revision. It restores provider
import keys and erasure markers with the records. A nonempty destination with the
same database ID may merge records and revisions after preview. One successful merge
increments the local database revision once. A nonempty destination with a different
database ID refuses the package. Combining two live databases needs a separately
approved merge design because they may belong to different athletes.

A same-database merge includes records, revisions, provider import keys, and erasure
markers. Imported records keep their exact `created_at`. Imported revisions keep
their exact `became_known_at` and `became_known_at_basis`. A matching record revision
must contain the same complete state. A provider import key must point to the same
record. A matching erasure marker must contain the same erasure time and reason code.
Any difference is a conflict and the merge writes nothing until it is resolved.

An import never removes local records merely because they are absent from a JSON
package. Deletion requires an imported deletion record or an explicit delete
command.

Import refuses to apply while any unknown field or unrepresentable record remains.
No flag silently discards it.

### 9.3 Prerelease `0.x` records

`ath import` reads `athleticstandard_version` before it previews an Athletic Standard
package. It refuses a version below `1.0.0` with a clear message that prerelease files
are unsupported. It does not modify the input or attempt a partial conversion.

The repository's 0.x fixtures are regenerated directly as v1 fixtures. The
[v0.4.3 migration appendix](appendix/v0.4.3-migration.md) is a superseded design
record. It is not an implementation or release gate.

## 10. CLI changes

Every athlete-data command accepts `--file <path>` for an explicit SQLite database
file.

File discovery prefers:

1. An explicit `--file`.
2. `athlete.ath.sqlite` in the current directory.
3. A clear unsupported-prerelease message when only a `0.x` `.ath.json` exists.

Command behavior:

| Command | v1.0.0 behavior |
|---|---|
| `ath init` | Creates the SQLite database, records the optional default timezone, and installs the local skill |
| `ath import` | Previews and imports provider exports or a v1 Athletic Standard JSON package |
| `ath export` | Writes a complete JSON package |
| `ath log` | Creates one plain-text entry or one confirmed structured batch, plus any unambiguous prediction-result link and tool grade, in one transaction |
| `ath link` | Previews a stable-ID link and writes a revision of the record that owns it |
| `ath records list` | Lists current records with bounded filters and stable ordering |
| `ath records show` | Shows one record, a named revision, its history, and its links |
| `ath correct` | Creates a revision of an existing record after preview and confirmation |
| `ath conflicts show` | Shows the complete local and imported records in one import conflict |
| `ath conflicts resolve` | Records an explicit local, imported, or corrected choice for an import |
| `ath check` | Checks SQLite integrity, schema version, foreign keys, and domain rules |
| `ath stats` | Shows the fixed overview: counts, sources, coverage, and baselines |
| `ath calculate` | Runs a named calculation with explicit sources, dates, and day basis |
| `ath series` | Queries samples by source, quantity, and time |
| `ath predict` | Builds evidence; bare terminal use may save its model's prediction after confirmation |
| `ath grade` | Grades an existing prediction against an existing result |
| `ath backtest` | Reads stable historical revisions without changing athlete data |
| `ath delete` | Previews and writes a deletion record |
| `ath erase` | Performs confirmed privacy erasure |
| `ath share` | Remains unimplemented in the public CLI unless separately approved |
| `ath key` | Keeps the existing optional model-gateway credential workflow; keys never enter athlete data |
| `ath models` | Keeps the existing model list and local default workflow |
| `ath skill` | Keeps installed local Skill copies current |
| `ath --update` | Keeps the existing package and installed-Skill update workflow |

JSON output from commands remains available for harnesses.

`--json` never confirms a write. A non-interactive write requires `--yes` or the
command-specific second confirmation required by `erase`. `ath predict --json`
retains D76's existing evidence response for agent harnesses.

`ath log` remains the general creation command and the only result-entry command.
Its plain-text path keeps the current measurement, athlete-report, and workout
workflow. Its structured JSON path accepts nutrition, plans, goals, reviews,
predictions, prediction error analyses, and other authored records. An attachment
supplied to `log` is checked, copied, hashed, and linked in the same operation.
Separate create commands for each record type are not added.

D72's structured JSON array remains. Every item is validated and shown in one
preview, and the batch writes in one transaction.

D76's narrow exception remains. Bare `ath predict` calls the selected model and may
save that prediction after preview and confirmation. `ath predict --json` only
returns evidence for a harness. The harness writes its prediction through `ath log`.

Logging a benchmark result grades it in the same transaction when exactly one open
prediction is an unambiguous match. The transaction creates the result, a
`prediction_result_link` record when that link is new, and the grade. The preview
shows all three records. No open prediction means only the result is written. More
than one possible prediction means no link or grade is guessed.

`ath grade <prediction-id> --result <record-id>` is the explicit path for imported
results, ambiguous matches, corrected results, and a newer calculation version. It
never creates a result. It creates the prediction-result link when that exact link
does not already exist, then writes the grade against the link revision. The old
`ath grade <benchmark> --actual <score>` form is removed in v1.0.0.
`ath grade --analysis` is also removed. A prediction error analysis is a separate
authored record created through structured `ath log`, which applies D62's existing
checks to every claimed cause.

`ath records` does not replace `stats` or `series`. `stats` is the fixed overview.
`series` is the efficient dense-sample view. `records` finds stable IDs and reads
individual logical records and revisions.

`ath calculate` exposes the runtime operations in §8. It accepts named operations,
not SQL. `ath link` follows the same preview and confirmation rule as `ath correct`.
Both write a revision and refuse a stale expected revision.

`ath backtest` and `ath predict --as-of` decide historical visibility from
`record_revisions.became_known_at`, not the current row's event time alone. Their
receipts identify the revisions and knowledge times used.

Existing positional file arguments for `check` and `stats` remain aliases through
v1.0.0.

Model preferences and backtest reports remain operational side files. Database
commands leave them untouched. Export excludes them unless a report is explicitly
saved as an athlete-owned review.

Add an explicit delete command that previews and writes a deletion record. It never
removes history without confirmation.

Add a separate `ath erase` command for privacy erasure. It shows every dependent
record and attachment that will be removed, requires a second explicit confirmation,
and cannot be undone from the database.

## 11. Database library

Use the current stable `@libsql/client` release if it passes the platform and
performance tests. It provides one API for local SQLite-compatible files and hosted
libSQL.

Use raw, parameterized SQL migrations. Do not add an ORM until repeated application
code shows a specific need.

The database layer must expose domain operations rather than general SQL. Its public
database handle is opaque:

```text
createDatabase
openDatabase
migrateDatabase
importRecords
exportRecords
writeRecord
correctRecord
previewConflict
resolveConflict
deleteRecord
eraseRecord
queryMeasurements
querySeries
querySessions
buildEvidence
listRecords
getRecord
calculate
gradePrediction
```

This keeps the CLI, tests, and an optional hosted adapter on the same rules.

## 12. Safety and integrity

`ath check` must run:

- SQLite `integrity_check`.
- Foreign-key checks.
- Schema migration checks.
- Two-tier wall checks.
- Source and device reference checks.
- Unit and value checks.
- Session ordering checks.
- Duplicate current-record checks.
- Prediction evidence checks.
- Attachment hash checks.
- JSON export validation when requested.

Measured records and athlete reports must remain separate tables. A nullable
`source_id` on one shared signal table is not acceptable.

Database and migration code must use parameterized statements. No record text is
interpolated into SQL.

Treat imports, archives, attachments, photos, and provider files as untrusted.
Enforce documented byte, file-count, record-count, and expansion limits. Stream
large inputs. Reject archive traversal, absolute paths, links, mismatched file types,
and unsupported image encodings. Clean temporary files after success, refusal, or
crash. Parser tests include malformed, deeply nested, oversized, and compressed
expansion cases.

Create databases, attachment directories, and exports with owner-only permissions
where the operating system supports them. Refuse to write secrets or health data
through a symbolic link. Warn before opening an existing file or directory that is
readable by other local users.

The `ath` CLI does not encrypt the local database. Documentation must recommend
operating system disk encryption and explain that anyone who can read the file can
read the athlete record.

## 13. Performance proof

Generate deterministic synthetic databases at:

- 100,000 records.
- 3 million records.
- 30 million records.

Each database must include:

- Several sources measuring the same quantities.
- Dense workout and all-day series.
- Provider summaries and scores.
- Sessions and segments.
- Corrections and deleted records.
- Athlete reports.
- Nutrition.
- Plans and predictions.

Measure:

- Initial import.
- Incremental import.
- Latest-value lookup.
- A 28-day evidence request.
- A 90-day baseline.
- A within-workout series calculation.
- Cross-source comparison.
- Full JSON export.
- Database check.

Record hardware, operating system, database settings, fixture seed, repetitions,
percentiles, cache-reset method, query plans, database size, and memory. Set release
limits from the first measured run and commit the benchmark metadata. Do not claim a
latency target before that run.

If 30 million row tests fail, first test these changes in order:

1. Better indexes and query bounds.
2. Batched writes and prepared statements.
3. Fewer stored text copies.
4. Chunked series tables.
5. Compressed series blocks that preserve complete samples.

A second database engine is not introduced before those tests.

## 14. Compatibility and release

v1.0.0 must:

- Generate and import the deterministic v1 fixture.
- Refuse Athletic Standard `0.x` documents without modifying them.
- Preserve source separation.
- Preserve SDNN and RMSSD as different measurements.
- Preserve every original timestamp and offset supplied by a route.
- Preserve every accepted dense sample.
- Preserve provider summaries and scores.
- Preserve prediction history and evidence.
- Export a package accepted by the v1.0.0 JSON Schema.

The npm package and format version both become `1.0.0`.

The existing `0.4.x` package remains available. Its behavior and old build history do
not change.

## 15. Tests

Required automated tests:

- SQL migration tests from an empty database through the latest version.
- Transaction rollback at each import and write failure point.
- Foreign-key and two-tier wall failures.
- Manual-source records never entering device baselines.
- V1 JSON fixture import.
- Import preview leaves the database unchanged until confirmation.
- JSON export and re-import equality.
- Deterministic export.
- V1 measurement-series file import and export with hash validation.
- UUIDv7 generation for new records.
- V1 package restore preserving record IDs, revisions, and creation times.
- Golden provider `record_import_keys.match_key` values for every supported route,
  including duplicate collision positions.
- Every `records.record_type` mapping to its typed table and every required parent
  foreign key.
- No superseded proposed-v1 names in current v1 schema keys, CLI help, v1 output
  fields, exports, or the generated Skill reference.
- Refusal of every Athletic Standard version below `1.0.0`.
- Date-only records without invented timestamps.
- WHOOP cycles and Oura activity, sleep, calendar, and unspecified day kinds
  remaining distinct.
- Apple timestamp offsets preserved without being claimed as recording timezones.
- Fixed-width normalized UTC timestamps sorting correctly across fractional seconds.
- Calendar calculations across travel and daylight-saving boundaries.
- Refusal when a calendar calculation has no explicit or saved timezone.
- Source-specific query behavior.
- Provider correction behavior.
- Provider re-import matching with provider IDs, fallback keys, and corrections.
- Export, empty-database restore, and later provider correction matching the restored
  import key instead of creating a duplicate.
- Same-database package merge preserving creation times, revision knowledge times,
  provider import keys, and erasure markers while refusing conflicting values.
- Stable IDs across export and import.
- Exact `became_known_at` behavior in historical queries.
- Record listing, revision reads, and stale-revision correction refusal.
- Structured `ath log` arrays remain atomic.
- Result logging with zero, one, and several possible open predictions.
- Explicit grading of existing prediction and result IDs.
- A prediction-result link without a saved grade.
- A logged or explicit grade creating a missing prediction-result link.
- Several grade calculation versions against one result-link revision.
- A corrected result creating a new link revision while an older grade keeps its
  original link revision.
- Removal of the old `ath grade <benchmark> --actual` form.
- Removal of `ath grade --analysis`.
- D62 evidence checks accepting and refusing structured `ath log` prediction error
  analyses.
- `--json` never confirming a write.
- Concurrent reader and writer behavior.
- WAL recovery after interrupted writes.
- Conflict refusal and explicit resolution.
- Conflict plans becoming stale after package or database changes.
- Privacy erasure with broken evidence references.
- Normal deletion retaining its parent and revision while privacy erasure may leave
  only a value-free `record_erasure_markers` row.
- Matching `athleticstandard_version` values across the v1 document, manifest,
  measurement-series files, and database metadata.
- No UUIDv5 method, `unknown` author type, `input_athleticstandard_version`,
  `legacy_sample_groups`, `legacy_sample_files`, or
  `prediction_error_cause_record_candidates` in the active v1 schemas.
- Untrusted archive, file-name, link, and image handling.
- Owner-only file creation where the platform supports it.
- Cross-platform file opening on Linux, macOS, and Windows.
- CLI help and generated Skill reference.
- Performance suites at the three required sizes.

CI uses the small fixture. Large performance suites run in a separate release job.

## 16. Documentation

Update:

- `SPEC.md` with the logical model and both encodings.
- The generated JSON Schema.
- README quickstart and the `0.x` refusal notice.
- CLI help.
- Skill references.
- Package exports.

Documentation must say:

- SQLite is the live local record.
- JSON is an explicit portable snapshot.
- An export is not kept synchronized.
- Import previews every change.
- The file contains sensitive health and training data.
- Local use requires no account or hosted service.

## 17. Build order

1. Record the approved plan and v1.0.0 decisions without changing prior decisions.
2. Define stable IDs, revisions, time and provider reporting periods, nutrition,
   plans, and authored records in Zod.
3. Add SQL migrations and the local libSQL adapter.
4. Generate the SQLite fixture and database test helpers.
5. Move read commands to SQLite.
6. Move write, grade, prediction, and backtest commands to SQLite transactions.
7. Add v1 JSON package import and deterministic JSON export.
8. Migrate provider importers and dense samples.
9. Update `ath check`, help, the Skill, SPEC, and README.
10. Run all functional and performance suites.
11. Release a prerelease for v1 package and database testing.
12. Release v1.0.0 only after round-trip and cross-platform tests pass.

Each implementation step must pass:

```text
pnpm typecheck
pnpm test
pnpm build
```

Schema changes must also regenerate the JSON Schema and fixture. CLI or help changes
must regenerate the Skill command reference.

## 18. Approved release-test size change

D114 supersedes the 30-million-record requirement in section 13. Required measured
runs use 100,000 records in normal release CI and 3 million records in the separate
large release job. Larger generated databases are optional investigation tools and
are not a release gate.
