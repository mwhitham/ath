# Athletic Standard 2.1.3 — an optional host for sharing a record

Status: **approved direction; not built; not released**. Nothing in this plan runs a
hosted service, publishes a package, or lists a skill.

Today a record lives in one file on one machine. An athlete who works with a coach has
no supported way to let the coach read selected training, or add a note, while the
athlete's device is off. Version 2.1.3 adds an optional host program, `ath-host`. It
serves many athletes' records through a documented HTTPS API. Each athlete keeps a
complete copy that opens offline in ordinary `ath`. Local use does not change and needs
no account (D75, D254).

The program, the npm package, the record package, and the database metadata use 2.1.3
(D254). The SQL migration step becomes 7.

The research behind the storage choices is in
[hosted-storage-options.md](appendix/hosted-storage-options.md). The local experiments
are in [hosting-experiments.md](appendix/hosting-experiments.md).

## 1. The result

| Person | What they can do | What success looks like |
|---|---|---|
| Athlete using `ath` alone | Nothing changes. | It works offline with no account. |
| Athlete who connects a host | Upload an existing record, keep a device copy, and refresh it. | Records, history, series, and attachments arrive. The device shows when it last refreshed. A refresh never overwrites local work silently. |
| Athlete working with a coach | Grant a named coach, through a named app, any of three permissions. Revoke them. | The coach sees only what was granted. The next request after a revocation fails. Past coach notes keep their author. |
| Coach | See a roster of the athletes who granted access. Read workout summaries. Add notes and programming. | The coach cannot correct, delete, or export the athlete's history. |
| Host operator | Run the host on one machine or on several, with either storage backend. | Setup, backup, and restore work from the written guide on a clean machine. |
| App developer | Sign a person in, connect to their host, and read and write within grants. | Two independently registered apps use the same record. The athlete can disconnect either one. |
| Athlete leaving a host | Export the complete record, or move it to another host. | The copy opens in stock 2.1.3. Old credentials give no access at the new host. |

## 2. Scope

In 2.1.3:

- Human coach authorship in the record (D255).
- A storage interface in the library, with a native SQLite backend and a libSQL remote
  backend (D257, D258).
- Attachments in a local directory or an S3-compatible bucket (D259).
- `ath-host`: sign-in, grants, the versioned API, write receipts, device refresh, moves
  between hosts, backups, and limits (D256, D260–D265, D267).
- Clients: a host connection in the npm package, a Swift example, and a small MCP server
  (D266).
- `docs/hosting.md`, a guide for operators and developers written from the storage
  study.
- A container definition and a setup guide for one machine.

Not in 2.1.3:

- A running hosted service. The build ships software and guides.
- Automatic merging of offline edits made by several people (D261).
- Direct database sync between a device and a host database (D261).
- Postgres or any other non-SQLite backend (D257).
- Per-athlete encryption keys on the native backend (D264).
- Changes to calculation methods, skills, or the model gateway key.

## 3. Who owns what

| Part | Owns | Does not own |
|---|---|---|
| Library (`src/`) | The record model, validation, revisions, the single encoder, migrations, export and restore, the storage interface and both backends. | Accounts, grants, network requests. |
| `ath` CLI | Local commands, unchanged. Built without the remote backend. | Hosting. |
| `ath-host` | Token checks, grants, the API, receipts, refresh, moves, backups, and limits. It calls the library for every record operation. | Record rules. It never writes record tables with its own SQL. |
| Clients | The sign-in flow, the user interface, and API calls. | Permission decisions. The host makes them. |

There is one set of record rules (D210). The host and every backend use it.

## 4. The record in 2.1.3

### 4.1 Coach authorship (D255)

`RecordActor` and `AuthoredRecordAuthor` gain:

```json
{ "author_type": "coach", "coach": "Display name when written", "coach_id": "6f1c…" }
```

The host that accepted the write assigns `coach_id`, an opaque UUID. A coach may author
`coaching_note`, `training_plan`, and `planned_session` records and their revisions. A
coach cannot author a measured record or an athlete report. The two-tier wall holds.

The Rust model, semantic checks, JSON Schema, npm protocol declarations, and Zod
compatibility schemas gain the variant. The SQL `author_type` columns have no CHECK
constraint, so no table is rebuilt.

