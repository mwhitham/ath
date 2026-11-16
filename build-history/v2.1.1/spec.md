# Athletic Standard 2.1.1 — complete native rebuild

Status: **approved direction; release target 2.1.1**. The complete native rebuild
is selected for this version. Publication remains separate from implementation.

This specification replaces the unapproved [first draft](appendix/draft-2.1.1/spec.md),
which was written as version 3.0.0. That draft remains in build history (D224). This
version preserves the current public contracts. The draft's proposed JavaScript
removals are not part of 2.1.1.

## 1. The result

Athletic Standard becomes one Rust library and one native `ath` executable. The
library owns the complete record lifecycle. Every existing working command uses
that library. TypeScript remains only in the optional npm client and its tests.

This is one rebuild with one release acceptance decision. Implementation has a
dependency order, but an intermediate Rust/TypeScript system is not a deliverable.
No feature is postponed to make the rebuild appear complete. The old release stays
usable until the complete replacement passes the requirements below.

The purpose remains an open, local-first athlete record that agents can read and
write. SQLite is the live local record. JSON is the complete portable package.
Measured records require a source. Athlete reports cannot claim one. The reference
agent predicts benchmark performance and grades predictions against results.

The rebuild is worthwhile only if it removes duplicated responsibilities, makes
installation self-contained, preserves data, and produces measured improvements.
A directory rearrangement or a line-for-line port does not satisfy this plan.

## 2. Evidence for the rebuild

The starting reference is commit `c6218e88c035708bdd15b3a20888d19713e1df29`:
software and record version 2.1.0, SQL schema step 4. These observations concern
the source, not a measured runtime result:

- `src/program.ts` has 2,499 lines and coordinates domain operations as well as
  argument handling and output.
- `src/` has 79 TypeScript files and 24,912 lines. The existing Rust source has
  15 files and 9,206 lines. These counts include comments and inline tests. They
  are context, not a target for deleting useful code.
- `src/db/records.ts`, `crates/ath-core/src/persist.rs`, and
  `crates/ath-core/src/store.rs` all encode logical records into SQL rows.
- WHOOP and Oura CSV readers run in Rust, return legacy signal objects through
  TypeScript, become current records in `src/db/provider-import.ts`, then return
  to Rust for persistence. The legacy schema is still a runtime dependency.
- The current CLI uses both `@libsql/client` and `rusqlite`. Package import,
  erasure, repair, and migrations still have TypeScript write implementations.
- `src/ath-core.ts` launches a child process with buffered JSON. A checkout may
  rebuild that child based on modification times during an ordinary command.
- Backup and repair require a separate `sqlite3` executable. The JavaScript driver
  also needs file-handle cleanup workarounds on Windows.
- `src/db/typed-denormalized.ts` checks disagreement between typed columns,
  complete JSON payloads, child rows, and revisions. Those representations serve
  query and history requirements. Their maintenance is spread across writers.
- `src/v1/schema.ts` defines the current format. `src/schema.ts` is legacy 0.x.
  Several comments and SQL instructions still describe older versions as current.

The new information is the implemented overlap after the partial Rust move. It
supports revisiting implementation ownership. It does not justify dropping the
record guarantees approved in earlier decisions.

Relevant decisions include D82–D105, D108–D118, D124–D128, D152–D174, and
D177–D197. Earlier specifications and decision entries remain unchanged.

## 3. Completion requirements

All of these are required for release:

1. One Rust model defines all 24 current record types, revisions, packages, and
   their semantic validation. Generated JSON schemas and TypeScript types agree
   with it. The native implementation has no second domain schema. The npm-only
   compatibility schemas retain the published Zod interface described in section 11.
2. One record encoder maintains typed rows, child collections, complete JSON,
   revisions, and relationships. Normal writes, imports, restore, and repair use it.
3. One transaction owner implements each complete operation. Callers cannot supply
   a sequence of low-level `insert`, `validate`, and `bump` instructions.
4. All currently working commands and supported provider routes are present.
   The existing `share` placeholder remains explicitly unimplemented. Hosted
   sharing is not invented as part of this rebuild.
5. Native local use needs no Node installation, Rust compiler, `sqlite3` executable,
   service, account, or network connection. Optional model calls and Garmin pulls
   keep their explicit network requirements.
6. Existing supported databases and packages migrate or restore without losing
   records, revisions, sources, links, unknown supplied fields, or attachments.
7. Published direct-SQL reads and writes continue to work. Critical database rules
   remain enforced by SQLite even when the Rust library is bypassed.
8. The tested release artifacts work on Linux x64, macOS arm64, and Windows x64.
   The npm route works on supported Node versions without compiling native code.
9. The performance requirements in section 14 pass on a named reference runner.
10. The old TypeScript domain implementation, legacy import intermediates, and
    `ath-core` executable bridge are removed from the active native source tree.
    The retained npm compatibility facade is not a TypeScript database implementation.

An acceptance checklist may not mark an absent feature as a follow-up. A test
failure may not be resolved by weakening a data guarantee or replacing the expected
output with the new implementation's output without a documented reason.

