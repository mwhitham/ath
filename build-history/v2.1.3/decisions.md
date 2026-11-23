# v2.1.3 decisions

Decisions continue after D253. Earlier decisions stay as they are. The plan is
[spec.md](spec.md). The storage research is in
[hosted-storage-options.md](appendix/hosted-storage-options.md), and the local
experiments are in [hosting-experiments.md](appendix/hosting-experiments.md).

## D254. Sharing a record ships as 2.1.3, through an optional host program

The program, npm package, record package, and database metadata use 2.1.3. D152's
single version rule stays. The SQL migration step becomes 7. The maintainer chose
2.1.3 for this work on 2026-11-22.

Sharing is a separate executable, `ath-host`. It calls the library for every record
operation. The `ath` CLI is built without it and works as before: one local file, no
account, no network (D75). Running a host is a choice an operator makes.

Version 2.1.3 adds one author variant (D255) and three internal tables. The published
version rule says a patch adds no fields (D96; SPEC.md section 13). As with D227,
this is an exception made by the maintainer's choice. SPEC.md section 13 records it.
Every 2.1.2 record and package stays valid 2.1.3 data. A 2.1.2 program refuses a
2.1.3 database by its SQL step.

This ends the deferral of sharing in D96. `ath share` stays a placeholder. Sharing
happens through a host, not through a command that sends files.

Rejected alternatives:

- Put the host inside `ath`. Rejected because the CLI would gain network code,
  sign-in, and dependencies that local users never need.
- Use 2.2.0, which the version rule suggests. Not chosen: the maintainer chose 2.1.3.
  The change is mechanical if it is made later.
- Share by sending files between devices. Rejected because a coach must be able to
  read and add while the athlete's device is off.

## D255. A coach is a record author

`RecordActor` and `AuthoredRecordAuthor` gain a `coach` variant with two fields:
`coach`, the display name when the record was written, and `coach_id`, an opaque
UUID assigned by the host that accepted the write.

A coach may author `coaching_note`, `training_plan`, and `planned_session` records
and their revisions. A coach cannot author a measured record or an athlete report.
The two-tier wall holds: coach work is authored advice, like agent advice (D228).

The name and ID are history, not credentials. A host grants rights only from its own
sign-in. A matching name or ID at a new host gives no access.

Notes and plans must keep their author after the athlete changes coach or host.
Today a coach's note can only be labelled as written by the athlete, an agent, or a
tool. All three are wrong.

The SQL `author_type` columns have no CHECK constraint, so no table is rebuilt.

Rejected alternatives:

- Store coach identity only on the host. Rejected because the author would be lost
  when the record leaves the host.
- Record the coach as an agent. Rejected because it misstates who wrote the advice.
- Use the coach's email address as the ID. Rejected because email addresses change
  and identify a person outside the record.

## D256. One database per athlete in every hosted mode

Every storage backend keeps each athlete's record in its own database. The host's
directory of accounts, grants, clients, and record locations is a separate database.
A request names a host-assigned resource ID. The host resolves it to a database only
after the access check. A record's database ID is not proof of ownership.

A database is already the unit the library validates, migrates, exports, restores,
and erases. Keeping it as the unit of hosting means one athlete can be restored,
moved, or deleted without touching another. A fault in one athlete's database cannot
change another's rows. It also matches how the services in the study scale: by number
of databases, not size of one database.

Rejected alternatives:

- One shared database with an athlete column on every row. Rejected because every
  query would need a filter that the schema does not have, a restore would roll back
  every athlete, and the package format is per athlete.
- Several athletes in one directory. Rejected because the library places attachments
  in a folder beside the database, so athletes would share it (experiment 1).

## D257. The library gets a storage interface with two database backends

The store's SQL calls go through one internal interface in `src/store/`. Two backends
implement it in 2.1.3:

- **Native**: a local SQLite file through bundled rusqlite. Every build has it.
- **libSQL remote**: a database on a server that speaks the libSQL protocol, through
  the `libsql` crate. Only `ath-host` has it, behind the `libsql-remote` Cargo
  feature.

The library stays synchronous. The host runs storage work on blocking worker threads.
The record rules stay in one place: the encoder, the revision checks, and the
migrations are the same code on both backends.

Native is the reference. It needs no new dependency, every existing test covers it,
and anyone can run it on one machine.

libSQL remote is the second backend because it is the only remote family that meets
the study's requirements R1–R6 together: SQLite rules, unlimited databases per
account, restore of one database, a Rust client from any server, and keys supplied
by the operator. One crate reaches three services (Turso Cloud, a self-run libSQL
server, and Bunny Database), so the backend is not tied to one vendor's API. Host
copies hold no record data, so an operator can add copies without moving athletes.
D93 already planned a hosted libSQL adapter on the same domain operations. D210
removed libSQL only from local use, and local use keeps rusqlite.

Rejected alternatives (details in the study):

- Postgres services (Supabase, Neon, Prisma Postgres, Nile). Rejected because
  Postgres rejects the schema and needs a second SQL model (experiment 4).
- Cloudflare Durable Objects. Not built in 2.1.3 because the host would have to run
  inside a Worker as WebAssembly, without rusqlite and without `BEGIN`.
- Cloudflare D1. Rejected because, from outside Cloudflare, it is reached only
  through an HTTP API limited to 1,200 requests every 5 minutes.
- SQLite Cloud. Not built because it has no Rust client and states no database count
  or size limits.
- Turso Sync of a file written by rusqlite. Rejected because Sync did not see the
  write (experiment 2), and its conflict rule is last push wins.
- rqlite, dqlite, LiteFS. Rejected because they do not fit one database per athlete
  with the library's transactions, and LiteFS Cloud has shut down.
- Make the library asynchronous. Rejected because the CLI and every existing caller
  would change for a need only the host has.
- Let the host write rows with its own SQL. Rejected because it would be a second
  copy of the record rules (D210).

## D258. A storage provider is supported only after the qualification suite

A backend or provider is listed as supported only after a named, reproducible run of
the qualification suite in spec section 8 passes. The report names the provider,
engine, region, crate versions, and date.

The first runs are a local libSQL server in continuous integration and Turso Cloud
libSQL-engine databases in a manual run. Turso-engine databases on Turso Cloud stay
unlisted while Turso calls them early preview. Bunny Database stays unlisted while it
is in preview.

On a libSQL server, the remote backend:

- reads `application_id` and `user_version` but never writes them. Turso Cloud makes
  them read-only, and the metadata table is authoritative (D93);
- skips `journal_mode` and `busy_timeout`, which Turso Cloud does not support;
- runs each write as one interactive transaction on one connection, and refuses to
  open a database where a dangling reference is not rejected;
- replaces the verified local backup before a migration with a provider restore point
  plus a complete package export, and refuses to migrate if either fails (D124);
- sends large imports in batches inside one transaction.

Turso's documentation does not say whether foreign keys hold on every remote
statement. The JavaScript client in 2.1.0 lost that setting between statements
([remote targets](../../packages/npm/REMOTE-TARGETS.md)). The suite tests it rather
than assuming it.

Rejected alternatives:

- List providers from their documentation. Rejected because the documentation leaves
  the foreign-key and backup questions open.
- Qualify Turso-engine databases first. Not chosen because they are in early preview
  on Turso Cloud, while libSQL has years in production there.

## D259. Attachments live in a directory or an S3-compatible bucket

The native backend keeps attachments in the folder beside the database, as today. The
remote mode keeps them in any S3-compatible bucket under `<resource-id>/attachments/`.

An upload goes to a staging key first. The host checks length, media type, and
SHA-256, then commits the record that names the file, then moves the file to its final
key. Downloads use short-lived signed links issued after the permission check. A
cleanup job removes staged files that no committed record names.