### 4.2 Migration 007

`007-version-2-1-3.sql` sets the metadata to 2.1.3 and SQL schema 7. It adds four
internal tables and changes no athlete-owned row (D211):

- `internal_change_log`: one row for each record revision, deletion, and erasure, with
  the database revision that committed it. Triggers fill it in the same transaction, so
  direct SQL writers are included. The host reads it to answer "what changed since
  revision N" (D263).
- `internal_write_receipts`: one row for each accepted host request: actor, app,
  operation, request ID, a keyed digest of the request, and the resulting IDs and
  revisions. It holds no request body and no health values (D261).
- `internal_access_state`: one row holding the access epoch (D262). Revoking a grant
  raises it.
- `internal_replication_log`: in a database a host keeps on a remote server, the
  changeset of each committed write, with the revision it started from (D268). An
  erasure deletes every row. It stays empty on a local file.

None of the four is exported in a package. All are listed in `spec/sql/contract.json`
as internal, like `internal_pending_file_operations`. A local database that never
connects to a host uses only the change log and the access row.

On the native backend, the migration keeps the preview, verified backup, confirmation,
and rollback rules (D124, D211). On the libSQL backend, a provider restore point and a
complete package export replace the verified backup (D258).

### 4.3 Versions

- Records and packages written as 1.0.0 through 2.1.2 stay valid. Packages from 2.0.0
  onward still import.
- A package that contains a coach author declares 2.1.3. A 2.1.2 program refuses it
  with its existing message.
- A 2.1.2 program refuses a 2.1.3 database by its SQL step.
- `schema/2.1.3/` is generated. `schema/2.1.2/` is not rewritten.
- SPEC.md section 13 records the version-rule exception (D254).

## 5. Storage

### 5.1 Two backends (D257, D268)

| Backend | Where the athlete database lives | How a write reaches it | Built into |
|---|---|---|---|
| Native | A local SQLite file, opened through bundled rusqlite. | The library writes it directly. | Every build. |
| libSQL remote | A database on a server that speaks the libSQL protocol. | The library writes a local working copy. The host commits the changed rows to the server in one transaction. | `ath-host` only, behind its `libsql-remote` Cargo feature. |

The library stays synchronous and keeps one code path. It gains a way to run a write
while recording its changed rows with SQLite's session extension. That is behind the
library's `changesets` Cargo feature, which only `ath-host` turns on. The CLI gains no
network code and no dependency.

### 5.2 The libSQL backend (D258, D268)

**Working copies.** The host keeps a working copy of each active athlete's database on
local temporary disk. It builds the copy by copying every table from the server. It
keeps the copy current from `internal_replication_log` in the remote database. It
rebuilds the copy when the log has a gap, deletes idle copies after a set time, and
deletes all copies when it starts.

**A write.**

1. Bring the working copy to the server's current database revision.
2. Run the library operation on the copy while recording its changeset.
3. On the server, in one transaction: check the database revision and access epoch,
   apply deletions (children first), insertions (parents first), then updates, with
   the database revision raised last; append the changeset to the replication log.
4. If the check fails, discard the copy's change, refresh, and run the request again.
   The receipt makes the second run safe.

An erasure deletes the replication log in the same transaction.

**Differences from local SQLite.**

- `application_id` and `user_version` are read but never written on the server. The
  metadata table is authoritative (D93).
- `journal_mode` and `busy_timeout` are not sent. The server manages both.
- The backend refuses to open a database where a dangling reference is not rejected.
- The SQLite online backup API is not used on the server. Section 4.2 says what
  replaces it before a migration.
- A large write is sent in batches inside its one transaction.

### 5.3 Attachments (D259)

- **Directory store:** the folder beside the database, as today.
- **Object store:** any S3-compatible bucket, under `<resource-id>/attachments/<sha256>`.
  An upload goes to a staging key. The host checks length, media type, and SHA-256,
  commits the record that names the file, then moves the file to its final key.
  Downloads use short-lived signed links issued after the permission check. A cleanup
  job removes staged files that no committed record names.

Attachments are never database BLOBs.

### 5.4 One database per athlete (D256)