## 4. Scope and preserved behavior

### 4.1 Record guarantees

Preserve all current record types. This includes plans, goals, nutrition, authored
work, prediction-result links, grades, attachments, provider summaries, scores,
samples, device records, and the athlete profile.

Preserve stable UUIDv7 record IDs and per-record revisions. Corrections retain
their IDs and complete earlier states. Evidence names both the record and its
revision. Provider matching keys remain distinct from record IDs.

Preserve measured, athlete-reported, and authored classes. Manual measurements
remain visibly manual and stay out of device baselines. Unknown provider fields
remain under their supplied names. Extensions retain their names, limits, and
source restrictions. `field_basis` retains its exact meaning and remains optional
on older data. Migration does not invent it for historical records.

Preserve original source values, units, timestamp text, offset text, missing
reasons, provider periods, and source-specific distinctions. RMSSD and SDNN remain
different quantities. Distinct temperature signals and modality distances remain
different quantities. A provider score is not a measurement. A provider-assigned
period is not silently changed into a calendar day.

Preserve normal deletion and privacy erasure as different operations. Erasure
removes value-bearing history and dependent records. Its retained markers contain
only the permitted value-free information.

### 4.2 Command coverage

Preserve command names, positional arguments, flags, defaults, confirmation rules,
exit behavior, and machine-readable field meanings unless this specification names
a change. Generate a command inventory from the old program before replacing it.
Every inventory entry must map to a test of the new executable.

- `init`, including profile prompts, timezone, seeded benchmarks, `--empty`, and
  `--no-skill`.
- `import`, including provider detection, preview, confirmation, complete package
  restore and merge, `--mapping`, and all supported file and directory forms.
- `conflicts show` and `conflicts resolve`, including local, imported, and corrected
  choices and refusal of stale choices.
- `export`, including deterministic output and optional fixed export time.
- `log`, including plain text, structured records, atomic arrays, attachments,
  manual measurement rules, and unambiguous automatic result grading.
- `correct`, `link`, `delete`, and `erase`, including expected revisions and the
  additional migration and erasure confirmations where currently required.
- `records list`, `records show`, retained history, exact revisions, and links.
- `check`, `stats`, `series`, `calculate`, and `apply`.
- `migrate` and both repair modes, including preview and verified backups.
- `predict`, including offline evidence for an agent and optional terminal model
  calls; `grade`; and `backtest` with per-model reports and partial model failures.
- `pull garmin`, `key` status/set/clear, `models`, `skill` status/install, and
  `--update` for existing npm installation routes.
- Global help, version, file selection, working-directory discovery, progress,
  standard input, `--json`, and non-interactive refusal behavior.

`--json` remains a preview on commands where it is a preview today. Supplying
`--yes` beside it does not convert that preview into a write. Native read commands
do not change journal mode, file headers, or athlete data merely by opening a file.

Human help can become clearer. JSON schemas, field meanings, exit behavior, and
confirmation behavior receive stronger compatibility checks than terminal spacing.
Intentional help changes regenerate the installed CLI reference in the same change.

### 4.3 Provider coverage

Preserve Apple Health XML and ZIP imports, inline and referenced workout routes,
ECG-derived beat processing, WHOOP CSV bundles, all supported Oura CSV variants,
Garmin account exports and activity JSON, GPX, FIT, Garmin personal pulls, Strava
bulk exports, and the shipped Open Wearables and Junction mappings.

Keep route-specific matching rules. Repeating an unchanged import matches existing
records. A changed provider record creates a revision. Ambiguous changed duplicate
groups report a conflict instead of guessing. Two devices describing one workout
remain two source records.

Retain all current route containment checks, Apple writer rules, FIT absent-value
handling, Garmin maximum-heart-rate distinctions, Oura periods, WHOOP cycles,
skip counts, and reasons for omitted objects. A new generic reader must not erase
provider-specific meaning.

New provider integrations, new online connections, mobile applications, hosted
services, a plugin registry, and new coaching behavior are outside the rebuild.
Existing editable mappings and calculation files remain supported.

## 5. Repository and ownership

Use one Cargo package named `athleticstandard`. It builds the `ath` binary and the
`athleticstandard` library. Start with ordinary Rust modules. Do not create a crate
per directory or a generic application framework.

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
src/
  lib.rs                 public library operations
  main.rs                process entry and top-level error handling
  cli/                   arguments, terminal confirmation, rendering, progress
  model/                 typed records, revisions, validation, units, time
  store/                 connections, queries, encoding, transactions, checks
  import/                detection, provider readers, matching, mappings
  package/               manifest, complete export, restore, merge, conflicts
  calculate/             typed calculation settings, arithmetic, receipts
  prediction/            historical evidence, linking, grading, replay rules
  runtime/               optional HTTP, credentials, model preferences, updates
  assets/                installed skill and mapping asset access
  files/                 private files, staging, durable publication, recovery
