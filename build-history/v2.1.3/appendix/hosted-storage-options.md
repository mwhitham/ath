# Where a hosted record can live: options and how to choose

This study supports D256–D259, D264 and D265. It compares the ways to store athlete
records on a server that many athletes share. It also compares the places the host
program can run. Facts were checked on 2026-11-23 against each vendor's own
documentation. Services change often. Check the current documentation before you
rely on a limit.

The study does not recommend a vendor. It explains what version 2.1.3 builds and
why, and it gives the questions a developer or operator should answer before
choosing.

## 1. What the record needs from storage

These requirements come from the record format and the existing library. They are
the same for every operator.

| # | Requirement | Why |
|---|---|---|
| R1 | Runs the existing SQL schema and record rules unchanged: STRICT tables, triggers, deferred foreign keys, revision chains, erasure. | The library is the only place record rules live (D210). A second implementation of those rules would drift. |
| R2 | Keeps one database per athlete, for any number of athletes. | A database is the unit of isolation, export, restore, and moving (D256). |
| R3 | Restores one athlete to an earlier moment without changing anyone else. | A shared restore would roll back other athletes or bring back revoked access. |
| R4 | Keeps the complete portable package possible. | The athlete must be able to leave with everything (D82). |
| R5 | Can be reached by a Rust program running on an ordinary server. | The host calls the Rust library for every record operation (D257). |
| R6 | Lets an operator lock each athlete's database with its own key, where wanted. | It limits what a copied disk or backup reveals. |
| R7 | Can be run without any vendor. | Anyone can run their own host. |
| R8 | Is mature enough to hold a health record. | Losing or corrupting a record cannot be undone. |

Two further facts shape the choice:

- **Every SQL statement over a network is a round trip.** A local SQLite call takes
  microseconds. A call to a database in the same region takes milliseconds. The
  library's writes run many statements. Large imports must be sent in batches.
- **The library speaks to SQLite through one crate, rusqlite, in 31 files.** Any
  remote option needs a second backend behind the storage interface (D257). The
  record rules stay in one place either way.

## 2. Summary

"Yes" means documented by the vendor or shown by a test in this study. "No" means
documented as unavailable. "Not stated" means the documentation does not say.

| Option | R1 SQL rules | R2 many databases | R3 restore one | R5 Rust from any server | R6 own keys | R7 self-run | R8 maturity |
|---|---|---|---|---|---|---|---|
| Native SQLite directory | Yes | Yes, limited by disk | With Litestream | Yes (in process) | Not in 2.1.3 | Yes | SQLite |
| Turso Cloud, libSQL engine | Expected; to qualify (D258) | Yes, no database limit except on the no-cost plan | Yes, 1–90 days by plan | Yes, `libsql` crate | Yes, on its two highest plans | No | Years in production |
| Turso Cloud, Turso engine | To qualify | Yes | Yes | Yes, `turso_serverless` crate | Yes | No | "Early preview" on the cloud |
| Self-run libSQL server (`sqld`) | Expected | Yes, as namespaces | With its S3 backup | Yes, `libsql` crate | Not checked | Yes | Last release 2025-02-14 |
| Bunny Database | Expected (libSQL) | 50 databases of 1 GB in preview | Not documented | Yes, `libsql` crate | Not stated | No | Public preview |
| Cloudflare Durable Objects (SQLite) | Partly: no `BEGIN` or `SAVEPOINT` in `sql.exec` | Yes, unlimited objects, 10 GB each | Yes, 30 days | No: code must run in a Worker | No | No | Generally available |
| Cloudflare D1 | Partly: transactions only through `batch()` | Up to 1,000,000, 10 GB each | Yes, 30 days | Only by a rate-limited HTTP API | No | No | Generally available |
| SQLite Cloud | Expected | Not stated | Yes, 24 hours by default | No Rust SDK | Not stated | No | Not stated |
| Turso Sync on a file written by rusqlite | No: writes not seen | — | — | — | — | — | Failed in test |
| rqlite, dqlite | No | No | — | — | — | Yes | Active |
| Postgres services (Supabase, Neon, Prisma Postgres, Nile) | No: second SQL model | Varies | Varies | Yes | Mostly no | Partly | Mature |
| Device sync engines (PowerSync, ElectricSQL) | Not a store | — | — | — | — | — | — |

## 3. The options in detail

### 3.1 Native SQLite files in a directory

Each athlete gets a private directory with `athlete.ath.sqlite` and its
`attachments/` folder. The host opens the file in process with the same library the
CLI uses.

- **Fits R1, R4, R5, R7 and R8 without new code.** It is the reference
  implementation. Every existing test already covers it.