Each athlete has one database in every backend. The host directory (principals, grants,
registered clients, record locations, routing) is a separate database on the same
backend. A request names a host-assigned resource ID. The host resolves it after the
access check.

### 5.5 Choosing a backend

| | Native directory | libSQL remote |
|---|---|---|
| Runs on | One machine with a persistent disk. | Any machine with network access, plus a libSQL server or service. |
| Copies of the host | One per disk. More machines need routing (section 6.1). | Any number. |
| Athletes per host | Limited by disk size. | Limited by the provider. |
| Backups | Litestream, attachment copies, scheduled package export. | Provider restore, attachment bucket copy, scheduled package export. |
| Per-athlete keys | Not in 2.1.3. | Where the provider supports them. |
| Each operation | In process. | In process on a working copy, plus a few round trips to commit. |
| Vendor needed | No. | No, if you run a libSQL server yourself. |

The [study](appendix/hosted-storage-options.md) section 6 gives the questions to ask.

## 6. The host

### 6.1 Modes

**Native directory mode.** One host process owns a data directory:

```
<data>/host.sqlite                      directory: principals, grants, clients, locations
<data>/athletes/<resource-id>/athlete.ath.sqlite
<data>/athletes/<resource-id>/attachments/
<data>/staging/                         uploads and imports before validation
```

A process lock on each athlete directory allows one writer. Several machines can each
own a set of athletes. The directory names the machine that serves each athlete, and a
request for another machine's athlete is forwarded or redirected. Only one machine
accepts writes for an athlete at a time (D267).

**Remote mode.** Host copies keep no durable record data, only working copies that can
be deleted at any time (D268). Any copy can serve any athlete. The libSQL server orders
writes to each database. The revision check, receipts, and the access epoch handle
races between copies (D261, D262).

### 6.2 Sign-in (D260)

- One configured OpenID Connect provider. The host checks issuer, signature, expiry,
  audience, and client, and maps issuer and subject to a principal.
- Native apps use the system browser with authorization code and PKCE (RFC 8252).
- The host publishes protected-resource metadata (RFC 9728) for CLI, mobile, and MCP
  clients. MCP clients follow the MCP authorization specification.
- Tested recipes: WorkOS AuthKit and Keycloak, with the same API tests.
- `ath-host setup` claims the first owner once, on the machine. Athletes accept their
  own invitations. A coach's invitation cannot make anyone an owner.
- Recovery uses the provider plus an owner recovery procedure the operator documents.
  Recovery rotates sessions.

### 6.3 Grants (D262)

| Grant | Allowed | Excluded |
|---|---|---|
| Read workout summaries | List current workouts: record ID, revision, start and end, activity type. | Routes, descriptions, raw samples, source IDs, reports, attachments, extensions, full record JSON. |
| Add coach notes | Append a note with optional title. Evidence links only to revisions visible under the read grant. | Reading other notes, correction, deletion. |
| Add programming | Create plans and planned sessions with schedule and blocks. Link sessions only to plans this coach and app created for this athlete. | Personal records, completion flags, completed workout links, deletion, editing existing plans. |

- Nothing is selected until the athlete approves.
- The owner keeps full reads, exports, corrections, imports, deletion, and erasure.
- Lists are filtered by grant before counting, sorting, and paging. Attachments, links,
  evidence, and derived values follow the same rule.
- Errors, IDs, counts, and timing do not reveal another athlete's records.
- Caches are keyed to the grant version or not shared.
- Revocation raises the access epoch. A write ordered after it fails. Data already
  delivered cannot be recalled.

### 6.4 Writes and retries (D261)

Example: a coach adds a note.

1. The app sends `POST /v1/athletes/{id}/notes` with an `Idempotency-Key` header.
2. The host checks the token and the grant, and reads the access epoch.
3. The library commits the note, its first revision, the change-log row, and the receipt
   in one transaction. The commit fails if the epoch changed.
4. The response has the record ID and revision. Repeating the request returns the same
   result. The same key with a different body is a conflict.

Corrections name the expected revision. A stale correction is refused, and the client
keeps the person's version for review. Imports keep the whole-database revision check
and an owner-only preview bound to the file digest and database revision.

### 6.5 API version 1

Operations: host capabilities; owner connect, export, refresh, log, correct, import
preview and apply, delete, erase; grant list, create, and revoke; coach roster, workout
summaries, note, and programming; attachment upload and download link; move steps
(D267).