The S3 protocol is the common interface of every bucket service the study found, so
operators can choose any of them. Upload, verify, then commit gives a safe retry
(experiment 5).

Rejected alternatives:

- Store attachments as database BLOBs. Rejected because they add transfer, enlarge
  every copy and backup of the database, and still need ordinary files for export and
  erasure (experiment 8).
- A vendor's own file API. Rejected because it ties the host to one vendor.

## D260. Sign-in is provider-neutral OpenID Connect

The host accepts OAuth access tokens from one configured OpenID Connect provider. It
checks issuer, signature, expiry, intended API, and client. It maps the issuer and
subject to a host principal. Email is never the identity.

Native apps sign in through the system browser with the authorization code flow and
PKCE (RFC 8252). The host publishes protected-resource metadata (RFC 9728) so CLI,
mobile, and MCP clients can find the provider.

Two recipes are tested with the same API tests: a hosted provider (WorkOS AuthKit)
and a self-run provider (Keycloak). Other providers that meet the same checks work
without code changes.

The operator claims the first owner with a one-time local setup command. An athlete
accepts their own invitation. An invitation from a coach cannot make anyone an owner.

Rejected alternatives:

- Build accounts and passwords into the host. Rejected because account recovery and
  multi-factor sign-in are hard to do safely, and providers already do them.
- Support one provider only. Rejected because an operator must be able to run every
  part without a vendor.
- Let the sign-in provider decide record access. Rejected because only the athlete's
  grant can give access (D262).

## D261. Shared edits go through the host

While a record is connected to a host, edits that others should see are sent to the
host. The host checks the grant, the expected revision, and the request ID, and
commits through the library. Devices refresh from accepted changes (D263). Shared
edits need a connection.

Every accepted request writes a receipt in the same transaction as the change. A
repeated request with the same ID and body returns the original result. The same ID
with a different body is a conflict. A stale correction is refused, and the client
keeps the person's version for review.

The library's revision checks already make shared writes safe when one place accepts
them (experiment 1). Coaches only add notes and programming, so concurrent offline
editing by several people is not needed.

Rejected alternatives:

- Sync database files between devices and the host. Rejected because the tested sync
  did not see library writes, and last-push-wins discards a stale edit instead of
  keeping it for review (experiment 2).
- Merge offline edits automatically. Rejected as unnecessary for append-only coach
  work, and unsafe for corrections to measured data.

## D262. Grants are checked on every request; revocation raises an access epoch

The athlete grants a named coach, through a named app, any of three permissions: read
workout summaries, add coach notes, add programming. Nothing is selected until the
athlete approves. The host checks the current grant on every request, including
lists, counts, searches, attachments, and queued work.

Revoking a grant raises an access epoch stored in the athlete's database. A coach
write commits only if the epoch it checked is still current, in the same transaction.
A write ordered after the revocation fails.

The grant directory and the athlete's database are separate databases, so one
transaction cannot cover both. The epoch puts the deciding value inside the athlete's
database. This works the same when several host copies serve one athlete.

Rejected alternatives:

- Check the grant only when the request starts. Rejected because a write could commit
  after the athlete revoked access.
- Route every grant change and write for an athlete through one process. Rejected as
  the only mechanism because the remote mode allows many host copies.
- Let a "read training" grant return full workout JSON. Rejected because workouts hold
  routes and heart-rate data that the summary grant does not cover.

## D263. A device refresh compares content, not only the database revision

A device keeps an ordinary SQLite copy. It remembers the database revision and the
deterministic package manifest digest of the last copy it verified. Before applying
host changes, it checks that the local file still matches that digest. If it does not,
the device keeps the local file, downloads the hosted copy beside it, and asks the
owner. It never chooses a winner by itself.

Changes come from `internal_change_log` and are applied through the library with their
revisions unchanged. If a change is missing or cannot be applied, the device falls
back to a verified full package.

Two copies with different notes reported the same database ID and revision
(experiment 7). Comparing only those numbers would overwrite the athlete's work.

