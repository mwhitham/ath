# Athletic Standard v1.0.0 — SQLite Build Plan

Status: planning draft. No v1.0.0 decisions are recorded yet.

On approval, append v1.0.0 decisions beginning with D82. They must explicitly
supersede the earlier no-database, sidecar-storage, compound-link, no-record-ID, and
no-planning decisions. Earlier decision files remain unchanged.

This version changes the live local record from JSON plus sidecar files to SQLite.
It keeps JSON as a complete import and export format.

The change is a major version because it changes how every command finds, reads, and
writes an athlete record. Existing `0.x` files remain readable through an explicit
import.

## 1. Outcomes

v1.0.0 must provide:

1. One SQLite database for one athlete.
2. The existing two-tier wall as database structure and validation.
3. Stable IDs and revisions for every record that can be corrected or synchronized.
4. Storage for every accepted wearable sample and every summary reported by a source.
5. Runtime calculations over the recorded data.
6. Complete, deterministic JSON import and export.
7. A safe migration from existing `.ath.json` files and series sidecars.
8. The same CLI and Skill workflow where the storage change does not require a command
   change.
9. A database model that uses portable SQLite features and can be adapted to libSQL.

The open package remains useful without an account, network connection, or hosted
service.

## 2. Scope boundary

This public build includes:

- The SQLite schema and migrations.
- The domain schemas that describe athlete-owned records.
- Local database discovery, opening, transactions, and checks.
- Importers for existing Athletic Standard JSON and supported provider exports.
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

`format_metadata` is the sole metadata table. It records:

- Application marker.
- SQL schema version.
- Athletic Standard format version.
- Database ID.
- Creation time.

Local SQLite mirrors the SQL schema version into `PRAGMA user_version` as an
additional check. Hosted libSQL adapters must use `format_metadata`; they cannot
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

A hand-entered measurement or benchmark result uses a source whose kind is `manual`,
as required by D45. It is always labeled as manually recorded. It never enters a
device baseline or counts as device-measured evidence. Feelings, journal answers,
meals, and notes remain source-free athlete reports.

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

Author types are `athlete`, `agent`, `tool`, and `unknown`. Agent and model are
required only for an agent author. Tool-generated records name the tool, tool
version, and calculation version. `unknown` is accepted only during a legacy import
that cannot recover the original author.

## 7. Required database tables

The first migration should use ordinary SQL without an ORM.

### 7.1 Database and athlete

```text
format_metadata
athlete_profile
database_revision
records
record_revisions
record_tombstones
```

`database_revision` is a monotonically increasing integer. Every committed athlete
change increments it once.

`records` is the parent for every athlete-owned record. It contains the stable record
ID, kind, current revision, creation time, and deletion state. Typed tables use that
ID as their primary key and foreign key. Evidence references `(record_id, revision)`
through this parent structure.

### 7.2 Sources

```text
sources
source_devices
```

A source is unique by writer and route. A named sensor class also splits the source
when its measurement method requires a separate baseline, such as Apple Watch optical
HRV and Apple Watch ECG beat timing. WHOOP through Apple Health and WHOOP CSV always
have different source IDs. Hardware model and serial details are descriptive. They do
not split a source when the writer, route, and sensor class stay unchanged.

The source record retains:

- Writer.
- Vendor.
- Device and sensor details when supplied.
- Import or connector route.

### 7.3 Measured data

```text
point_measurements
series_samples
sleep_sessions
workout_sessions
session_segments
benchmark_results
provider_scores
```

`series_samples` stores every accepted sample. Its main index is:

```text
(quantity, source_id, recorded_at)
```

Every measured row also carries:

- A stable record ID.
- Source ID.
- Normalized timestamp.
- Original timestamp text and offset when the route exposes them.
- An absence reason when the original timestamp or offset is unavailable.
- Original unit and value when the route exposes them.
- Normalized unit and value.
- Provider record ID when supplied.
- Time received.
- Current revision.

Every revision stores the complete record state, the time it became known, the author
of the change, and the revision it superseded. The current typed row is updated in
the same transaction. An as-of query considers only revisions known by its cutoff.