Stable error codes: expired sign-in, missing permission, unknown resource, stale
revision, request-ID conflict, host moving, limit reached.

The description is generated from the Rust definitions into `schema/host-api/1/`, and
`cargo test --test generated` checks it (D266).

### 6.6 Device refresh (D263)

1. The first connection downloads a verified full package.
2. Later refreshes ask for changes since the device's database revision. The host reads
   `internal_change_log` and returns record revisions, deletions, and erasures in
   order, with any new attachments.
3. Before applying, the device checks that its file still matches the manifest digest it
   last verified. If not, it keeps the local file, downloads the hosted copy beside it,
   and asks the owner.
4. The library applies the changes with revisions unchanged. Attachments are checked by
   SHA-256.
5. The device computes the new manifest digest and compares it with the host's.
6. A gap or failure falls back to a full package.

A managed app directory switches complete copies atomically. A file other programs may
open gets a verified sibling copy and a coordinated handoff.

### 6.7 Moving between hosts (D267)

| Step | Who accepts writes | On failure |
|---|---|---|
| Owner connects an empty destination. The source may pre-copy files. | Source | The destination cannot show the staged record. |
| Owner freezes the source. It finishes accepted work and signs its final manifest digest and transfer ID. | Neither | Before release, the owner can abort and reopen the source. |
| The destination verifies every file, restores privately, runs `ath check`, and reports ready. | Neither | Resume from the last verified step. Show which host is waiting. |
| The source retires its writer and records a single-use activation for this transfer, destination, and digest. The destination activates once. | Destination | A lost reply is retried with the same transfer ID. The source cannot reopen by itself. |
| The device reconnects and the owner reviews grants. | Destination | Keep an owner recovery copy until cleanup is confirmed. |

A restore from a device copy when the source cannot be reached is a separate action. It
is labelled a recovered fork.

### 6.8 Backups and restore (D264)

- **Native:** Litestream for each athlete database. Attachment copies on upload. A
  scheduled package export of each athlete to storage with separate credentials. The
  host keeps connections open and avoids truncating checkpoints while Litestream runs.
  The recipe is published after the test in step 8 passes.
- **Remote:** the provider's per-database restore, a copy of the attachment bucket, and
  the same scheduled package export.
- **Every restore:** one athlete, isolated first, sessions invalidated, later erasures
  and revocations replayed from ordered recovery records before serving. If those
  records are missing, the restore waits for the owner.
- **Erasure:** the host records an erase intent before applying it, replays unfinished
  intents after a failure, and applies erasure to staged copies.

### 6.9 Security and trust (D264)

- Whoever runs the host and its storage can read records. The connect screen names who
  runs the host and the storage.
- TLS on every connection. Storage encryption at rest is the provider's or the
  operator's.
- Per-athlete keys on the libSQL backend where the provider supports them. Keys come
  from a key service the operator configures. They are never stored in the directory
  database.
- Backup credentials are separate from the host's credentials.
- Logs hold no credentials, request bodies, or health values.

### 6.10 Limits

For each athlete: total and expanded upload bytes, page size, job duration, concurrent
jobs, and temporary disk. Uploads expire and can be cancelled. Jobs have opaque IDs, and
only the requester can read their results. A large job for one athlete must not delay a
small write for another.

## 7. Clients (D266)

- **npm:** a host client with an explicit host URL and token. Local behavior and
  [REMOTE-TARGETS.md](../../packages/npm/REMOTE-TARGETS.md) do not change.
- **Swift:** an example client for macOS and the iOS simulator: sign-in, roster, read a
  workout summary, add a note.
- **MCP:** a small server with `list_workout_summaries` and `add_coaching_note`. It acts
  with the signed-in person's grants and uses MCP authorization.
- **Starter:** a JavaScript example app that does the same flow.

## 8. Qualification suite (D258)

A backend or provider is listed as supported after a named run of this suite passes:

1. Migrations 001–007 run. STRICT tables, triggers, and foreign keys are enforced. A
   dangling reference is refused on the remote connection.
2. The library's write tests pass on the backend: create, correction with stale
   detection, deletion, erasure, import preview and apply, receipts.
