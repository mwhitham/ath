# Remote libSQL targets in athleticstandard 2.1.1

Version 2.1.0 accepted a database target that is a URL, such as
`libsql://example.turso.io` or `https://example.com/db`, and gave it to the
libSQL client. Version 2.1.1 keeps that target parse and keeps the results and
refusals 2.1.0 produced. It never treats a URL as a local file name, and it adds
no remote capability: the native library opens local files only, and the npm
client opens no network connection.

This file lists what every public export did with a remote target in 2.1.0 and
what it does in 2.1.1.

## Target parsing

The rule is unchanged (`src/client/handle.ts`, `parseDatabaseTarget`):

- A target that starts with `file:` is a local file. Its path is resolved.
- A target that matches `<scheme>://` is remote. Its URL is kept as given and
  it has no local path.
- Anything else is a local path, resolved against the working directory.

## Opening a remote target

`openDatabase`, `inspectDatabase`, and `createDatabase` are the only exports
that turn a target into a handle.

| Target | 2.1.0 result | 2.1.1 result |
| --- | --- | --- |
| A scheme the libSQL client does not support, such as `sqlite://` | `LibsqlError` with code `URL_SCHEME_NOT_SUPPORTED`, before any network use | The same message, `name` `LibsqlError`, and `code` `URL_SCHEME_NOT_SUPPORTED` |
| `libsql:`, `http:`, `https:`, `ws:`, or `wss:` with no reachable server | A network error: `TypeError: fetch failed` for HTTP routes, `LibsqlError` `HRANA_WEBSOCKET_ERROR` for WebSocket routes | `Error: SQLite foreign-key enforcement could not be enabled`, with no connection attempt |
| The same schemes with a reachable libSQL server | `Error: SQLite foreign-key enforcement could not be enabled` | The same error |
| `createDatabase` with an invalid `databaseId` or `createdAt` | `ZodError`, before connecting | `ZodError`, before refusing |

Why a reachable server refused in 2.1.0: the open ran `PRAGMA foreign_keys = ON`
and then read `PRAGMA foreign_keys` back. The libSQL client runs each
statement on a new server stream, and a new SQLite connection starts with
foreign keys off. The read returned 0, so the open refused. The 2.1.1 error
keeps that message; its `cause` says that 2.1.1 made no connection.

These outcomes were observed by running the 2.1.0 source with unreachable
targets of each scheme. The reachable-server row follows from the 2.1.0 open
code and the libSQL client's per-statement streams; it was not run against a
live server.

A server configured to start every connection with foreign keys on would have
let the 2.1.0 open continue. That is the one case where 2.1.1 refuses what
2.1.0 might have allowed. Such a handle could then read over the network, and
most writes refused because they needed a local file (see below). 2.1.1 adds
no remote reads, so it refuses at open.

## Exports that take a handle

No remote handle can be made, so these exports never see one. An object that
the library did not return, including one written by hand with `isLocal: false`,
is refused with `Athletic Standard database is closed`, as in 2.1.0. A handle
holds no connection in either case.

The exceptions are the exports whose 2.1.0 code checked for a local file
before it used the database. They keep that refusal or result for an object
without `localPath`:

| Export | Result without a local path, in 2.1.0 and 2.1.1 |
| --- | --- |
| `releaseLocalDatabase` | Calls the object's `close()` and returns |
| `markSkipWalCheckpointOnRelease` | Returns |
| `packageConflictSetPath` | `temporary import conflicts require a local SQLite database` |
| `readPackageConflictSet` | `temporary import conflicts require a local SQLite database` |
| `savePackageConflictSet` | Returns `null` when the preview has no savable conflicts; otherwise `temporary import conflicts require a local SQLite database` |
| `clearPackageConflictSet` | Returns without changing anything |
| `prepareAttachments` | `[]` for no paths; otherwise `attachment ingestion requires a local SQLite database` |
| `withPreparedAttachmentFiles` | Runs `write` for no attachments; otherwise `attachment ingestion requires a local SQLite database` |
| `createVerifiedSqliteBackup` | `SQL schema migration backup requires a local database file` |

Every other export that takes a handle is refused as a closed handle:
`applyLogPlan`, `applyPendingDatabaseMigrations`, `assertPackageConflictSetCurrent`,
`buildEvidence`, `calculate`, `checkDatabase`, `correctRecord`,
`countProviderEncodingRepairCandidates`, `countRevisionAlignRepairCandidates`,
`databaseStats`, `deleteRecord`, `eraseRecord`, `executeConfirmedDatabaseMigration`,
`executeConfirmedProviderEncodingRepair`, `exportRecords`, `getRecord`,
`gradePrediction`, `importProviderRecords`, `importRecords`,
`listRecords`, `listRevisionAlignRepairRecordIds`, `migrateDatabase`,
`planLogEntry`, `planPredictionGrade`, `previewConflict`, `previewPackageImport`,
`previewProviderImport`, `previewRecordErasure`, `queryMeasurements`,
`queryRecordsAsOf`, `querySeries`, `querySessions`, `readDatabaseMetadata`,
`readRecord`, `readRecordLinks`, `readRecordRevisions`, `readSeriesView`,
`repairProviderEncoding`, `resolveAsOfTime`, `resolveCalendarRange`,
`resolveConflict`, `resolveEvidenceReference`, `resolvePackageConflict`,
`runV1Backtest`, `writeRecord`, `writeRecordChanges`, and `writeRecords`.

In 2.1.0 a few of these checked for a local file before the closed check:
the record writes refused with `ath-core writes a local database file` and
`calculate` with `ath-core reads a local database file`. That order only
mattered for hand-written objects, and 2.1.1 reports them as closed.

## Exports that take a path or a value

These exports take no database target. A URL passed to them is text, as in
2.1.0, except where a local file would be written:

| Export | 2.1.0 | 2.1.1 |
| --- | --- | --- |
| `defaultMigrationBackupPath` | Returns the URL text with the backup suffix; touches no file | The same |
| `writeV1BacktestReport` | Wrote beside a relative folder named after the URL text, such as `libsql:/host/`, and usually failed with `ENOENT` | Refuses: `A backtest report is written beside a local database file, not a remote URL: <url>` |
| `loadAthleticStandardPackage` | A package path; a URL is a relative path that does not exist | The same kind of file-not-found refusal |
| `makeTemporaryDirectory`, `removeTemporaryDirectory` | Local folders | The same |

The remaining exports take values only and are unaffected by targets:
`addAttachmentsToLogPlan`, `buildMigrationPreview`, `calendarDateInZone`,
`conflictResolutions`, `createUuidV7`, `databaseNeedsMigration`,
`deterministicJson`, `formatMigrationBackupTimestamp`, `readImportedPackageRecord`,
`renderCalculation`, `renderDatabaseStats`, `renderRecord`, `renderRecordList`,
`renderSeriesView`, `sqlite3BackupDotCommand`, the error classes, the constants,
and every `athleticstandard/schema` export.

## Tests

`test/database.test.ts` ("keeps remote URL targets remote and refuses them as
2.1.0 did") checks the open refusals for each scheme family, the Zod refusal
order in `createDatabase`, that no local file named after a URL appears, and the
`writeV1BacktestReport` refusal. The closed-handle test checks a hand-written
handle object.