A tombstone names the record ID, deletion time, author, and revision. It hides the
record from current queries while preserving the historical event until the
retention rule permits removal.

Normal correction and deletion use revisions and tombstones. Privacy erasure is a
different operation. It removes the current row, all value-bearing revisions,
samples, attachments, and derived records that depend on the erased record. It may
leave only a minimal tombstone with the record ID, erasure time, and reason. That
tombstone contains no health value, free text, source details, or attachment hash.
An evidence reference to it resolves as `erased`; it never falls through to another
record.

### 7.4 Athlete reports

```text
soft_signals
nutrition_entries
nutrition_reported_items
attachments
```

`soft_signals` and nutrition tables have no device source column.

Structured nutrition must support:

- Original athlete description.
- Foods, amounts, and nutrients stated by the athlete.
- Attachment receipt for a photo.

Missing meals never mean zero intake.

The athlete's description, photo, and stated values remain the athlete report.
Model-inferred foods and nutrients are a linked authored record in
`nutrition_interpretations`. It names the model, model version, confidence, evidence
attachment or text, and creation time. Neither the report nor its interpretation may
carry a device source.

### 7.5 Benchmarks and predictions

```text
benchmarks
predictions
prediction_evidence
prediction_grades
miss_analyses
```

The prediction ledger remains a first-class part of the format. Evidence references
name stable record IDs and revisions.

Grades are calculated at runtime. A saved grade is an authored record of what the
tool calculated at that time. It names the calculation version and inputs.

### 7.6 Plans and saved agent work

```text
training_plans
planned_sessions
planned_blocks
coaching_notes
saved_reviews
goals
nutrition_interpretations
```

Plans are athlete-owned data and must round-trip through JSON.

A planned session remains separate from a completed workout session. Completion may
link the two without rewriting the plan as a measured workout.

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

Raw series remain available. A provider summary must not replace or hide the samples
that support a different question.

Calculations group by quantity and source. Multiple sources return separate results
unless the requested operation is a named source comparison. SDNN and RMSSD never
share a calculation. Distinct temperature types and activity-specific distance types
also remain separate.

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
3. Reads current athlete-owned records, retained revisions, and tombstones.
4. Writes deterministic JSON with stable ordering.
5. Writes generated series files and permitted attachments.
6. Validates the result against the public schemas.
7. Writes the manifest last.

The document and manifest have normative Zod schemas. A separate deterministic JSON
writer defines key order, number text, Unicode encoding, line endings, timestamp
serialization, revision records, and deletion records. Zod validates values but does
not define their byte encoding. Every referenced attachment must be exported or the
export fails.

The manifest has an optional `exported_at`. It is absent by default. A caller may
supply a fixed value explicitly. Two exports from the same database revision and the
same explicit options are byte-identical. Golden byte fixtures enforce this.

### 9.2 Import

`ath import` detects an Athletic Standard JSON document or package.

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

An empty destination adopts the source database ID from an Athletic Standard
package. This is a restore. A nonempty destination with the same database ID may
merge records and revisions after preview. A nonempty destination with a different
database ID refuses the package. Combining two live databases needs a separately
approved merge design because they may belong to different athletes.

An import never removes local records merely because they are absent from a JSON
package. Deletion requires an imported tombstone or an explicit delete command.

Import refuses to apply while any unknown field or unrepresentable record remains.
No flag silently discards it.

### 9.3 Existing `0.x` records

The migration steps are:

```text
ath init
ath import athlete.ath.json
ath check
ath export migrated-athlete/
```

The original JSON and sidecars remain untouched.

When a command finds only an old JSON record, it prints the migration command. It
does not migrate automatically.

Legacy IDs are deterministic. Their inputs are the complete old reference:

- Point or score: kind, source, timestamp, and original position when needed.
- Session: kind, source, and start timestamp.
- Benchmark result: benchmark, recorded timestamp, and original position when needed.
- Athlete report: type, reported timestamp, and original position when needed.
- Prediction: old prediction ID where present, otherwise benchmark, creation time,
  and original position.

Hash collisions receive a deterministic suffix based on original position. Repeating
the same import produces the same IDs and no new revisions.

