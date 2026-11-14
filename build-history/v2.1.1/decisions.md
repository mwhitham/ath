# v2.1.1 rebuild decisions

The complete native rebuild targets version 2.1.1. The specification is
[spec.md](spec.md). Numbers continue after the unapproved D198–D207 proposals in
[the earlier draft](../v3.0.0/decisions.md). That draft remains in build history.

## D208. The complete native rebuild ships as 2.1.1

The program, npm package, record package, and database metadata use 2.1.1 for this
rebuild. The selected version replaces the unapproved 3.0.0 proposal. D152's single
version rule remains in force.

The release still requires the complete Rust library and native command. It must
replace every working command before acceptance. It must remove duplicate local
writers, legacy import intermediates, and the TypeScript command implementation.
The compatibility, recovery, cross-platform, and performance requirements remain.

This supersedes D174's TypeScript front end and the remaining implementation split
described in D155 and D162. The implemented overlap is the new evidence, documented
in spec section 2. The implementation is replaced without redesigning the public
athlete record.

Rejected alternatives:

- Keep the proposed 3.0.0 version. Rejected because 2.1.1 is the selected release.
- Reduce the rebuild to a smaller cleanup. Rejected because the complete native
  replacement is still the selected goal.
- Ship an incomplete native command and finish the rest later. Rejected because
  current workflows require a complete replacement.

## D209. The patch preserves existing public contracts

Version 2.1.1 retains public commands, arguments, confirmations, JavaScript exports,
Zod schema behavior, record fields, and the direct-SQL contract. The draft's
proposed API removals and calculation-receipt additions are not selected.

Preserve existing remote-target behavior and existing unsupported-operation
refusals. The rebuild adds no remote capability. Every current public export maps
to its retained implementation and a compatibility test.

The npm package retains genuine in-process Zod objects as versioned compatibility
code. Their existing composition, inference, parsing, and error behavior remain.
The native implementation validates in Rust and never calls this compatibility
code. Tests compare the frozen public schema behavior with the Rust checks. This
preserves existing callers without a JavaScript implementation of local storage.

Rejected alternatives:

- Apply the proposed major-version API removals under a patch number. Rejected
  because existing callers must continue to work.
- Replace synchronous schema parsing with a child-process call. Rejected because
  that would change both behavior and ordinary validation performance.
- Retain the TypeScript local database implementation for compatibility. Rejected
  because public adapters can call the new library without duplicating its rules.

## D210. Rust owns the native record lifecycle

Rust types and semantic checks own native validation. One encoder maintains typed
rows, complete payloads, child collections, and relationships. Creation, correction,
imports, restore, and repair share it while keeping their distinct history rules.
Bundled SQLite through rusqlite owns local operations and verified backups.

This replaces D83's native dependence on Zod and D93's local libSQL adapter.
The npm compatibility requirement in D209 is explicit. D85, D103, D104, D116,
D118, D124, and D125's record and database protections remain in force.

The native executable needs no Node runtime, external SQLite executable, compiler,
or service for local use. npm continues to install the same tested executable and
provides adapters for existing JavaScript callers.

Rejected alternatives:

- Keep several encoders with the same field knowledge. Rejected because the
  rebuild must remove the implemented duplication.
- Replace public typed tables with one generic JSON table. Rejected because that
  would break direct writers and the indexed query contract.
- Remove source separation or revision checks to simplify the port. Rejected
  because the record guarantees remain required.

## D211. Migration changes metadata and internal state without changing athlete fields

Published migrations 001–004 remain byte-identical. Migration 005 records 2.1.1
and any internal file-operation recovery state required by the specification.
It does not change existing athlete-owned fields, record IDs, or revisions.
It does not backfill field_basis or collapse sources.

Keep preview, verified backups, explicit confirmation, and rollback behavior.
Use the in-process SQLite backup API. New creation runs the same ordered migration
chain. The published SQL files become the executable migration source.

This retains D165's protection of published migrations. It does not change the
meaning of the integer SQL migration step. Retained revisions keep their original
contents and authorship, and existing supported packages remain importable.

Rejected alternatives:

- Rewrite earlier SQL files. Rejected because they are published contracts.
- Skip migration safety because this release has a patch number. Rejected because
  metadata and internal storage changes still need a verified backup.
- Redesign athlete-owned fields during the rebuild. Rejected because the selected
  work is an implementation replacement with compatible public behavior.
