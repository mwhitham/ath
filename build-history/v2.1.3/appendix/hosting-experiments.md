# Hosting experiments on version 2.1.2

These local experiments informed D256, D257, D259, D261 and D263. They ran on
2026-11-22 and 2026-11-23 against commit 073f9bc on macOS arm64. All athlete data was
synthetic. No live cloud account was used. Database, HTTP and object-storage servers
ran on the same machine. The comparison of services is in
[hosted-storage-options.md](hosted-storage-options.md).

## Fixture

The checked-in demo database: 491,520 bytes, 25 current records, 26 revisions, one
attachment, and one heart-rate series. SQLite listed 43 tables, 60 indexes, 16
triggers and 5 views. Its package was four files totalling 61,028 bytes.

The demo attachment is 68 bytes, which says little about media transfer. The tests
also used an invented 1024 × 683 PNG of 2,099,567 bytes, and a valid PNG padded to
the 26,214,400-byte attachment limit.

## 1. A Rust host over the library, one directory per athlete

A 33-line test server called the library unchanged. Each athlete had a private
directory. The server resolved the directory from the signed-in athlete, never from
the request.

- Two athletes had records with identical IDs. Each athlete's credentials could read
  and change only their own record.
- A Swift client on macOS and a separate agent request corrected the same workout
  from the same revision. One succeeded. The other received a conflict. Exactly one
  revision was added.
- A write lock held on one athlete did not delay a write for another athlete.
- A test coach credential could read selected workout fields. It could not correct a
  record, read the profile, or reach the other athlete.
- An official MCP SDK client called a tool through the same server, and the same
  athlete check applied.
- A device copy edited offline replayed a stale correction on reconnect. The host
  refused it, and both values stayed readable. An unrelated offline edit applied.

**What it shows:** the library's revision checks already give safe shared writes
when every write goes through one host. Separation between athletes depended on the
server's own checks.

## 2. Turso Sync around a file written by rusqlite

The Turso 0.8.4 local sync server and Sync client were seeded with the demo. Stock
`ath` then added one athlete report to the synced file. `ath` saved it and could read
it. On reopening, the Sync client found 26 local records and zero pending changes. A
fresh client pulled 25 records.

A separate two-client test of conflicting edits kept the last push. No stale-revision
response was produced.

**What it shows:** Sync cannot carry writes made by the library as built. Its
conflict rule replaces the library's revision check (D261).

## 3. The same edit written through Turso's own client

In a fresh copy, the same report was written through Turso's client: the record,
its revision, its typed row and the database revision. Sync saw four pending
changes, and the second client received the report. After the attachment and the
two file header values were supplied, stock `ath` exported the receiving copy. All
four package files matched a stock export byte for byte.

**What it shows:** a SQLite-compatible engine can hold the record exactly. Writes
must go through that engine's own connection. That is what the libSQL backend does
(D257). The libSQL remote protocol itself was not tested here. The qualification
suite does that (D258).

## 4. Postgres

Postgres rejected the first migration at `STRICT`. A small translated model loaded
the fixture rows. Row-level security with separate login roles hid other athletes'
rows. A translated workout correction kept its history, and stock `ath` restored the
exported result.

**What it shows:** Postgres can enforce the rules only through a second SQL model
maintained beside the library (D257).

## 5. A report with a photo, a retry, and a refresh

The test made a report with the 2,099,567-byte photo. The photo went to an
S3-compatible test server. The rows went into a database through Turso's client, with
a receipt in the same transaction.

- A truncated upload left the database at revision 3, with no report and no
  attachment.
- A complete upload whose reply was then discarded, followed by a retry, left one
  report, one attachment and one receipt at revision 4.
- A reader fetched the changed rows and the photo. It detected a partial download and
  accepted the intact retry.
- The updated copy passed `ath check`. Its five-file export matched the reference
  export byte for byte. A clean import restored the photo intact.

**What it shows:** upload, verify, then commit with a receipt gives a safe retry
(D259, D261).

## 6. Export, restore, and files beside the record

After edits and the 2 MB attachment, the record was exported, restored into a new
workspace, checked and exported again. Every package file matched byte for byte.

A separate test stopped a workspace and recovered the whole folder. It brought back an
extra report written beside the database, which the standard package does not
include.

**What it shows:** the package carries the record exactly. A host backup must also
list any supported extra files separately.

## 7. Two copies with the same revision and different contents

Two copies were restored from the same package. Each received a different note
through stock `ath`. Both then reported the same database ID and the same database
revision. Their package manifests differed.

**What it shows:** a refresh that compares only the database ID and revision can
silently overwrite local work. A refresh must compare content (D263).

## 8. Attachments stored inside the database

Two attachments stored as BLOBs through Sync kept their bytes. The file sizes and the
byte counts the Sync SDK reported were:

| File | Sent by the uploading client | Received by a new client |
|---:|---:|---:|
| 2,099,567 | 2,801,287 | 2,121,664 |
| 26,214,400 | 34,954,398 | 26,306,540 |

The S3 test client moved the same 2,099,567-byte payload unchanged.

**What it shows:** BLOBs work but add transfer, enlarge every database copy, and
still need ordinary files for export and erasure (D259).

## Limits of this evidence

- Everything ran on one machine with synthetic data.
- No live provider account, region or network latency was measured.
- The libSQL remote protocol, provider tokens, per-database keys and provider
  restores were not tested.
- The test server implemented one coach read grant and returned one conflict code for
  every write error.
- Live sign-in, an iOS device and a remote MCP authorization flow were not tested.

The qualification suite (D258) and the build steps in the [spec](../spec.md) cover
these.