- **One writer per disk.** SQLite allows one writer per file, and only one host
  process may own a directory. More host machines need a routing table that says
  which machine serves which athlete. Moving an athlete follows the move protocol
  (D267).
- **Disk size limits the number of athletes per machine.** For example, a Railway
  volume holds up to 1 TB without contacting Railway, and a service can have only
  one volume ([Railway volumes](https://docs.railway.com/volumes/reference)).
- **Backups need a separate tool.** Litestream copies each change in a directory of
  SQLite databases to S3-compatible storage within about a second. It restores one
  database at a time to a chosen moment ([directory replication](https://litestream.io/guides/directory/)).
  Version 0.5.18 was released on 2026-11-03.
  - Litestream needs WAL mode and wants to control checkpoints
    ([tips and caveats](https://litestream.io/tips/)). The library uses WAL for
    writes. Its `close()` runs a truncating checkpoint, and a fresh restore briefly
    switches the journal mode. The host must keep connections open and must be
    tested with Litestream before the recipe is published.
  - Litestream does not copy attachments. The host copies each attachment when it is
    uploaded.
  - Litestream 0.5.x has no client-side encryption. Only bucket-level encryption is
    available ([configuration](https://litestream.io/reference/config/)).
- **Per-athlete keys are possible but not in 2.1.3.** rusqlite can be built with
  SQLCipher (`bundled-sqlcipher`). No source confirms that Litestream can replicate
  an encrypted file. Version 2.1.3 does not offer this (D264).

### 3.2 Turso Cloud

Turso Cloud hosts SQLite-compatible databases reached over the network. It offers
two engines ([Turso Cloud](https://docs.turso.tech/turso-cloud)):

- **libSQL**, a fork of SQLite that Turso describes as "battle-tested in production
  on Turso Cloud for years". Rust programs reach it with the `libsql` crate
  (0.10.0, October 2026) with the `remote` feature
  ([Rust reference](https://docs.turso.tech/sdk/rust/reference)).
- **Turso**, a rewrite of SQLite in Rust. On Turso Cloud these databases are in
  "early preview". Rust programs reach them with `turso_serverless`. The engine
  dropped its beta warning in July 2026 and has not reached 1.0. Its maintainers
  advise keeping independent backups ([repository](https://github.com/tursodatabase/turso)).

Facts that matter here:

- **Database count.** Turso's no-cost plan allows 100 databases. Its other plans
  have no database limit. An idle database costs only its storage.
- **Restore.** Each database can be restored to an earlier moment. The window is
  1 to 90 days depending on plan.
- **Own keys.** A database can be encrypted with a key the operator supplies. The
  key is passed on each connection and Turso does not store it. It is available on
  Turso's two highest plans. A key cannot be changed in place yet
  ([encryption](https://docs.turso.tech/cloud/encryption)).
- **Differences from local SQLite** ([limitations](https://docs.turso.tech/cloud/limitations)):
  `application_id` and `user_version` are read-only. `journal_mode` and
  `busy_timeout` are not supported. The library already treats its metadata table
  as authoritative (D93), so the remote backend reads the header values and does
  not write them.
- **Not stated in the documentation:** whether `PRAGMA foreign_keys` holds for every
  statement on a remote connection, and whether the SQLite online backup API is
  available. The JavaScript libSQL client used by 2.1.0 lost the setting between
  statements ([remote targets](../../../packages/npm/REMOTE-TARGETS.md)). The
  qualification suite tests both (D258).
- **Regions.** The pages read do not list Turso's regions. Place the host and the
  databases in the same region and measure the round trip.
- **Ownership.** Supabase announced on 2026-10-02 that it is acquiring Turso. It
  says Turso keeps operating its platform and that nothing changes for existing
  users ([announcement](https://supabase.com/blog/supabase-is-acquiring-turso)).

### 3.3 A self-run libSQL server

The libSQL server (`sqld`) serves many databases ("namespaces") and can back them up
to S3 ([libSQL](https://github.com/tursodatabase/libsql)). It speaks the same
protocol as Turso Cloud's libSQL databases, so the same backend reaches it. Its last
release was v0.24.32 on 2025-02-14, and its maintainers point new projects to the
Turso engine. It is useful for tests and for operators who want the remote mode
without a vendor. It is a weak choice for new production use because it is no longer
actively released.

### 3.4 Bunny Database

Bunny Database is built on libSQL and is reached with the same `libsql` crate
([Rust](https://bunny.net/docs/database/connect/rust)). It has been in public preview
since 2026-02-03. Preview accounts are limited to 50 databases of 1 GB each, raised
on request ([limits](https://docs.bunny.net/database/limits)). It documents no
per-database restore, and up to 10 seconds of writes can be lost when its main server
fails ([durability](https://bunny.net/docs/database/durability-and-consistency)).
It is worth testing again when it leaves preview.

### 3.5 Cloudflare Durable Objects with SQLite storage

Each Durable Object has its own SQLite database. Objects are unlimited, each holds up
to 10 GB, and each can be restored to any moment in the last 30 days
([limits](https://developers.cloudflare.com/durable-objects/platform/limits/),
[SQLite storage](https://developers.cloudflare.com/durable-objects/api/sqlite-storage-api/)).
It became generally available on 2025-04-07.

It does not fit this library as built:

- Code that uses the database must run inside a Cloudflare Worker. Rust runs there
  only as WebAssembly through the `worker` crate, without rusqlite.
- `sql.exec` cannot run `BEGIN` or `SAVEPOINT`. Transactions use the storage API's
  own functions, so the store's transaction code would change.
- Keys are managed by Cloudflare
  ([data security](https://developers.cloudflare.com/durable-objects/reference/data-security/)).

Using it means moving the host into Cloudflare, not pointing an existing host at a
database. It is the closest match after Turso for operators who already run on
Cloudflare.

### 3.6 Cloudflare D1

D1 offers up to 1,000,000 databases per account, 10 GB each, and a 30-day restore
([limits](https://developers.cloudflare.com/d1/platform/limits/)). Outside a Worker it
is reached through Cloudflare's HTTP API, which Cloudflare limits to 1,200 requests
every 5 minutes per user ([API limits](https://developers.cloudflare.com/fundamentals/api/reference/limits/)).
A Worker can bind about 5,000 databases. D1 also charges for each database above an
included 100,000. Multi-statement transactions are only available through
`batch()`. These limits rule it out for a host that serves many athletes from
outside Cloudflare.

### 3.7 SQLite Cloud

SQLite Cloud (now at sqlite.ai) hosts SQLite databases. Its documentation recommends a
separate database for each tenant ([overview](https://docs.sqlitecloud.io/docs/overview)).
It restores one database to the second, with 24 hours of history by default
([backups](https://docs.sqlitecloud.io/docs/backups)). It has an extension for syncing
copies to devices. It has SDKs for C, Go, JavaScript, PHP, Python and Swift, and none
for Rust. Its documentation does not state a database count limit, a size limit, or
support for keys supplied by the operator. Those questions need an answer from the
vendor before it can be considered.

### 3.8 Turso Sync on a file written by rusqlite

Turso Sync keeps a local file and pushes and pulls logical changes. Its conflict rule
is "last push wins", and it only sees changes made through its own connection
([Sync usage](https://docs.turso.tech/sync/usage)). In this study's test, stock `ath`
saved a record into a Sync-created file and Sync reported zero changes to push
([experiments](hosting-experiments.md), test 2). It cannot carry writes made by the
library as built, and last-push-wins conflicts with revision checks (D261).

### 3.9 rqlite, dqlite and LiteFS

- **rqlite** serves one database per cluster and does not support explicit
  `BEGIN`/`COMMIT` ([FAQ](https://rqlite.io/docs/faq/)).
- **dqlite** is a C library with Go bindings and its own network protocol
  ([dqlite](https://github.com/canonical/dqlite)).
- **LiteFS Cloud** shut down on 2024-10-15. Fly.io says it cannot support LiteFS
  itself, which is still before 1.0 ([notice](https://community.fly.io/t/sunsetting-litefs-cloud/20829),
  [LiteFS](https://docs.fly.io/litefs)).

None fits a database per athlete with the library's transactions.

### 3.10 Postgres services

Supabase, Neon, Prisma Postgres and Nile host Postgres. They are mature, but Postgres
is not SQLite. In this study's test, Postgres rejected the first migration because of
`STRICT` ([experiments](hosting-experiments.md), test 3). A translated schema worked,
but it would be a second SQL model to maintain beside the library (D210, D257).
Their database-per-tenant support also varies: Neon limits projects unless an
account is approved for more, Prisma Postgres allows 1,000 databases, and Nile keeps
tenants as rows in shared tables.

### 3.11 Device sync engines

PowerSync keeps SQLite on the device and syncs it with Postgres, MongoDB, MySQL or
SQL Server on the server. Its Rust SDK is in beta. ElectricSQL sends data from
Postgres to devices and leaves writes to your own API. Neither hosts a database per
athlete. Either could sit beside a host later. Neither replaces one.

## 4. Where the host program runs

The host is one executable, usually shipped as a container (D265). What it needs
depends on the storage backend.

| Backend | The host needs | Number of copies |
|---|---|---|
| Native directory | A persistent disk and a long-running process. | One per disk. More machines need routing. |
| libSQL remote | A long-running process with network access. No disk. | As many as needed. |

Places researched:

- **A container host with disks, such as Railway.** A service can have one volume.
  A service with a volume cannot run replicas, and a redeploy causes a short outage.
  Without a volume, a service can run many replicas, up to 42 depending on plan.
  Railway encrypts volumes, backups and buckets at rest. Volume backups restore the
  whole volume, and wiping a volume deletes its backups
  ([volumes](https://docs.railway.com/volumes/reference),
  [backups](https://docs.railway.com/volumes/backups),
  [security](https://docs.railway.com/enterprise/security)).
- **Supabase Edge Functions.** They run TypeScript, or Rust compiled to
  WebAssembly. Each request gets 2 seconds of CPU time, 256 MB of memory and at most
  400 seconds. There is no persistent disk
  ([limits](https://supabase.com/docs/guides/functions/limits)). The host cannot run
  there as built, and large imports would not fit. Supabase Compute, announced in
  October 2026, runs any container. It is a private alpha in one region with no
  persistent disk, "for evaluation, not production"
  ([alpha](https://supabase.com/blog/supabase-compute-private-alpha)).
- **Cloudflare Workers.** Rust runs only as WebAssembly. See section 3.5.
- **Any machine with Docker.** A rented server or a computer at home works for
  either backend. The operator owns updates, disks and network access.

Attachments need an S3-compatible bucket in remote mode. Railway buckets, Supabase
Storage, Amazon S3 and Cloudflare R2 all speak the S3 protocol. Railway buckets and
Supabase Storage do not keep earlier versions of an object, so a deleted file is gone
unless it was copied elsewhere ([Railway buckets](https://docs.railway.com/storage-buckets),
[Supabase S3](https://supabase.com/docs/guides/storage/s3/compatibility)).

## 5. What 2.1.3 builds, and why

Version 2.1.3 builds two database backends (D257):

1. **Native SQLite directory.** It needs no new dependency, it is what every
   existing test covers, and anyone can run it on one machine. It is the reference
   that every other backend is compared with.
2. **libSQL remote.** It is the only remote family that meets R1 to R6 together. One
   crate reaches three services (Turso Cloud, a self-run libSQL server, Bunny
   Database), so the backend is not tied to one vendor's API. Host copies hold no
   record data, so an operator can add copies without moving athletes.

Version 2.1.3 does not build the others:

- Durable Objects and D1 would need the host to run inside Cloudflare, or would hit
  API limits from outside. A Workers-based host is a separate project.
- SQLite Cloud has no Rust client and no stated limits.
- Postgres services need a second SQL model.
- Turso Sync cannot see rusqlite writes and uses last-push-wins.
- rqlite, dqlite and LiteFS do not fit a database per athlete.

## 6. How to choose for your own host

Answer these questions in order.

1. **Do you need anyone but the athlete to reach the record?** If not, use `ath` with
   a local file. You need no host.
2. **Will one machine hold every record you expect?** Estimate each record's size
   from your own data: the size of its `.ath.sqlite` file and `attachments/` folder. If one disk holds them all, the native
   directory backend is the simplest choice. Add Litestream and a scheduled package
   export.
3. **Do you need more than one copy of the host running at once, or no disks to
   manage?** Use the libSQL remote backend. Choose a provider that has passed the
   qualification suite (D258), and put it in the same region as the host.
4. **Must every part run on machines you control?** Use the native directory backend,
   or a self-run libSQL server if you accept its release status. Several native hosts
   need a routing table and the move protocol.
5. **Do you need each athlete locked with a separate key?** Use the libSQL remote
   backend with a provider that accepts keys you supply, and keep the keys in a key
   service outside the host. The native backend does not offer this in 2.1.3.
6. **Who can read the records?** Whoever runs the host and its storage can read them.
   Per-athlete keys limit what a stolen disk or backup reveals. They do not stop
   someone who controls the running host. Tell athletes who runs the host and the
   storage when they connect.

Measure before you commit. Run the qualification suite against your account and
region. Time a workout save and a large import.

## 7. What would change this decision

- A provider that passes the qualification suite with a different protocol. It gets
  a backend behind the same interface.
- Turso-engine databases leaving early preview on Turso Cloud. They would then be
  qualified with `turso_serverless`.
- Bunny Database leaving preview with documented per-database restore.
- Litestream confirming support for encrypted SQLite files. The native backend could
  then offer per-athlete keys.
- Supabase Compute becoming generally available with a persistent disk. It would
  become another place the host can run.
- A change in Turso's terms or operation after the acquisition. The libSQL backend
  also reaches other servers, and every athlete can be exported and moved.
