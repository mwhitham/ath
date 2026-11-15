# v3.0.0 proposed decisions

Status: **proposed for review**. None of the entries below changes the currently
approved implementation rules until the rebuild specification is approved.
Numbers continue after D197 in [v2.1.0](../v2.1.0/decisions.md).

The implementation evidence and complete requirements are in [spec.md](spec.md).
Earlier decisions are not rewritten. These proposals identify the specific parts
they would supersede.

## D198. Proposed: one complete native replacement

The release builds one Rust library and one `ath` executable. All working features
must be present before cutover. The TypeScript database and command implementations
are removed from the active runtime.

This supersedes D174's TypeScript front end and the remaining implementation split
described in D155, D162, D163, D166, and D173. Implemented overlap in record writing
and import conversion is the new evidence. It is documented in spec section 2.

Rejected alternatives:

- Leave a permanent TypeScript front end with domain work on both sides. Rejected
  because it retains the ownership and distribution problems being addressed.
- Release a smaller Rust command set and finish the rest later. Rejected because
  it would leave existing workflows without a complete replacement.
- Translate files without consolidating responsibilities. Rejected because changing
  languages alone would preserve the duplication.

## D199. Proposed: typed Rust records define the current model

Rust record types and explicit semantic checks become authoritative at cutover.
Current JSON schemas and TypeScript types are generated from those definitions.
Language-neutral conformance examples cover checks a JSON schema cannot express.

This supersedes D83's choice of Zod as the current domain definition. SQLite and
JSON continue to encode the same athlete-owned records. D85's source separation
and D118's extension restrictions remain required.

Rejected alternatives:

- Maintain handwritten Zod and Rust definitions indefinitely. Rejected because
  every future field and rule would need coordinated edits in both languages.
- Treat deserialization as complete validation. Rejected because history, units,
  field relationships, and source restrictions need additional checks.
- Remove strict record validation to simplify the port. Rejected because that
  would weaken the format.

## D200. Proposed: one SQLite encoder serves every official operation

One encoder maintains typed data, complete JSON, child collections, and links.
Normal writes, provider imports, retained-history restore, and repair share it.
Each operation owns its complete transaction and expected-state checks.

This consolidates the implementations without replacing the public SQL layout.
D103, D104, D116, and D125 remain in force. Restore and repair retain their different
history semantics; they do not become ordinary corrections.

Rejected alternatives:

- Keep a separate writer per importer and restore route. Rejected because field
  maintenance would remain duplicated.
- Replace public typed tables with a generic JSON store. Rejected because it would
  break direct writers and discard the current indexed query contract.
- Expose low-level action arrays to callers. Rejected because the library should
  own validation, revision changes, and transaction completeness.

## D201. Proposed: bundled rusqlite owns local database operations

Use bundled SQLite through rusqlite for all local database operations and verified
backups. Native local commands need no external SQLite executable or JavaScript
database driver.

This supersedes D93's libSQL adapter choice and the external-backup implementation.
D124's preview, backup verification, and confirmation requirements remain in force.
Direct-SQL access remains supported.

Rejected alternatives:

- Keep both local database drivers. Rejected because it preserves duplicate
  connection behavior and platform-specific cleanup workarounds.
- Remove verified backups. Rejected because simpler installation does not justify
  weaker migration and repair behavior.
- Add a database service. Rejected because local use requires no service.

## D202. Proposed: SQL files are the executable migration source

Published SQL migrations 001–004 remain byte-identical and are embedded directly
in the new library. Migration 005 carries 3.0.0 metadata and required internal
recovery state. Fresh creation executes the same ordered migrations.

This replaces TypeScript migration strings as the source that generates SQL.
D165's preservation of published migrations remains in force. D152 and D184's
single software/record/database version policy remains in force.

Rejected alternatives:

- Rewrite published SQL to match a cleaner fresh schema. Rejected because existing
  databases and direct writers depend on the published migration chain.
- Keep generated SQL and independently maintained Rust SQL. Rejected because it
  would introduce another source of drift.
- Split software and record version numbers during the rebuild. Rejected because
  the implementation does not require reopening the approved version policy.

## D203. Proposed: imports produce current records through one procedure