Rejected alternatives:

- Replace the device copy with the hosted copy on every refresh. Rejected because it
  discards offline local work.
- Copy raw rows into the device database. Rejected because it bypasses the library's
  checks.

## D264. Backups, restores, and keys depend on the backend

Native backend: Litestream copies each athlete database to S3-compatible storage. The
host copies each attachment when it is uploaded. A scheduled job exports each
athlete's complete package to storage with separate credentials. The Litestream recipe
is published only after it is tested with the library's checkpoints and restores.

libSQL remote backend: the provider's per-database restore, plus the same scheduled
package export, plus a copy of the attachment bucket.

Every restore is one athlete. It starts isolated, invalidates sessions, and replays
later erasures and revocations before it serves requests.

Per-athlete keys: the libSQL backend passes a per-database key when the provider
supports it. The host reads keys from a key service the operator configures, never
from its own directory database. The native backend has no per-athlete encryption in
2.1.3. SQLCipher is a rusqlite build option, but no source confirms that Litestream
can replicate an encrypted file.

Whoever runs the host and its storage can read records. Keys limit what a copied disk
or backup reveals. The host tells the athlete who runs the host and the storage when
they connect.

Rejected alternatives:

- Rely only on a provider's volume snapshots. Rejected because they restore every
  athlete at once, and on some hosts wiping the volume deletes them.
- Rely only on a provider's database restore. Rejected because the Turso engine's
  maintainers advise independent backups, and the package is the format an athlete
  can open anywhere.
- Offer native encryption in 2.1.3. Deferred until backups of encrypted files are
  tested.

## D265. The host is one long-running program

`ath-host` is one executable, shipped with a container definition. It needs a
long-running process. The native backend also needs a persistent disk, and only one
copy may own that disk. The remote backend needs no disk, and any number of copies
may run.

Large provider imports take minutes and hundreds of megabytes. A program that runs
per request with seconds of CPU time cannot do them.

Rejected alternatives:

- Supabase Edge Functions. Rejected because each request gets 2 seconds of CPU time,
  256 MB of memory, and no persistent disk.
- Cloudflare Workers. Rejected for 2.1.3 because Rust runs there only as WebAssembly,
  without rusqlite.
- Supabase Compute. Not a target while it is a private alpha "for evaluation, not
  production" with no persistent disk.

## D266. Clients reach records only through the host API

Apps, the npm client, Swift code, and MCP tools talk to the host's versioned HTTPS
API. They never receive database credentials. The npm client gains an explicit host
connection. Its handling of `libsql:` and other database URLs stays as
[REMOTE-TARGETS.md](../../packages/npm/REMOTE-TARGETS.md) describes.

The API description is generated from the Rust definitions, like the npm protocol
declarations, and `cargo test --test generated` checks it. Version 1 contains only operations whose permission and output
were reviewed. A local command becomes remote only after that review.

A database credential would let a client bypass grants, receipts, and the access
epoch.

Rejected alternatives:

- Give each app a database token limited to one athlete. Rejected because the token
  would expose the whole database, beyond any grant.
- Expose the local worker protocol over the network. Rejected because some of its
  operations take local file paths.

## D267. A move between hosts has one writer at every step

Moving a record to another host follows five steps: connect an empty destination,
freeze the source, verify and restore privately at the destination, retire the source
writer with a single-use activation bound to this transfer, then activate the
destination. At most one host accepts writes at any moment. A lost reply is retried
with the same transfer ID.

If the source cannot be reached, the owner can restore from a device copy or backup.
That copy is labelled a recovered fork, because software cannot prove an unreachable
host stopped writing.

Changing coach never moves a record. It changes grants only.

Rejected alternatives:

- Copy the database and switch clients at once. Rejected because writes made during
  the copy would be lost or duplicated.
- Let the destination activate after a timeout. Rejected because a timer cannot prove
  the source stopped.