mappings/                shipped editable mapping files
skills/                  shipped agent and calculation instructions
spec/sql/                normative ordered SQL migrations and public contract
schema/                  generated current schemas and preserved old versions
packages/npm/            launcher, generated types, thin client, client tests
tools/                   generation, public-data checks, comparison, benchmarks
tests/                   Rust integration tests and synthetic fixtures
docs/
  architecture.md        current responsibilities and ordinary operation walkthrough
  upgrading-to-2-1-1.md      database, CLI, npm, and JavaScript migration instructions
build-history/           append-only specifications, decisions, and progress
```

Dependency rules:

- `model` has no database, terminal, HTTP, credential-store, or process dependency.
- `store` depends on the model and file-safety helpers. It does not prompt or call
  a model. It owns the connection and transaction lifecycle.
- Readers depend on the model and a small input-reader interface. They produce
  typed import candidates. They do not issue SQL or select a confirmation policy.
- Package restore and provider import use the shared store encoder. Their matching
  policies remain distinct because restoring retained history differs from receiving
  a new provider observation.
- Calculations consume typed records or bounded query results. They return values
  and receipts. Reading a calculation does not silently save an athlete record.
- Prediction rules depend on the store and calculations. Optional network behavior
  is injected through a narrow model-client interface.
- `cli` composes operations and renders results. It contains no record-to-table
  mapping, import matching, grading arithmetic, or history-maintenance logic.
- `runtime` implements explicit optional services. Local record operations do not
  initialize a network client or require a credential store.
- The npm client performs transport and JavaScript adaptation. It contains no SQL,
  import rules, arithmetic, or handwritten record validation.

Use synchronous library operations and streaming I/O by default. A global async
runtime is not required for local SQLite work. Add concurrency only for a named
operation that benefits from it and has bounded resource use.

## 6. Typed model and generated contracts

Port every current schema field and refinement into Rust. The port must cover
absence versus null, strict objects, allowed values, ranges, timestamp precision,
safe integer bounds, nested collections, protected fields, and cross-field rules.
Deriving deserialization alone does not complete validation.

Use typed record variants and small checked types for IDs, revisions, quantities,
units, UTC times, calendar dates, and evidence references. Keep arbitrary JSON only
where the format permits it, such as supplied fields and namespaced extensions.
Unknown fields on a strict record remain errors. They are not silently ignored by
Serde. Historical versions have explicit readers; permissive field dropping is not
a version-conversion strategy.

The source of truth after cutover is the Rust model plus its explicit semantic
checks. Use Serde for encoding and Schemars for JSON Schema generation. Generate
TypeScript declarations from the same published definitions. Custom checked types
provide the schema metadata that plain primitive types cannot express. Preserve
draft 2020-12 and published schema IDs for preserved historical files.

Keep an explicit list of semantic rules that JSON Schema cannot fully express.
Those rules execute in the Rust validator and have language-neutral valid/invalid
test vectors. Schema generation must not claim that successful structural schema
validation proves a complete package's history or database relationships are valid.

One generation command writes current JSON schemas, current deterministic fixtures,
SQL contract metadata, npm declarations, and the CLI skill reference. A separate
check command runs generation into temporary output and compares committed files.
Generators do not overwrite published historical schemas, SQL, or fixture baselines.
Temporary comparison normalization is not a change to a published file.

Zod stays authoritative during implementation until the contract comparison passes.
At the final cutover, update AGENTS.md and the public field reference together.
The native implementation uses Rust validation exclusively. Preserve the published
Zod schema objects in an npm compatibility facade. Their 2.1.x behavior is frozen
and tested against the Rust validator. They do not become a second place to design
record changes. All native writes still use the complete Rust checks.

## 7. One storage implementation

Keep the current relational layout and the published SQL contract. Do not replace
the database with one generic JSON table. Keep complete revision payloads and the
typed columns and relationships needed for efficient queries and direct-SQL use.

Use bundled SQLite through `rusqlite`. A store operation owns its connection and
holds a single write transaction for its complete logical change. Backup uses the
additional source/destination connections required by SQLite's backup API. This is
an explicit same-process operation, not a second database implementation.

The store has one record encoder. It maps a checked record into its typed row,
ordered children, relationship rows, and complete stored JSON. The same field
description is used for consistency checks and repair. A field is not separately
mapped by the normal writer, importer, restorer, and repair tool.

Keep three distinct operations over that encoder:

- Create or correct: validate the complete candidate, check the expected state,
  append the revision, replace current typed data and children, and update the
  database revision once for the complete operation.
- Restore or merge retained history: validate the complete history, preserve its
  IDs and revision metadata, and materialize its final state without inventing new
  local revisions except where the existing merge rules require one.
- Repair current encoding: rebuild typed data from the retained current revision
  after a verified backup. Do not manufacture an athlete correction to disguise a
  storage repair.

Public methods describe domain operations. They do not expose arbitrary SQL
callbacks or a low-level action interpreter. Prepared statements and batched writes
are internal details. Foreign keys and deferred relationship checks are enabled
and checked for each write transaction. The required busy timeout remains applied.

Retain SQLite constraints and triggers for critical source, revision, deletion,
erasure, and protected-link rules. Direct-SQL writers still follow the documented
transaction recipe and collection replacement rules. Tests exercise these writes
with an independent SQLite connection, without the library validator.

Read-only operations use an explicit read policy. They inspect versions before
changing file state. A mismatched application marker or header is refused. They
must read committed WAL data correctly without deleting sidecars, forcing an
immutable-file interpretation of an actively written database, or checkpointing on
close. The plan does not promise that SQLite can read every WAL configuration from
a directory that forbids all auxiliary files; unsupported cases receive a clear
error and leave the database untouched.

Historical queries keep event time separate from the time a revision became known.
Newer corrections cannot leak into evidence for an earlier prediction. Query limits
are enforced in SQL before full payloads are loaded where possible.

## 8. Import previews and bounded processing

Every provider reader emits a typed candidate with its route, original matching
facts, source facts, receipt, and the information needed to report omissions. It
does not emit a legacy 0.x signal or write directly to the live database.

One import operation performs detection, source resolution, matching, validation,
preview, and commit. Provider adapters supply route-specific matching information.
The operation owns revision and conflict handling. New providers should not require
another persistence implementation.

Large inputs stream into a private temporary SQLite staging database or bounded
files. Staging is disposable working data. It is not a second live athlete record.
It stores the plan without retaining millions of candidates in memory. Temporary
data receives the same privacy protections as the live record.

The preview is built from that staged plan and a known live database revision.
Apply checks the revision and relevant current record revisions inside the write
transaction. A stale plan is refused. Matching rules are evaluated against the same
immutable input bytes that were staged. A later command rereads and stages its
input before making a new preview and confirmed change; it does not trust counts
from an earlier terminal run.

No writer lock is held while waiting for a person, reading a large archive, calling
a model, or fetching a provider response. Source creation, inserted records,
corrections, matching keys, and database revision changes commit together.

Preserve mapping syntax, installed-file precedence, CRLF handling, layout settings,
unit pairs, fallback order, writer conditions, supplied-field rules, and the four
semantic refusals. Parse mapping settings into typed structures before any write.
The engine continues to support locally edited files without executing arbitrary
code. Preserve the current calculation-file language under the same rule.

All archive and package limits become named constants in one input-policy module.
Carry forward existing limits for entries, expanded bytes, expansion ratios,
document sizes, nesting, object counts, extensions, and attachments. Measure actual
expanded bytes as well as trusting declared sizes. Explicit limits may reject work;
silent truncation may not make work appear complete.

## 9. Complete packages, files, and recovery

### 9.1 Package behavior

Export reads one consistent database snapshot. It streams records and complete
history in the existing stable order. Every current sample travels with all of its
retained revisions, even if a revision changed its source, quantity, or time.

Preserve main-document and sample-file ownership, series grouping and splitting,
deletions, erasure markers, matching keys, attachments, counts, and manifest hashes.
Retain unknown-field and unrepresentable-content refusals on restore. A partial
restore is not success.

Keep deterministic serialization explicitly tested. Rust map ordering and default
number formatting are not assumed to reproduce JavaScript output. Test UTF-16 key
order, supplementary Unicode characters, escaping, negative zero, exponent forms,
large safe integers, and floating-point round trips. For preserved versions, the
serializer must reproduce expected package bytes. Include integer-like object keys
and malformed Unicode escape cases in the compatibility vectors. Where JavaScript
and Rust disagree, define the accepted representation explicitly before changing a
reader. Do not discard supplied data to make the serializer simpler. For 2.1.1,
only documented version and manifest differences may change when the logical
contents are unchanged.

Empty restore preserves the source database ID, retained record IDs, history, and
database revision under the existing rules. Same-database merge retains the conflict
workflow and refuses stale resolutions. Cross-database cases keep their current
explicit rules; this rebuild does not invent synchronization.

### 9.2 Filesystem safety

Preserve exclusive creation, owner-only files where supported, restrictive temporary
directories, symbolic-link checks, archive traversal checks, and attachment hash,
size, media-type, and containment validation. Carry these rules into native code;
switching languages does not remove them.

One small file module owns staged writes, file synchronization, final rename, and
cleanup. Export publishes the manifest last and makes a complete staged directory
visible according to the supported operating system's rename guarantees. Existing
destinations are not overwritten accidentally. Cleanup acts only on owned staging
areas and never removes an active process's work based on a filename prefix alone.

SQLite transactions cannot atomically commit filesystem changes. Attachment import
and privacy erasure therefore need explicit recovery states. Do not describe an
ordinary rename plus SQL transaction as atomic across both systems.

- Attachment creation synchronizes operation-owned files before committing their
  references. An interrupted attempt may leave unreferenced files, but it must not
  commit a reference to a missing file. Durable ownership markers allow later
  cleanup of only those unreferenced files created by that operation.
- Erasure records pending attachment removal durably as part of the committed
  database operation. It reports completion only after the files are removed and
  directory changes are synchronized where supported. A failed removal remains
  visible as incomplete cleanup. It is not reported as successful erasure.
- A writable operation can resume already approved cleanup. A read-only command
  reports pending work without changing the database or files. `check` reports it.
- Recovery metadata is internal operational data. It contains no copied health
  values and is not exported as athlete-owned history. Newly required internal
  tables belong in migration 005; existing public tables are not repurposed.

Test process termination at each boundary. Include full disk, denied permissions,
corrupt files, sharing violations, two concurrent writers, and a reader with an
active WAL writer. Never delete WAL or shared-memory files to force cleanup.

## 10. Calculations and reference-agent behavior

Preserve the eight named range calculations and every shipped one-record skill.
Retain source separation, manual-source exclusions, provider-period rules, method
names, units, fields read, input revision references, and exclusions.

Keep the 100,000-input refusal and the 1,000-reference receipt limit until a
separately approved format or calculation change replaces them. A shortened receipt
must say it is shortened. Arithmetic must still use the complete accepted input
set. Inspect query plans so bounded calculations do not scan all historical JSON.

Keep calculation definitions editable. Typed settings choose reviewed arithmetic
operations and parameters; they do not become a general expression interpreter.
Identical records and identical definition files produce identical results.
Preserve the existing receipt fields and their meaning. Do not add record fields
or change a stored calculation method as part of this implementation rebuild.

Port beat intervals, ECG peak detection, RMSSD arithmetic, route splits, elevation,
and all import-time calculations that currently live outside the Rust core.
Invented noise and artifact cases test the numerical behavior. Do not assume a
different numeric implementation is equivalent because an ordinary example passes.

Prediction evidence remains available offline. Athlete reports can explain a miss
and affect confidence under the existing reasoning instructions. They do not
become measured evidence. Result linking and deterministic grading remain exact
revision-based library operations.

Backtest replays exclude both future events and revisions learned after the replay
cutoff. Preserve the first-result exclusion and benchmark matching. Tests include
late imports, corrected outcomes, changed definitions, deleted evidence, and
provider periods. A known old error is not preserved merely to match a snapshot;
each corrected behavior needs a smallest synthetic regression and an explicit note.

Runtime model clients retain Vercel and OpenRouter support, model discovery, saved
defaults, key precedence, and validation of model output. Network failures never
leave a partially written prediction. Backtests write their reports separately
from athlete data and retain individual model failures.

Keep the existing password-store entry names so saved keys remain usable. Keep
environment-variable support and refuse plaintext fallback. Test native credential
access on every supported operating system. Tests use fake keys and local model
servers. Automated checks do not spend tokens against a live model service.

## 11. Native CLI, assets, npm, and JavaScript

### 11.1 Native executable

Use Clap for command definitions and generated help. Keep named typed errors with
human-readable messages and structured field locations. Standard output contains
the requested result. Diagnostics and terminal progress use standard error.

Local commands run inside this executable and call the library directly. They do
not spawn `ath-core`, invoke a JavaScript interpreter, or find a system `sqlite3`.
Subprocesses for an explicitly requested package-manager update are separate from
athlete-data operations.

Embed shipped mappings, calculation definitions, skills, and the required runtime
assets in native release builds. An executable copied into an empty directory must
still be able to install skills and use its shipped defaults. Explicit installed
mapping and calculation overrides continue to win where the existing rules say
they win. Old tool-skill copies are replaced by the current tool skill. User-edited
calculation and mapping files are preserved.

### 11.2 npm distribution and client

Keep the `athleticstandard` package name and `ath` command. The npm package selects
a prebuilt native executable and forwards arguments, input, output, signals, and
exit status. It does not download an executable at runtime, compile on install,
or contain another command implementation. Pack the same tested executables used
for native downloads. Unsupported targets fail with a specific installation error.

The optional JavaScript client calls typed library operations through an explicit
versioned stdio interface in the same executable. This is a client boundary for
JavaScript callers. Native CLI operations do not call it. Each client operation
owns its worker process; no daemon, listening port, or persistent background service
is introduced. Large operations stream and use bounded backpressure. They do not
reuse the old 64 MiB buffered subprocess interface.

The interface exposes domain operations and checked data. It does not expose raw
SQL or the old action interpreter. Requests name a protocol version, request ID,
operation, and typed input. Responses distinguish results, errors, progress, and
callback requests. Refuse incompatible protocol versions before opening a database.
Only requested results reach a caller's normal output. Operation definitions
generate request types, response types, and client dispatch so transport names do not become a second
hand-maintained domain API. Callback-based operations such as injected backtest
completion use explicit request/response events during the same worker operation.
Cancellation and worker exit are tested, including cancellation during a write.

Keep the public root, `/database`, `/schema`, `/json-schema`, manifest schema,
measurement-series schema, and `/skill` subpaths. Preserve every public exported
name, signature, result, error contract, and generated `*T` type name. A client
handle enforces the existing closed-handle behavior. It does not open a second
local SQL connection in JavaScript. Each native request validates its target again.

Keep genuine Zod schema objects in an npm-only compatibility facade. Existing
`parse`, `safeParse`, composition, inference, and Zod error behavior continue to
work in process. Do not replace an in-process parse with a subprocess call.
Preserve the existing schema definitions as versioned compatibility code and
mechanically update only the permitted version declarations. Test accepted and
rejected inputs against the new Rust validator. The native executable does not
import or invoke this facade. Native validation remains entirely Rust.

Do not remove existing behavior for remote libSQL targets in this patch. Inventory
which public operations currently work and preserve those results and refusals.
Native local commands remain local-first. A remote compatibility adapter, if needed
for existing JavaScript calls, must not duplicate local record-write logic or add
a new supported remote operation. Preserve target parsing and explicit refusals;
never interpret a remote URL as a local filename.

Before implementation is complete, enumerate every currently exported symbol in
`src/index.ts`, `src/db/index.ts`, `src/v1/schema.ts`, and package exports. Map each
to its retained implementation and a compatibility test. Driver-specific and
maintenance helpers remain available through adapters where necessary. No public
symbol is removed or given an incompatible replacement in 2.1.1. Add compile-and-run
examples using existing consumer code, including Zod composition and inference.

### 11.3 Updates and installation ownership

Keep npm, pnpm, and bun global-update behavior through the matching package manager.
The launcher passes installation context to the native executable. Temporary
`npx`, `dlx`, and `bunx` runs keep their explicit behavior. A source checkout never
silently starts a build during a data command.

On Windows, the running native executable must exit before its package manager
replaces the installed binary. The launcher performs the requested update after
that exit, then runs the new executable to refresh skills. This handoff uses a
checked update result and argument list, not a shell command string. Test failure
and cancellation without leaving a partially replaced installation.

Native archive and Cargo installations receive precise update instructions from
`ath --update`. They are not mistaken for npm installations and do not install a
second copy. Building a separate native self-updater is outside this rebuild.
Refreshing local skills remains available through `ath skill install`.

## 12. Versions, migration, and rollback

The selected release is 2.1.1. Keep D152's single version for the program, npm
package, record package, and database metadata. SQL's integer remains the ordered
migration step. This plan does not reopen that version policy.

This is an implementation rebuild with patch-release compatibility. Keep the
athlete-owned fields, public JavaScript contracts, CLI behavior, and direct-SQL
contracts intact. New exports identify version 2.1.1 under the existing single-version
policy. Retained revisions keep their original content and authorship. Changes to
version declarations do not authorize changes to the record layout.

Keep published migrations 001–004 byte-for-byte. Make the SQL files themselves the
normative executable migration source; embed those files in Rust. Eliminate
generation of SQL from a separate TypeScript string. Contract metadata and its
checks still derive from the published SQL and declared public contract.

Add migration 005 for the 2.1.1 metadata and any internal pending-file-operation
state required by section 9. It does not rewrite existing athlete payloads, change
record IDs, backfill `field_basis`, collapse sources, or rebuild the public tables.
Fresh creation runs the same ordered chain as migration. Do not maintain a separate
fresh-schema implementation.

Support the existing readable database versions and SQL steps through the explicit
migration command. A 1.0.0 database passes through the existing repairs and ordered
steps. A 2.0.0 or 2.1.0 database preserves its complete history. Package restore
accepts the currently importable 2.0.0 and 2.1.0 packages plus 2.1.1. Supporting
0.x migration or adding older package formats is outside this rebuild.

Opening does not silently migrate. Preview and exact confirmation rules remain.
Use SQLite's in-process backup API and verify the resulting backup before writing
the migration. Bind the verified backup to the source database ID and revision.
Recheck that state under the migration write lock; a concurrent change requires a
new verified backup and preview rather than using a stale one.

Migration failures roll back the schema change. Preserve the verified backup and
report its location. Keep foreign application-ID and conflicting header refusals.
Do not repair a mismatched header during an ordinary open.

Rollback after successful migration means restoring the verified pre-migration
backup and keeping the newer database separately. Old software must refuse a
2.1.1 database. It must not attempt a best-effort write. Test this refusal with the
old executable. Explain that restoring a backup omits work recorded after that
backup; rollback never merges later work silently.

## 13. Proof that behavior survived

### 13.1 Fixed old/new comparison

Use the exact reference commit in section 2. Record the package version, Cargo
lockfile, fixture hashes, and tool versions. Run that version
in a separate temporary checkout. The new test suite must not import old source
files as a production dependency.

Capture valid and invalid examples for every record type, nested structure,
extension restriction, command, importer, and public API class. Preserve existing
fixtures. Add invented cases for gaps before changing behavior.

Compare old and new outputs at the strongest useful level:

- Exact package bytes and manifest hashes after only explicitly allowed version
  normalization in the comparison tool.
- Exact logical records, complete history, import keys, relationships, source facts,
  deletion state, and erasure markers.
- Exact deterministic grades and calculation receipts; documented numeric
  tolerances only for numerical routines whose method specifies that tolerance.
- Command argument and confirmation behavior, JSON field meanings, exit statuses,
  and generated help/reference consistency.
- Independent direct-SQL conformance and database integrity checks.

Do not erase `received_at`, authorship, or evidence references indiscriminately to
make comparisons pass. Inject clocks and ID generation where new output is expected
to be deterministic. Any remaining normalization is field-specific and explained.
Old/new agreement is evidence, not proof that an old bug should survive.

### 13.2 Required difficult cases

Test repeated and corrected imports, reordered duplicate groups, missing values,
deleted sources, mapping precedence, unsupported units, both HRV statistics,
provider dates, fractional offsets, clock changes, invalid dates, and timezone
changes. Include the previously observed offset-free GPX case without forcing the
whole test suite to UTC to hide machine-dependent behavior.

Test every record type through create, correction, history, export, restore, merge,
delete, and applicable erasure. Test historical evidence after later corrections.
Test child removal, link changes, grade revision retention, unknown fields, strict
schema refusal, malicious extensions, and Unicode field names.

Test all file-changing operations with failure injection. Include termination
before and after file publication, SQL commit, manifest publication, and attachment
removal. Reopen with a new process and check both usable data and pending cleanup.
Preview, refusal, stale confirmation, and cancelled operations must not leave
partial athlete-data changes.

Run native CLI tests on Linux, macOS, and Windows. Run npm client tests with Node
22 and 24. Test whitespace and non-ASCII directories, Windows drive paths, CRLF,
permission failures, closed handles, concurrent operations, and missing assets.
Use pseudoterminal tests for prompts and ordinary pipes for non-interactive tests.

Native artifact smoke tests run with Node, Cargo, `sqlite3`, and the source checkout
absent from their environment. They create a record, import synthetic provider data,
correct it, calculate, export, restore, migrate, repair, and check. The npm smoke
tests install the actual packed tarball into an empty directory with no compiler.

### 13.3 Existing test disposition

Map every existing test file to retained behavior. Port behavior tests into Rust or
executable-level tests. Keep thin JavaScript-client tests in the npm package.
Retain language-neutral fixtures and outputs once the comparison harness no longer
needs the legacy implementation. Remove tests that only exercise retired internal
TypeScript machinery only after their public behavior is covered elsewhere.

Do not compare test counts as a quality claim. Show which promises each suite
checks and report missing coverage explicitly. No old suite is deleted first to
make the new build green.

## 14. Proof that the rebuild is worthwhile

The architectural requirements in section 3 are mandatory and directly countable:
one native model, one encoder, one transaction owner per operation, no legacy
runtime format, no Node or external SQLite dependency for native local use, and
no second command implementation in npm. The npm-only Zod compatibility facade
is a required public adapter. It is not used by native commands.

Measure the same nine operations and the same deterministic 100,000-record and
3-million-record datasets used by the existing performance suite. Preserve
`performance/limits.json` as historical evidence. A new report records hardware,
operating system, SQLite and compiler versions, cache settings, query plans,
repetitions, latency, peak memory, output bytes, and output hashes.

Run old and new release builds on the same named runner with the same inputs and
cache conditions. Do not include Rust compilation in native command latency or
exclude old process startup from end-to-end comparisons. Report library query
latency separately from complete command latency. If the original fixture generator
has invalid format data, correct the invented case and run both versions against
that same corrected fixture.

Required performance outcomes:

- All nine existing review limits pass. A speedup in one operation does not excuse
  a regression beyond an existing limit in another.
- On the 3-million-record fixture, complete initial package import and complete
  database check each take at most half the old implementation's median elapsed
  time. This is a proposed release target, not a claim of achieved speed.
- Complete export is no slower than the old implementation beyond measured runner
  variation. Record that variation from repeated measurements instead of selecting
  a convenient single run.
- Bulk import, export, and check use at most 512 MiB peak native process memory on
  the existing synthetic fixtures, including the configured SQLite cache. Include
  helper-process memory if any helper is introduced. This is also a proposed target.
- Additional generated fixtures with the same record sizes and increasing counts
  show bounded working memory. Staging disk use may grow with input size and is
  measured. A very large individual valid record is measured separately and is
  not silently rejected just to meet the ordinary-fixture memory target.
- Cold native `--help`, `--version`, `stats`, and a latest-value query are measured
  end to end. They do not launch another application runtime or build themselves.

Use at least three complete runs for bulk comparisons and enough short-query runs
to report meaningful percentiles. If a required target is missed, investigate and
revise the implementation before release. A target change needs an explicit new
decision and measured evidence. Correctness tests remain mandatory.

Prove maintainability with two temporary change exercises: add a synthetic optional
field to an existing record, and add an invented provider reader that emits existing
record types. The first must use one encoder and the normal generators. The second
must need no new SQL writer or command implementation. Do not ship the invented
provider. Record the touched modules and remove the exercise changes afterward.

Source-line reduction is not a release gate. Less repeated responsibility is.
New Rust code that reproduces the old ownership split fails this plan even if it
has fewer files or runs faster.

## 15. Build execution and final cutover

Build on a feature branch from `main`. Keep logical commits reviewable. There is
one target release and one completion checklist; this plan does not split the work
into a series of incomplete product releases.

The dependency order is:

1. Freeze the reference, inventories, fixtures, semantic-rule list, and comparison
   runner. Record baseline performance before changing the implementation.
2. Build the typed model, checked values, validation, serializer, generated contracts,
   and independent SQL conformance checks.
3. Build the unified store, bounded queries, backup, migrations, repair, file safety,
   and recoverable file operations against the current public layout.
4. Connect every provider route and mapping to the typed import procedure. Complete
   package export, restore, merge, attachments, and conflict handling through the
   same encoder. Prove all-record round trips and interrupted-operation recovery.
5. Complete calculations, import-time arithmetic, historical evidence, grading,
   backtest, credentials, model calls, assets, and updates. These are required work.
6. Complete the native command tree, generated help, npm launcher, generated client,
   JavaScript compatibility examples, and packed-artifact checks.
7. Run old/new comparisons, operating-system checks, performance comparisons, and
   maintainability exercises. Resolve failures before removing the reference setup.
8. Switch the active source layout, remove retired implementations, regenerate
   current outputs, update all current documentation, and rerun every release gate
   on the final commit and the exact packed artifacts.

Temporary new source may live under `native/` while the old root `src/` remains
testable. The final layout in section 5 is required before acceptance. The temporary
tree is not installed or exposed as another product. A release candidate must not
select TypeScript or Rust according to which operation is implemented.

While the old build remains, every commit follows the existing required checks:
`pnpm check:public-data`, `pnpm typecheck`, `pnpm test`, `pnpm build`, and Rust tests.
Before deleting those commands, replace their responsibilities in the new checks and
update AGENTS.md in the same cutover. Public-data protection remains a required
check for Rust fixtures and all release artifacts.

The final check entry point runs formatting, Clippy, Rust tests, conformance vectors,
generator drift, public-data checks, npm type/client tests, and artifact smoke tests.
Performance runs remain explicit jobs for the required generated sizes. The release
job consumes results tied to the same commit and refuses stale or absent results.

Pin the Rust toolchain and lockfile for the release. Select the minimum supported
operating-system versions explicitly and test the native artifacts there. Bundled
SQLite does not eliminate platform requirements for libc, TLS, or credential stores.
Record those dependencies and verify that ordinary offline commands work without
an active credential service. Credentials and update commands have separate native
integration tests. Preserve license notices for embedded third-party components.

One release workflow builds and verifies native archives and the npm tarball from
the same commit. Pull requests execute its validation jobs without publication
credentials. Tag and manual release jobs reuse those jobs. Packaging does not rebuild
a different executable after testing. Only the checked tarball is eligible for npm
publication. Artifact hashes and embedded version metadata must agree.

Before cutover, remove the TypeScript database implementation, legacy runtime schema,
legacy signal conversion, duplicate Rust encoders, string action interpreter,
`ath-core` bridge, source-modification-time rebuilds, external SQLite backup calls,
and their obsolete build scripts. Retain preserved historical schemas, approved
synthetic fixtures, old published SQL, and append-only build history.

Update README, SPEC, AGENTS, CONTRIBUTING, current SQL instructions, performance
instructions, installed skills, and the JavaScript compatibility guide. The current
architecture document must explain one ordinary import and one correction from
input through durable storage. It must name each owner without requiring a reader
to reconstruct the decision history.

## 16. Review and release evidence

The final review includes:

- A complete command and public-export disposition with no unassigned entry.
- An invariant checklist mapped to passing tests, including direct-SQL tests.
- Old/new comparison results and a short list of deliberate behavior changes.
- Migration, failure recovery, and rollback results on supported synthetic versions.
- The before/after performance report and the two maintenance exercises.
- Native clean-install and npm packed-install results on all supported systems.
- A removal list showing that no production operation still requires legacy code.
- Generated-file checks, public-data checks, and documentation checks from the
  final release commit.

Private validation may use real exports outside Git with permission already granted
for those files. Public evidence uses only sanitized conclusions. No real health
data, exact workout dates, routes, personal details, or device inventories enter
commits, fixtures, command recordings, or CI artifacts.

The selected release target is 2.1.1. This specification creates no release, tag,
publication, or data migration by itself. Publication remains separate and follows
only after the exact replacement has passed the complete release requirements.

## 17. Primary references

- [Current domain schemas](../../src/v1/schema.ts) and
  [extension checks](../../src/v1/extensions.ts).
- [Current command implementation](../../src/program.ts) and
  [public database exports](../../src/db/index.ts).
- [Current SQL contract](../../spec/sql/contract.json) and
  [published migrations](../../spec/sql/migrations/).
- [Current import baseline tests](../../tests/import-baseline.test.ts),
  [direct-SQL tests](../../tests/v1-direct-sql.test.ts), and
  [performance definitions](../../performance/README.md).
- [2.1.1 decisions](decisions.md) and [progress](progress.md).
- [Schemars](https://docs.rs/schemars/latest/schemars/): schema generation from
  Rust types and Serde-compatible definitions.
- [rusqlite backup API](https://docs.rs/rusqlite/latest/rusqlite/backup/index.html):
  in-process SQLite backup, including its distinct connection requirements.
- [Clap](https://docs.rs/clap/latest/clap/): native argument definitions and help.