3. Two writers correct the same record from the same revision. One conflicts.
4. With identical record IDs in two athletes' databases, neither athlete's request
   reaches the other. Where the provider has database-scoped credentials, one athlete's
   credential cannot open another's database.
5. An export from the backend equals a native export of the same content, byte for byte.
6. Restoring one database to an earlier moment leaves every other database unchanged. A
   package restore onto a clean machine passes `ath check`.
7. A network drop during a transaction leaves no partial write. A retry with the same
   request ID returns the original result.
8. Timings for a workout save, a 28-day evidence read, and an import of the
   100,000-record workload, with region, hardware, and crate versions.

First runs: a local libSQL server in continuous integration, and Turso Cloud
libSQL-engine databases in a manual run.

## 9. Verification

Before any commit, the four checks in [AGENTS.md](../../AGENTS.md) pass. The build adds:

- Host tests with two athletes, two coaches, two apps, revocation during a queued write,
  and guessed IDs.
- The qualification suite against a local libSQL server.
- Fault tests: stop the process around a note commit, an attachment publication, an
  import, each move step, and a restore. Disk full. Staging cleanup after restart.
- Scale runs with the 100,000- and 3-million-record workloads: peak memory, disk
  high-water mark, first transfer, unchanged refresh, one-note refresh, and one-athlete
  restore time. A small write for a second athlete completes during a large job.
- Regenerated files: `schema/2.1.3/`, `schema/athleticstandard.schema.json`,
  `schema/host-api/1/`, `packages/npm/src/generated/protocol.ts`,
  `skills/athletic-standard/references/cli.md`, and the demo database.

## 10. Build order

Each step ends in something a person can do. A screenshot, a mocked provider, or a
successful sign-in alone does not finish a step.

| Step | Work | Done when |
|---|---|---|
| 1 | This plan, D254–D267, and the storage study. | Approved. |
| 2 | Version 2.1.3, migration 007, coach authorship. | A coach-authored note survives export and restore. A 2.1.2 program refuses the database. All existing tests pass. |
| 3 | Changeset capture in the library (D268). | A write recorded as a changeset and applied to an empty copy gives the same export, byte for byte. Every existing test passes with no change in CLI behavior. |
| 4 | The libSQL remote backend: working copies, guarded commits, the replication log, and the qualification suite. | The suite passes against a local libSQL server in continuous integration. |
| 5 | `ath-host` in native directory mode: owner connect, read, write with receipts, attachments, device refresh, restore. | An owner saves a report with a photo on one device and opens both on another. |
| 6 | Sign-in and grants: both recipes, three grants, access epoch, roster. | Two coaches, two apps, two athletes. A revocation during a queued write blocks it. |
| 7 | Remote mode with the object store, and several host copies. | Steps 5 and 6 pass in remote mode. A named run of the suite passes against one provider. |
| 8 | Moves and backups: the move protocol, the Litestream recipe, scheduled exports, restore drills. | A record moves native to remote and back. One athlete is restored without changing another. |
| 9 | Clients and guides: npm host client, Swift example, MCP server, starter, `docs/hosting.md`, container and setup guide. | A developer follows the guide from a clean checkout and completes the flow. |
| 10 | Scale and fault runs, release evidence. | The report lists which backends and providers passed and which clients ran. |

## 11. References

- [Storage options and how to choose](appendix/hosted-storage-options.md)
- [Hosting experiments](appendix/hosting-experiments.md)
- [Architecture](../../docs/architecture.md) · [Store](../../src/store/mod.rs) ·
  [Attachments](../../src/files/attachments.rs) · [Export](../../src/package/export.rs) ·
  [Restore](../../src/package/restore.rs) · [Erasure](../../src/package/erase.rs)
- [OAuth 2.0 for native apps (RFC 8252)](https://www.rfc-editor.org/rfc/rfc8252) ·
  [Protected resource metadata (RFC 9728)](https://www.rfc-editor.org/rfc/rfc9728) ·
  [MCP authorization](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization)
- [Turso Rust reference](https://docs.turso.tech/sdk/rust/reference) ·
  [Turso Cloud limitations](https://docs.turso.tech/cloud/limitations) ·
  [Litestream](https://litestream.io/guides/directory/)