All provider readers produce checked current-record candidates. Shared code owns
source resolution, staged preview, matching, revisions, and atomic commit. Provider
matching rules remain route-specific. Package restore keeps its retained-history
semantics while sharing the record encoder.

This removes the legacy signal intermediate without changing D97, D108, D112, or
D177–D196's import meaning. Large inputs use private bounded staging rather than
retaining all candidates in memory.

Rejected alternatives:

- Keep 0.x signal objects as the internal interchange format. Rejected because
  current records would still depend on a retired format.
- Force every provider to use one matching recipe. Rejected because their IDs,
  dates, cycles, and duplicate groups differ.
- Let previews and writes calculate different changes. Rejected because a person
  must approve the operation that will actually be applied.

## D204. Proposed: native distribution and a thin npm client

Ship native `ath` downloads and retain npm installation. The npm launcher invokes
the same tested executable. An optional JavaScript client uses typed operations
over a versioned stdio interface. It contains no SQL or domain implementation.

This supersedes D167 and D170's `ath-core` child packaging. The same-artifact
testing rule and D171's checked-tarball publication requirement remain.

Retain supported domain calls, generated types, package subpaths, and validator
`parse`/`safeParse` conveniences. Remove Zod-specific composition and remote libSQL
targets in the major release with an explicit migration guide. Inventory every
public export before cutover; accidental export loss is not permitted.

Rejected alternatives:

- Drop npm installation or JavaScript callers without a replacement. Rejected
  because those are existing supported ways to use the project.
- Keep handwritten JavaScript validation for compatibility. Rejected because it
  would leave two current models.
- Add a daemon or a second HTTP service for JavaScript callers. Rejected because
  the native worker can perform each complete requested operation locally.
- Promise full Zod object compatibility through a different validator. Rejected
  because that would hide a real major-version change.

## D205. Proposed: retain editable files and isolate optional network behavior

Keep current mapping and calculation languages, override rules, and installed
file preservation. Parse them into typed settings. Preserve model calls, keys,
model selection, Garmin pull, and backtest behavior through narrow runtime modules.
Local data operations do not depend on those optional services.

D157–D161 and D177–D196 remain in force except where the specification explicitly
adds calculation-definition receipt metadata. No generic execution language or
runtime plugin system is introduced.

Rejected alternatives:

- Compile every editable method into fixed code. Rejected because editing a
  calculation or mapping is an approved user capability.
- Postpone model and credential behavior until after release. Rejected because
  the native release must replace the complete working command set.
- Turn configuration into arbitrary executable code. Rejected because reviewed
  operations and typed parameters meet the existing requirements.

## D206. Proposed: file changes have explicit recovery states

Use bounded private staging, synchronized publication, and narrowly scoped recovery
metadata for operations that touch SQLite and attachment files. A committed record
must not point at a missing newly imported file. Erasure is not complete while its
attachment removal remains pending. Read-only commands report pending work without
performing it.

This makes D91, D92, D94, D103, and D151's existing guarantees explicit across
process termination. It does not promise erasure of external backups, filesystem
snapshots, or device history.

Rejected alternatives:

- Call filesystem writes and a SQL commit one atomic transaction. Rejected because
  the operating system and SQLite do not provide that combined transaction.
- Delete WAL sidecars to recover from errors. Rejected because committed writes
  may still depend on them.
- Hide incomplete attachment cleanup. Rejected because it would misstate whether
  an approved erasure finished.

## D207. Proposed: release requires proof of completeness and improvement

Require complete old/new behavior comparisons, all-record round trips, independent
SQL conformance, failure recovery, cross-platform packed-artifact tests, and the
measured targets in spec section 14. The final native runtime has one model and
one encoder. The TypeScript domain implementation is removed before acceptance.

This carries D95 and D114's measurement requirement into the rebuild and adds
specific proposed improvement targets. It does not claim those targets are met.

Rejected alternatives:

- Accept the rebuild because it compiles or uses fewer source lines. Rejected
  because neither proves preserved behavior or simpler maintenance.
- Relax tests to meet performance targets. Rejected because the record's
  guarantees remain the reason to use the software.
- Leave temporary old/new selection in the release. Rejected because that would
  leave the main architectural problem unresolved.
