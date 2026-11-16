/**
 * One compatibility test or more for every `athleticstandard/database`
 * export, run against the native executable.
 */
import { copyFileSync, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { ZodError } from "zod";
import * as database from "../src/database.js";
import { callAsync } from "../src/client/worker.js";
import { athlete, noteData, removeFolder, repositoryRoot, temporaryFolder } from "./support.js";
import { buildScenario, day, type Scenario } from "./scenario.js";

let folder: string;
let scenario: Scenario;

beforeAll(async () => {
  folder = temporaryFolder();
  scenario = await buildScenario(folder);
}, 120_000);

afterAll(() => {
  scenario.handle.close();
  removeFolder(folder);
});

describe("handles", () => {
  it("creates, opens, inspects, and reads metadata", async () => {
    const path = join(folder, "handles.ath.sqlite");
    const created = await database.createDatabase(path, {
      databaseId: "01890f3e-e7d3-7cc0-88c8-000000000001",
      createdAt: "2026-01-01T00:00:00.000Z",
    });
    expect(created).toEqual({
      url: `file://${path}`,
      isLocal: true,
      localPath: path,
      close: expect.any(Function),
    });
    expect(await database.readDatabaseMetadata(created)).toEqual({
      application_marker: "ATH1",
      sql_schema_version: 6,
      athleticstandard_version: "2.1.2",
      database_id: "01890f3e-e7d3-7cc0-88c8-000000000001",
      created_at: "2026-01-01T00:00:00.000Z",
    });
    expect(statSync(path).mode & 0o777).toBe(0o600);
    created.close();
    await expect(database.createDatabase(path)).rejects.toThrow(`Refusing to replace existing database: ${path}`);
    await expect(database.createDatabase(join(folder, "bad.ath.sqlite"), { databaseId: "nope" })).rejects.toBeInstanceOf(
      ZodError,
    );

    const warnings: string[] = [];
    const opened = await database.openDatabase(path, { onSecurityWarning: (message) => warnings.push(message) });
    expect((await database.readDatabaseMetadata(opened)).database_id).toBe("01890f3e-e7d3-7cc0-88c8-000000000001");
    const inspected = await database.inspectDatabase(`file://${path}`);
    expect(inspected.localPath).toBe(path);
    expect((await database.readDatabaseMetadata(inspected)).sql_schema_version).toBe(6);
    await database.releaseLocalDatabase(opened);
    database.markSkipWalCheckpointOnRelease(inspected);
    await database.releaseLocalDatabase(inspected);
    await expect(database.readDatabaseMetadata(opened)).rejects.toThrow("Athletic Standard database is closed");
  });

  it("refuses every operation on a closed handle", async () => {
    const handle = await database.openDatabase(scenario.path);
    handle.close();
    handle.close();
    const id = scenario.fran.record_id;
    const closed = "Athletic Standard database is closed";
    const calls: Array<() => Promise<unknown>> = [
      () => database.readRecord(handle, id),
      () => database.getRecord(handle, id),
      () => database.listRecords(handle),
      () => database.readRecordRevisions(handle, id),
      () => database.readRecordLinks(handle, id),
      () => database.writeRecord(handle, { record_type: "athlete_report", data: noteData(), changed_by: athlete }),
      () => database.writeRecords(handle, [{ record_type: "athlete_report", data: noteData(), changed_by: athlete }]),
      () => database.writeRecordChanges(handle, { records: [] }),
      () => database.correctRecord(handle, { record_id: id, expected_revision: 1, data: {}, changed_by: athlete }),
      () => database.deleteRecord(handle, { record_id: id, expected_revision: 1, deleted_by: athlete }),
      () => database.queryMeasurements(handle),
      () => database.querySeries(handle, { quantity: "heart_rate" }),
      () => database.querySessions(handle),
      () => database.queryRecordsAsOf(handle, { as_of: day(1) }),
      () => database.readSeriesView(handle, "heart_rate"),
      () => database.resolveCalendarRange(handle, {}),
      () => database.resolveAsOfTime(handle, undefined),
      () => database.checkDatabase(handle),
      () => database.databaseStats(handle),
      () => database.calculate(handle, { operation: "summary", quantity: "hrv_rmssd" }),
      () => database.planLogEntry(handle, { text: "felt good", actor: athlete }),
      () => database.applyLogPlan(handle, { inputs: [], expected_current_revisions: [], interpretation: "" }),
      () => database.buildEvidence(handle, { benchmark: "fran" }),
      () => database.exportRecords(handle, join(folder, "never")),
      () => database.previewRecordErasure(handle, id, 1),
      () => database.readDatabaseMetadata(handle),
      () => database.migrateDatabase(handle),
      () => database.applyPendingDatabaseMigrations(handle),
      () => database.executeConfirmedDatabaseMigration(handle),
      () => database.repairProviderEncoding(handle),
      () => database.countProviderEncodingRepairCandidates(handle),
      () => database.countRevisionAlignRepairCandidates(handle),
      () => database.listRevisionAlignRepairRecordIds(handle),
      () => database.executeConfirmedProviderEncodingRepair(handle),
      () => database.planPredictionGrade(handle, { prediction_id: id, result_id: id, actor: athlete }),
      () => database.resolveEvidenceReference(handle, { record_id: id, revision: 1 }),
      () => database.previewProviderImport(handle, folder),
    ];
    for (const call of calls) {
      await expect(call()).rejects.toThrow(closed);
    }
    // A handle this module did not make is treated as closed, as in 2.1.0.
    const forged = { url: "file:///x", isLocal: true, localPath: scenario.path, close() {} };
    await expect(database.readRecord(forged, id)).rejects.toThrow(closed);
  });

  it("keeps remote URL targets remote and refuses them as 2.1.0 did", async () => {
    for (const url of ["libsql://example.turso.io", "https://example.com/db", "ws://localhost:8080"]) {
      await expect(database.openDatabase(url)).rejects.toThrow("SQLite foreign-key enforcement could not be enabled");
      await expect(database.inspectDatabase(url)).rejects.toThrow("SQLite foreign-key enforcement could not be enabled");
      await expect(database.createDatabase(url)).rejects.toThrow("SQLite foreign-key enforcement could not be enabled");
    }
    await expect(database.openDatabase("sqlite://x/y")).rejects.toMatchObject({
      name: "LibsqlError",
      code: "URL_SCHEME_NOT_SUPPORTED",
    });
    await expect(database.createDatabase("libsql://x", { databaseId: "nope" })).rejects.toBeInstanceOf(ZodError);
    expect(existsSync(join(process.cwd(), "libsql:"))).toBe(false);
    await expect(
      database.writeV1BacktestReport("libsql://example.turso.io/db", {} as never),
    ).rejects.toThrow("not a remote URL");

    // Exports whose 2.1.0 code checked for a local file keep that refusal.
    let closed = false;
    const remote = { url: "libsql://example.turso.io", isLocal: false, close: () => (closed = true) };
    await database.releaseLocalDatabase(remote);
    expect(closed).toBe(true);
    expect(() => database.packageConflictSetPath(remote)).toThrow(
      "temporary import conflicts require a local SQLite database",
    );
    await expect(database.readPackageConflictSet(remote)).rejects.toThrow(
      "temporary import conflicts require a local SQLite database",
    );
    await database.clearPackageConflictSet(remote);
    expect(await database.savePackageConflictSet(remote, { conflicts: [] } as never)).toBeNull();
    expect(await database.prepareAttachments(remote, [], athlete, day(1))).toEqual([]);
    await expect(database.prepareAttachments(remote, ["x.png"], athlete, day(1))).rejects.toThrow(
      "attachment ingestion requires a local SQLite database",
    );
    await expect(database.createVerifiedSqliteBackup(remote, "x", "y", {} as never)).rejects.toThrow(
      "SQL schema migration backup requires a local database file",
    );
    await expect(database.readRecord(remote, scenario.fran.record_id)).rejects.toThrow(
      "Athletic Standard database is closed",
    );
  });

  it("makes and removes temporary folders", async () => {
    const made = await database.makeTemporaryDirectory("ath-npm-temp-");
    expect(existsSync(made)).toBe(true);
    writeFileSync(join(made, "file"), "x");
    await database.removeTemporaryDirectory(made);
    expect(existsSync(made)).toBe(false);
  });
});

describe("records", () => {
  it("writes, corrects, deletes, and reads history", async () => {
    const { handle } = scenario;
    const { record, database_revision } = await database.writeRecord(handle, {
      record_type: "athlete_report",
      data: noteData(day(2)),
      changed_by: athlete,
    });
    expect(record).toMatchObject({ record_type: "athlete_report", current_revision: 1, is_deleted: false });
    expect(database_revision).toBeGreaterThan(0);
    expect(await database.readRecord(handle, record.record_id)).toEqual(record);
    expect(await database.getRecord(handle, record.record_id)).toEqual(record);

    const corrected = await database.correctRecord(handle, {
      record_id: record.record_id,
      expected_revision: 1,
      data: { ...noteData(day(2)), reported_via: "voice" },
      changed_by: athlete,
    });
    expect(corrected.record).toMatchObject({ record_id: record.record_id, current_revision: 2 });
    await expect(
      database.correctRecord(handle, { record_id: record.record_id, expected_revision: 1, data: noteData(), changed_by: athlete }),
    ).rejects.toThrow(`Record ${record.record_id} is at revision 2, not expected revision 1`);

    const revisions = await database.readRecordRevisions(handle, record.record_id);
    expect(revisions.map((revision) => revision.revision)).toEqual([1, 2]);
    expect(revisions[1]!.previous_revision).toBe(1);

    const deleted = await database.deleteRecord(handle, {
      record_id: record.record_id,
      expected_revision: 2,
      deleted_by: athlete,
    });
    expect(deleted).toMatchObject({ record_id: record.record_id, record_revision: 2 });
    expect((await database.listRecords(handle, { record_type: "athlete_report" })).some((item) => item.record_id === record.record_id)).toBe(false);
    expect(
      (await database.listRecords(handle, { record_type: "athlete_report", include_deleted: true })).some(
        (item) => item.record_id === record.record_id,
      ),
    ).toBe(true);
    await expect(database.listRecords(handle, { limit: 0 })).rejects.toThrow("Record list limit must be between 1 and 10000");
    await expect(database.readRecord(handle, "not-a-uuid")).rejects.toBeInstanceOf(ZodError);
  });

  it("refuses a measured record without a source before writing", async () => {
    const before = (await database.databaseStats(scenario.handle)).database_revision;
    const complete = {
      source_id: scenario.source.record_id,
      received_at: day(9, 7),
      received_at_basis: "import_time",
      quantity: "hrv_rmssd",
      recorded_at: day(9),
      input_timestamp: {
        timestamp_missing_reason: "not_provided_by_source",
        utc_offset_missing_reason: "not_provided_by_source",
      },
      value: {
        standard_value: 44,
        standard_unit: "ms",
        input_value_missing_reason: "not_provided_by_source",
        input_unit_missing_reason: "not_provided_by_source",
      },
    };
    // With its source, the data passes every rule.
    const checked = await callAsync("model.validate", {
      schema: "PointMeasurementData",
      json_text: JSON.stringify(complete),
    });
    expect(checked.output).toMatchObject({ valid: true, issues: [] });
    const { source_id: _source, ...withoutSource } = complete;
    const error = await database
      .writeRecord(scenario.handle, {
        record_type: "point_measurement",
        data: withoutSource,
        changed_by: athlete,
      } as never)
      .then(
        () => undefined,
        (caught: unknown) => caught,
      );
    expect(error).toBeInstanceOf(ZodError);
    const issues = (error as ZodError).issues;
    expect(issues).toHaveLength(1);
    expect(issues[0]!.path.slice(-2)).toEqual(["data", "source_id"]);
    expect(issues[0]!.code).toBe("invalid_type");
    expect((await database.databaseStats(scenario.handle)).database_revision).toBe(before);
  });

  it("writes a batch as one database revision and keeps the caller's order", async () => {
    const before = (await database.databaseStats(scenario.handle)).database_revision;
    const written = await database.writeRecords(scenario.handle, [
      { record_type: "athlete_report", data: noteData(day(4)), changed_by: athlete },
      { record_type: "athlete_report", data: noteData(day(5)), changed_by: athlete },
    ]);
    expect(written.database_revision).toBe(before + 1);
    expect(written.records.map((record) => (record.data as { reported_at: string }).reported_at)).toEqual([day(4), day(5)]);
    await expect(database.writeRecords(scenario.handle, [])).rejects.toThrow("writeRecords needs at least one record");
  });

  it("writes corrections and new records together", async () => {
    const { record } = await database.writeRecord(scenario.handle, {
      record_type: "athlete_report",
      data: noteData(day(6)),
      changed_by: athlete,
    });
    const changed = await database.writeRecordChanges(scenario.handle, {
      corrections: [
        { record_id: record.record_id, expected_revision: 1, data: noteData(day(7)), changed_by: athlete },
      ],
      records: [{ record_type: "athlete_report", data: noteData(day(8)), changed_by: athlete }],
      expected_current_revisions: [{ record_id: scenario.fran.record_id, revision: 1 }],
    });
    expect(changed.records.map((item) => item.current_revision)).toEqual([2, 1]);
    await expect(database.writeRecordChanges(scenario.handle, {})).rejects.toThrow(
      "writeRecordChanges needs at least one change",
    );
    await expect(
      database.writeRecordChanges(scenario.handle, {
        corrections: [{ record_id: record.record_id, expected_revision: 1, data: noteData(), changed_by: athlete }],
      }),
    ).rejects.toThrow(`Record ${record.record_id} changed after preview; preview it again`);
  });

  it("reads links and renders records", async () => {
    const result = scenario.results[0]!;
    const links = await database.readRecordLinks(scenario.handle, result.record_id);
    expect(links).toEqual(
      expect.arrayContaining([
        { direction: "outgoing", relation: "source", record_id: scenario.source.record_id },
        { direction: "outgoing", relation: "benchmark_definition", record_id: scenario.fran.record_id },
      ]),
    );
    const revisions = await database.readRecordRevisions(scenario.handle, result.record_id);
    const text = database.renderRecord(result, { revision: revisions[0]!, history: revisions, links });
    expect(text).toContain(result.record_id);
    expect(text).toContain("showing revision: 1");
    expect(text).toContain("history:");
    expect(text).toContain("links:");
    const list = database.renderRecordList([scenario.fran, ...scenario.measurements]);
    expect(list.split("\n")[0]).toContain("record ID");
    expect(list).toContain("fran");
    expect(list).toContain("hrv_rmssd 42ms");
    expect(database.renderRecordList([])).toBe("no records matched");
  });

  it("makes lowercase UUIDv7 values", () => {
    expect(database.createUuidV7()).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    expect(database.createUuidV7(0).startsWith("00000000-0000-7")).toBe(true);
    expect(() => database.createUuidV7(-1)).toThrow("UUIDv7 timestamp is outside its 48-bit range");
    expect(() => database.createUuidV7(Number.NaN)).toThrow("UUIDv7 timestamp is outside its 48-bit range");
  });
});

describe("queries and views", () => {
  it("queries measurements, series, sessions, and history", async () => {
    const { handle } = scenario;
    const measurements = await database.queryMeasurements(handle, { quantity: "hrv_rmssd", from: day(1), to: day(3) });
    expect(measurements.map((record) => record.record_id)).toEqual(scenario.measurements.slice(1, 4).map((record) => record.record_id));
    expect(await database.queryMeasurements(handle)).toHaveLength(5);
    await expect(database.queryMeasurements(handle, { limit: 0 })).rejects.toThrow("Query limit must be between 1 and 100000");

    const series = await database.querySeries(handle, { quantity: "heart_rate", order: "descending", limit: 2 });
    expect(series.map((record) => record.record_id)).toEqual([scenario.samples[3]!.record_id, scenario.samples[2]!.record_id]);
    expect(await database.querySessions(handle, { session_type: "sleep_session" })).toEqual([scenario.sleep]);
    const asOf = await database.queryRecordsAsOf(handle, { as_of: day(1, 0), record_type: "point_measurement" });
    expect(asOf.map((record) => record.record_id)).toEqual([scenario.measurements[0]!.record_id]);
    await expect(
      database.queryRecordsAsOf(handle, { as_of: day(1), record_type: "source", record_types: ["source"] }),
    ).rejects.toThrow("Use record_type or record_types, not both");
  });

  it("reads and renders a calendar series view", async () => {
    const { handle } = scenario;
    const view = await database.readSeriesView(handle, "heart_rate", { time_zone: "UTC" });
    expect(view).toMatchObject({ quantity: "heart_rate", unit: "bpm", time_zone: "UTC", truncated: false });
    expect(view.days).toEqual([
      expect.objectContaining({ date: "2026-01-02", source_id: scenario.source.record_id, sample_count: 4, minimum: 58, maximum: 64 }),
    ]);
    expect(database.renderSeriesView(view)).toContain("heart_rate (bpm) by calendar day in UTC");
    const raw = await database.readSeriesView(handle, "heart_rate", { raw: true, limit: 2 });
    expect(raw.samples).toHaveLength(2);
    expect(raw.truncated).toBe(true);
    expect(database.renderSeriesView(raw)).toContain("raw samples");
    await expect(database.readSeriesView(handle, "heart_rate")).rejects.toThrow(
      "a calendar-day series view needs --timezone <IANA name> or a default timezone in the athlete profile",
    );
  });

  it("resolves calendar ranges and as-of times", async () => {
    const { handle } = scenario;
    expect(await database.resolveCalendarRange(handle, { from: "2026-03-08", to: "2026-03-08", time_zone: "America/Chicago" })).toEqual({
      from: "2026-03-08T06:00:00.000Z",
      to: "2026-03-09T04:59:59.999Z",
      time_zone: "America/Chicago",
    });
    expect(await database.resolveAsOfTime(handle, "2026-01-02", "UTC")).toEqual({
      as_of: "2026-01-02T23:59:59.999Z",
      time_zone: "UTC",
    });
    expect(await database.resolveAsOfTime(handle, day(1))).toEqual({ as_of: day(1) });
    expect((await database.resolveAsOfTime(handle, undefined)).as_of).toMatch(/Z$/);
    expect(database.calendarDateInZone("2026-10-02T03:00:00.000Z", "America/Chicago")).toBe("2026-10-01");
  });
});

describe("checks, statistics, and repair", () => {
  it("checks and summarizes the database", async () => {
    const check = await database.checkDatabase(scenario.handle);
    expect(check).toMatchObject({ valid: true, issues: [] });
    expect(check.metadata?.athleticstandard_version).toBe("2.1.2");
    const stats = await database.databaseStats(scenario.handle);
    expect(stats.current_records.by_type).toMatchObject({ point_measurement: 5, benchmark_result: 3 });
    expect(stats.benchmarks).toEqual({ defined: 1, results: 3 });
    const text = database.renderDatabaseStats(stats);
    expect(text).toContain("unnamed athlete · Athletic Standard 2.1.2");
    expect(text).toContain("benchmarks defined: 1 · results recorded: 3");
  });

  it("finds no repair work in a healthy database", async () => {
    expect(await database.countProviderEncodingRepairCandidates(scenario.handle)).toBe(0);
    expect(await database.countRevisionAlignRepairCandidates(scenario.handle)).toBe(0);
    expect(await database.listRevisionAlignRepairRecordIds(scenario.handle)).toEqual([]);
    const repaired = await database.repairProviderEncoding(scenario.handle);
    expect(repaired.repaired_record_count).toBe(0);
    const confirmed = await database.executeConfirmedProviderEncodingRepair(scenario.handle);
    expect(confirmed).toMatchObject({ backup_path: "", repaired_record_count: 0, revision_align_record_count: 0 });
  });

  it("repairs and migrates the npm 1.0.0 fixture with a verified backup", async () => {
    const copy = join(folder, "npm-100.ath.sqlite");
    copyFileSync(join(repositoryRoot, "tests", "fixtures", "npm-100-encoding", "athlete.ath.sqlite"), copy);
    await expect(database.openDatabase(copy)).rejects.toBeInstanceOf(database.MigrationRequiredError);
    try {
      await database.openDatabase(copy);
    } catch (error) {
      const required = error as InstanceType<typeof database.MigrationRequiredError>;
      expect(required.name).toBe("MigrationRequiredError");
      expect(required.preview.source_sql_schema_version).toBe(1);
      expect(required.preview.target_sql_schema_version).toBe(6);
      expect(required.message).toBe(
        "Database SQL schema 1 must be upgraded to 6 before other commands can run. Run ath migrate.",
      );
    }
    const handle = await database.openDatabase(copy, { allowOutdatedSchema: true, deferWal: true });
    const metadata = await database.readDatabaseMetadata(handle);
    expect(database.databaseNeedsMigration(metadata)).toBe(true);
    expect(await database.countProviderEncodingRepairCandidates(handle)).toBeGreaterThan(0);
    const preview = database.buildMigrationPreview(metadata, copy, new Date("2026-01-15T12:00:00.123Z"));
    expect(preview.planned_steps.map((step) => step.target_sql_schema_version)).toEqual([2, 3, 4, 5, 6]);
    expect(preview.planned_backup_path).toMatch(/\.migration-backup\.20260115T120000\.123Z-\d+\.sqlite$/);

    const backup = join(folder, "npm-100.backup.sqlite");
    await database.createVerifiedSqliteBackup(handle, copy, backup, metadata);
    expect(existsSync(backup)).toBe(true);

    const result = await database.executeConfirmedDatabaseMigration(handle, {
      backupPath: join(folder, "npm-100.migration-backup.sqlite"),
    });
    expect(result).toMatchObject({ migrated: true, backup_path: join(folder, "npm-100.migration-backup.sqlite") });
    expect(database.databaseNeedsMigration(await database.readDatabaseMetadata(handle))).toBe(false);
    expect((await database.checkDatabase(handle)).valid).toBe(true);
    handle.close();

    // Unbacked migration for callers who made their own backup.
    const second = join(folder, "npm-100-unbacked.ath.sqlite");
    copyFileSync(join(repositoryRoot, "tests", "fixtures", "npm-100-encoding", "athlete.ath.sqlite"), second);
    const other = await database.openDatabase(second, { allowOutdatedSchema: true });
    await database.migrateDatabase(other);
    expect((await database.readDatabaseMetadata(other)).sql_schema_version).toBe(6);
    await database.applyPendingDatabaseMigrations(other);
    other.close();
  });

  it("names backups as 2.1.0 did", () => {
    expect(database.formatMigrationBackupTimestamp(new Date("2026-01-15T12:00:00.123Z"))).toBe("20260115T120000.123Z");
    const first = database.defaultMigrationBackupPath("/data/a.ath.sqlite", new Date("2026-01-15T12:00:00.123Z"));
    const second = database.defaultMigrationBackupPath("/data/a.ath.sqlite", new Date("2026-01-15T12:00:00.123Z"));
    expect(first).toMatch(/^\/data\/a\.ath\.sqlite\.migration-backup\.20260115T120000\.123Z-\d+\.sqlite$/);
    expect(second).not.toBe(first);
    expect(database.sqlite3BackupDotCommand('/tmp/it\'s "x"\\y')).toBe('.backup "/tmp/it\'s \\"x\\"\\\\y"\n');
    expect(() => database.formatMigrationBackupTimestamp(new Date(Number.NaN))).toThrow(RangeError);
  });
});

describe("calculations", () => {
  it("calculates and renders a per-source summary", async () => {
    const result = await database.calculate(scenario.handle, { operation: "summary", quantity: "hrv_rmssd" });
    expect(result).toMatchObject({
      calculation_version: "ath-runtime-v1",
      operation: "summary",
      record_kind: "measurement",
      quantity: "hrv_rmssd",
      unit: "ms",
      sources: [scenario.source.record_id],
    });
    expect(result.input.record_count).toBe(5);
    const text = database.renderCalculation(result);
    expect(text.split("\n")[0]).toBe("summary: measurement hrv_rmssd (ms)");
    expect(text).toContain("inputs: 5 records");
    await expect(
      database.calculate(scenario.handle, { operation: "workout-drift", quantity: "heart_rate" }),
    ).rejects.toThrow("workout-drift needs --session <workout-session-id>");
  });
});

describe("logging and attachments", () => {
  it("plans and applies a structured log entry", async () => {
    const plan = await database.planLogEntry(scenario.handle, {
      text: JSON.stringify({ record_type: "athlete_report", data: noteData(day(9)) }),
      actor: athlete,
      created_at: day(9),
    });
    expect(plan.interpretation).toBe("1 structured record");
    expect(plan.inputs[0]).toMatchObject({ record_type: "athlete_report", created_at: day(9), changed_by: athlete });
    const applied = await database.applyLogPlan(scenario.handle, plan);
    expect(applied.records).toHaveLength(1);
    await expect(database.planLogEntry(scenario.handle, { text: "hrv 50", actor: athlete })).rejects.toThrow(
      "HRV is ambiguous",
    );
  });

  it("prepares attachments and writes them with the record that names them", async () => {
    const png = join(folder, "meal.png");
    writeFileSync(png, Buffer.from("89504e470d0a1a0a0000000d49484452", "hex"));
    expect(await database.prepareAttachments(scenario.handle, [], athlete, day(10))).toEqual([]);
    const prepared = await database.prepareAttachments(scenario.handle, [png], athlete, day(10));
    expect(prepared).toHaveLength(1);
    expect(prepared[0]!.input).toMatchObject({ record_type: "attachment", created_at: day(10) });
    expect(prepared[0]!.bytes.equals(readFileSync(png))).toBe(true);

    const plan = await database.planLogEntry(scenario.handle, {
      text: JSON.stringify({ record_type: "athlete_report", data: noteData(day(10)) }),
      actor: athlete,
      created_at: day(10),
    });
    const withFiles = database.addAttachmentsToLogPlan(plan, prepared);
    expect(withFiles.interpretation).toBe("1 structured record with 1 attachment");
    expect(withFiles.inputs[0]!.record_type).toBe("attachment");
    const written = await database.withPreparedAttachmentFiles(scenario.handle, prepared, () =>
      database.applyLogPlan(scenario.handle, withFiles),
    );
    expect(written.records.map((record) => record.record_type)).toEqual(["attachment", "athlete_report"]);
    expect(existsSync(prepared[0]!.final_path)).toBe(true);
    expect(readFileSync(prepared[0]!.final_path).equals(readFileSync(png))).toBe(true);
    expect((await database.checkDatabase(scenario.handle)).valid).toBe(true);

    const text = join(folder, "notes.txt");
    writeFileSync(text, "plain");
    await expect(database.prepareAttachments(scenario.handle, [text], athlete, day(10))).rejects.toThrow(
      "use PNG, JPEG, WebP, or PDF",
    );
    expect(await database.withPreparedAttachmentFiles(scenario.handle, [], async () => 7)).toBe(7);
  });
});

describe("grades, evidence, and backtests", () => {
  it("plans and writes a grade", async () => {
    const plan = await database.planPredictionGrade(scenario.handle, {
      prediction_id: scenario.prediction.record_id,
      result_id: scenario.results[2]!.record_id,
      actor: athlete,
      created_at: day(5),
    });
    expect(plan.existing_link).toBeNull();
    expect(plan.link_input).toMatchObject({ record_type: "prediction_result_link" });
    expect(plan.grade_input).toMatchObject({ record_type: "prediction_grade" });
    const graded = await database.gradePrediction(scenario.handle, plan);
    expect(graded.link.record_type).toBe("prediction_result_link");
    expect(graded.grade.record_type).toBe("prediction_grade");
    expect((graded.grade.data as { signed_error: number }).signed_error).toBe(5);
  });

  it("builds offline evidence and resolves evidence references", async () => {
    const evidence = await database.buildEvidence(scenario.handle, { benchmark: "fran", as_of: day(3) });
    expect(evidence).toMatchObject({ purpose: "prediction_evidence", is_prediction: false, as_of: day(3) });
    expect(evidence.benchmark.record_id).toBe(scenario.fran.record_id);
    expect(evidence.prior_results.map((record) => record.record_id)).toEqual(
      scenario.results.slice(0, 2).map((record) => record.record_id),
    );
    const available = await database.resolveEvidenceReference(scenario.handle, {
      record_id: scenario.fran.record_id,
      revision: 1,
    });
    expect(available.status).toBe("available");
    const missing = await database.resolveEvidenceReference(scenario.handle, {
      record_id: scenario.fran.record_id,
      revision: 9,
    });
    expect(missing).toEqual({ status: "missing", reference: { record_id: scenario.fran.record_id, revision: 9 } });
  });

  it("runs a backtest with an injected completion function and writes its report", async () => {
    const requests: string[] = [];
    const before = (await database.databaseStats(scenario.handle)).database_revision;
    const report = await database.runV1Backtest(scenario.handle, {
      models: ["model-a", "model-b"],
      gateway: "openrouter",
      created_at: "2026-02-01T00:00:00.000Z",
      complete: async (request) => {
        requests.push(request.model);
        if (request.model === "model-b") {
          throw new Error("model-b is offline");
        }
        return { content: JSON.stringify({ predicted_score: { duration_s: 285 } }), model: request.model };
      },
    });
    expect(requests.length).toBeGreaterThan(0);
    expect(report.athleticstandard_version).toBe("2.1.2");
    expect(report.skipped_first_result_count).toBe(1);
    expect(report.replayable_result_count).toBe(2);
    expect(report.replays.filter((replay) => replay.model === "model-b").every((replay) => replay.error === "model-b is offline")).toBe(true);
    expect(report.summary).toEqual([
      expect.objectContaining({ model: "model-a", completed: 2, failed: 0 }),
      expect.objectContaining({ model: "model-b", completed: 0, failed: 2, median_absolute_error_percentage: null }),
    ]);
    expect((await database.databaseStats(scenario.handle)).database_revision).toBe(before);
    const path = await database.writeV1BacktestReport(scenario.path, report);
    expect(path).toBe(join(folder, "backtest-2026-02-01T000000000Z.json"));
    expect(JSON.parse(readFileSync(path, "utf8"))).toEqual(report);
  });
});

describe("export, package import, and conflicts", () => {
  it("exports deterministically and restores into an empty database", async () => {
    const out = join(folder, "export-a");
    const exported = await database.exportRecords(scenario.handle, out, { exported_at: "2026-02-02T00:00:00.000Z" });
    expect(exported.directory).toBe(out);
    expect(exported.manifest.exported_at).toBe("2026-02-02T00:00:00.000Z");
    const loaded = await database.loadAthleticStandardPackage(out);
    expect(loaded.source_path).toBe(out);
    expect(loaded.all_records).toBe(loaded.package.records);
    expect(loaded.all_records.some((record) => record.record_id === scenario.fran.record_id)).toBe(true);
    expect(loaded.attachment_files).toBeInstanceOf(Map);
    expect(loaded.attachment_files.size).toBe(1);
    expect(await database.readImportedPackageRecord(loaded, scenario.fran.record_id)).toEqual(
      loaded.all_records.find((record) => record.record_id === scenario.fran.record_id),
    );
    expect(await database.readImportedPackageRecord(loaded, scenario.samples[0]!.record_id)).toMatchObject({
      record_id: scenario.samples[0]!.record_id,
    });
    expect(await database.readImportedPackageRecord(loaded, "01890f3e-e7d3-7cc0-88c8-0000000000ff")).toBeNull();

    const empty = await database.createDatabase(join(folder, "restore.ath.sqlite"));
    const preview = await database.previewPackageImport(empty, loaded);
    expect(preview).toMatchObject({ restore: true, conflicts: [], loaded });
    const imported = await database.importRecords(empty, preview);
    expect(imported.restored).toBe(true);
    expect((await database.readDatabaseMetadata(empty)).database_id).toBe(exported.database_id);
    expect((await database.checkDatabase(empty)).valid).toBe(true);
    // The handle follows the restored database ID.
    expect(await database.readRecord(empty, scenario.fran.record_id)).toEqual(scenario.fran);
    empty.close();
  });

  it("saves, reads, resolves, and clears package conflicts", async () => {
    // Two copies of one database change the same record differently.
    const base = join(folder, "export-base");
    await database.exportRecords(scenario.handle, base);
    const copy = await database.createDatabase(join(folder, "conflicts.ath.sqlite"));
    await database.importRecords(copy, await database.previewPackageImport(copy, await database.loadAthleticStandardPackage(base)));
    const result = scenario.results[0]!;
    const changeScore = async (handle: typeof copy, seconds: number): Promise<void> => {
      const current = await database.readRecord(handle, result.record_id);
      await database.correctRecord(handle, {
        record_id: result.record_id,
        expected_revision: current!.current_revision,
        data: { ...(current!.data as object), score: { duration_s: seconds } },
        changed_by: athlete,
      });
    };
    await changeScore(scenario.handle, 299);
    await changeScore(copy, 295);
    const theirs = join(folder, "export-theirs");
    await database.exportRecords(scenario.handle, theirs);

    const conflictPath = database.packageConflictSetPath(copy);
    expect(conflictPath).toBe(`${copy.localPath}.import-conflicts.json`);
    const loaded = await database.loadAthleticStandardPackage(theirs);
    const preview = await database.previewPackageImport(copy, loaded);
    expect(preview.conflicts.map((conflict) => conflict.record_id)).toContain(result.record_id);
    await expect(database.importRecords(copy, preview)).rejects.toThrow(
      "import has unresolved conflicts or unsupported records",
    );

    const state = await database.savePackageConflictSet(copy, preview);
    expect(state).toMatchObject({ conflict_set_version: 1, source_path: theirs, source_fingerprint: loaded.source_fingerprint });
    expect(await database.readPackageConflictSet(copy)).toEqual(state);
    await database.assertPackageConflictSetCurrent(copy, state!, preview);

    const shown = await database.previewConflict(copy, preview, result.record_id);
    expect(shown.local?.record_id).toBe(result.record_id);
    expect((shown.imported?.data as { score: unknown }).score).toEqual({ duration_s: 299 });
    await expect(database.previewConflict(copy, preview, scenario.fran.record_id)).rejects.toThrow(
      "record is not in the package conflict preview",
    );

    let next = state!;
    for (const conflict of state!.conflicts) {
      next = await database.resolveConflict(copy, next, conflict.record_id, {
        record_id: conflict.record_id,
        choice: "imported",
      });
    }
    expect(database.conflictResolutions(next)).toEqual(
      state!.conflicts.map((conflict) => ({ record_id: conflict.record_id, choice: "imported" })),
    );
    expect(await database.readPackageConflictSet(copy)).toEqual(next);
    const imported = await database.importRecords(copy, preview, database.conflictResolutions(next));
    expect(imported.restored).toBe(false);
    expect(((await database.readRecord(copy, result.record_id))!.data as { score: unknown }).score).toEqual({
      duration_s: 299,
    });
    await expect(database.assertPackageConflictSetCurrent(copy, next, preview)).rejects.toThrow(
      "the package or database changed after the conflict preview; run ath import again",
    );

    // The conflict file needs no open database, as in 2.1.0.
    copy.close();
    await database.clearPackageConflictSet(copy);
    expect(existsSync(conflictPath)).toBe(false);
    await expect(database.readPackageConflictSet(copy)).rejects.toThrow("no temporary import conflicts");
  });

  it("refuses a package this version cannot represent", async () => {
    const out = join(folder, "export-unknown");
    await database.exportRecords(scenario.handle, out);
    const documentPath = join(out, "athlete.ath.json");
    const document = JSON.parse(readFileSync(documentPath, "utf8")) as { records: Array<Record<string, unknown>> };
    document.records[0]!.invented_field = 1;
    writeFileSync(documentPath, `${JSON.stringify(document, null, 2)}\n`);
    try {
      await database.loadAthleticStandardPackage(out);
      expect.unreachable();
    } catch (error) {
      expect(error).toBeInstanceOf(database.PackageRepresentationError);
      const refusal = error as InstanceType<typeof database.PackageRepresentationError>;
      expect(refusal.name).toBe("PackageRepresentationError");
      expect(refusal.message).toMatch(/^package has unknown fields or records that v1 cannot represent/);
      expect(refusal.unknown_fields.length + refusal.unrepresentable_records.length).toBeGreaterThan(0);
    }
  });

  it("writes deterministic JSON as 2.1.0 did", () => {
    expect(database.deterministicJson({ b: 1, a: [1, { d: 2, c: 3 }], 10: "x", 2: "y" }).toString("utf8")).toBe(
      '{\n  "2": "y",\n  "10": "x",\n  "a": [\n    1,\n    {\n      "c": 3,\n      "d": 2\n    }\n  ],\n  "b": 1\n}\n',
    );
    expect(database.deterministicJson(undefined).toString("utf8")).toBe("undefined\n");
    expect(Buffer.isBuffer(database.deterministicJson(1))).toBe(true);
  });
});

describe("privacy erasure", () => {
  it("previews and erases a record with its dependents", async () => {
    const { record } = await database.writeRecord(scenario.handle, {
      record_type: "athlete_report",
      data: noteData(day(11)),
      changed_by: athlete,
    });
    const preview = await database.previewRecordErasure(scenario.handle, record.record_id, 1);
    expect(preview).toMatchObject({ root_record_id: record.record_id, root_expected_revision: 1, attachments: [] });
    expect(preview.records).toEqual([{ record_id: record.record_id, record_type: "athlete_report", current_revision: 1 }]);
    const erased = await database.eraseRecord(scenario.handle, {
      preview,
      reason_code: "athlete_request",
      erased_at: day(12),
    });
    expect(erased.erased_record_ids).toEqual([record.record_id]);
    expect(await database.readRecord(scenario.handle, record.record_id)).toBeNull();
    const resolution = await database.resolveEvidenceReference(scenario.handle, { record_id: record.record_id, revision: 1 });
    expect(resolution.status).toBe("erased");
  });
});

describe("provider imports", () => {
  it("previews and imports the WHOOP fixture, then matches it on repeat", async () => {
    const handle = await database.createDatabase(join(folder, "whoop.ath.sqlite"));
    const exportPath = join(repositoryRoot, "tests", "fixtures", "exports", "whoop");
    const preview = await database.previewProviderImport(handle, exportPath, { received_at: "2026-03-01T00:00:00.000Z" });
    expect(preview.provider).toBe("whoop");
    expect(preview.new_records.length).toBeGreaterThan(0);
    expect(preview.plan.received_at).toBe("2026-03-01T00:00:00.000Z");
    expect(preview.plan.drafts).toHaveLength(preview.changes.length);
    const imported = await database.importProviderRecords(handle, preview);
    expect(imported.new_record_count).toBe(preview.new_records.length);
    const again = await database.previewProviderImport(handle, exportPath, { received_at: "2026-03-01T00:00:00.000Z" });
    expect(again.new_records).toEqual([]);
    expect(again.matching_records.length).toBe(preview.new_records.length);
    // A preview whose database changed is refused.
    await database.writeRecord(handle, { record_type: "athlete_report", data: noteData(), changed_by: athlete });
    await expect(database.importProviderRecords(handle, again)).rejects.toThrow(
      "Database changed after provider import preview; preview it again",
    );
    handle.close();
  });

  it("refuses a preview whose export changed even when its counts did not", async () => {
    const handle = await database.createDatabase(join(folder, "whoop-changed.ath.sqlite"));
    const exportPath = join(folder, "whoop-changed");
    mkdirSync(exportPath);
    const fixture = join(repositoryRoot, "tests", "fixtures", "exports", "whoop");
    for (const name of ["physiological_cycles.csv", "sleeps.csv", "workouts.csv", "journal_entries.csv"]) {
      copyFileSync(join(fixture, name), join(exportPath, name));
    }
    const preview = await database.previewProviderImport(handle, exportPath, { received_at: "2026-03-01T00:00:00.000Z" });
    expect(Object.keys(preview)).not.toContain("input_fingerprint");
    const cycles = join(exportPath, "physiological_cycles.csv");
    writeFileSync(cycles, readFileSync(cycles, "utf8").replace(",68.4,", ",69.4,"));
    await expect(database.importProviderRecords(handle, preview)).rejects.toThrow(
      "The provider export changed after its import preview; preview it again",
    );
    expect(await database.listRecords(handle)).toEqual([]);
    handle.close();
  });

  it("refuses a provider preview it did not make", async () => {
    await expect(
      database.importProviderRecords(scenario.handle, { plan: { drafts: [], received_at: day(1) } } as never),
    ).rejects.toThrow("preview it again");
  });
});

describe("files the 2.1.0 helpers wrote", () => {
  it("keeps a backtest report from replacing an existing file", async () => {
    const reportFolder = join(folder, "reports");
    mkdirSync(reportFolder);
    const report = { created_at: "2026-02-03T00:00:00.000Z" } as never;
    await database.writeV1BacktestReport(join(reportFolder, "a.ath.sqlite"), report);
    await expect(database.writeV1BacktestReport(join(reportFolder, "a.ath.sqlite"), report)).rejects.toThrow();
  });
});