Convert legacy links from `(source,start)` and `(benchmark,recorded_at)` to stable
record IDs. Preserve durations, derived-value receipts, source device windows, soft
input details, notes, benchmark-result segments, prediction authorship, grades, and
miss analyses. Hand-recorded result segments remain separate from device session
segments.

A valid legacy package may omit its series folder. Import its inline records and
preserve one unavailable-series receipt per source and quantity with the old
coverage and hash. Never invent samples. `ath check` reports the missing data.

A field-by-field v0.4.3 migration appendix must be approved before implementation.

## 10. CLI changes

Every athlete-data command accepts `--file <path>` for an explicit SQLite database
file.

File discovery prefers:

1. An explicit `--file`.
2. `athlete.ath.sqlite` in the current directory.
3. A clear migration message when only one legacy `.ath.json` exists.

Command behavior:

| Command | v1.0.0 behavior |
|---|---|
| `ath init` | Creates the SQLite database and installs the local skill |
| `ath import` | Imports provider exports or an Athletic Standard JSON package |
| `ath export` | Writes a complete JSON package |
| `ath log` | Writes one confirmed record in a transaction |
| `ath link` | Links stable IDs instead of timestamp-only references |
| `ath check` | Checks SQLite integrity, schema version, foreign keys, and domain rules |
| `ath stats` | Calculates requested counts and statistics from current records |
| `ath series` | Queries samples by source, quantity, and time |
| `ath predict` | Builds evidence from SQLite |
| `ath grade` | Writes a versioned grade and linked result |
| `ath backtest` | Reads stable historical revisions without changing athlete data |
| `ath share` | Remains unimplemented in the public CLI unless separately approved |

JSON output from commands remains available for harnesses.

`ath key`, `ath models`, `ath skill`, and `ath --update` retain their current
behavior. Existing positional file arguments for `check` and `stats` remain aliases
through v1.0.0.

Model preferences and backtest reports remain operational side files. Migration
leaves them untouched. Export excludes them unless a report is explicitly saved as
an athlete-owned review.

Add an explicit delete command that previews and writes a tombstone. It never removes
history without confirmation.

Add a separate `ath erase` command for privacy erasure. It shows every dependent
record and attachment that will be removed, requires a second explicit confirmation,
and cannot be undone from the database.

## 11. Database library

Use the current stable `@libsql/client` release if it passes the platform and
performance tests. It provides one API for local SQLite-compatible files and hosted
libSQL.

Use raw, parameterized SQL migrations. Do not add an ORM until repeated application
code shows a specific need.

The database layer must expose domain operations rather than general SQL:

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

Hard and soft records must remain separate tables. A nullable `source_id` on one
shared signal table is not acceptable.

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

- Read and import every valid current fixture.
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
- Existing JSON fixture import.
- JSON export and re-import equality.
- Deterministic export.
- Sidecar migration with hash validation.
- Source-specific query behavior.
- Provider correction behavior.
- Stable IDs across export and import.
- Concurrent reader and writer behavior.
- WAL recovery after interrupted writes.
- Conflict refusal and explicit resolution.
- Privacy erasure with broken evidence references.
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
- README quickstart and migration guide.
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

1. Approve this plan and record the v1.0.0 decisions without changing prior decisions.
2. Define stable IDs, revisions, nutrition, plans, and authored records in Zod.
3. Add SQL migrations and the local libSQL adapter.
4. Generate the SQLite fixture and database test helpers.
5. Move read commands to SQLite.
6. Move write, grade, prediction, and backtest commands to SQLite transactions.
7. Add legacy JSON import and deterministic JSON export.
8. Migrate provider importers and dense series.
9. Update `ath check`, help, the Skill, SPEC, and README.
10. Run all functional and performance suites.
11. Release a prerelease for migration testing.
12. Release v1.0.0 only after round-trip and cross-platform tests pass.

Each implementation step must pass:

```text
pnpm typecheck
pnpm test
pnpm build
```

Schema changes must also regenerate the JSON Schema and fixture. CLI or help changes
must regenerate the Skill command reference.
