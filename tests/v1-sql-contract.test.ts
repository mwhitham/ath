import { readFileSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { createClient } from "@libsql/client";
import { pathToFileURL } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { checkSchemaDrift } from "../scripts/check-schema-drift.js";
import {
  STABLE_VIEWS,
  STABLE_WRITABLE_TABLES,
  buildSqlContractDocument,
} from "../src/db/sql-contract.js";
import {
  correctRecord,
  createDatabase,
  migrateDatabase,
  readDatabaseMetadata,
  readRecord,
  SQL_SCHEMA_VERSION,
  writeRecord,
} from "../src/db/index.js";
import {
  readApplicationId,
  readUserVersion,
  SQLITE_APPLICATION_ID,
} from "../src/db/client.js";
import type { AthleticDatabase } from "../src/db/index.js";

const databaseId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f10";
const createdAt = "2026-08-09T12:00:00.000Z";
const sourceId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02";
const summaryId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e20";

const toolActor = {
  author_type: "tool" as const,
  tool: "ath",
  tool_version: "1.0.0",
};

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];

async function tempDatabasePath(): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), "ath-v1-sql-contract-"));
  directories.push(directory);
  return join(directory, "athlete.ath.sqlite");
}

afterEach(async () => {
  for (const database of openDatabases.splice(0)) {
    database.close();
  }
  for (const directory of directories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

describe("v1 SQL contract", () => {
  it("passes schema drift checks", async () => {
    await checkSchemaDrift();
  });

  it("creates schema version 1 from zero and migrateDatabase is a no-op", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    const metadata = await readDatabaseMetadata(database);
    expect(metadata.sql_schema_version).toBe(SQL_SCHEMA_VERSION);
    expect(await readUserVersion(database)).toBe(SQL_SCHEMA_VERSION);
    await migrateDatabase(database);
  });

  it("exposes stable read views to plain sqlite3 clients", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    await writeRecord(database, {
      record_type: "source",
      record_id: sourceId,
      created_at: createdAt,
      changed_by: toolActor,
      data: {
        source_type: "wearable",
        data_writer: "WHOOP",
        data_route: "whoop_csv",
        vendor: "whoop",
      },
    });

    const query = spawnSync(
      "sqlite3",
      [path, "SELECT COUNT(*) FROM sql_v1_active_records"],
      { encoding: "utf8" },
    );
    expect(query.status).toBe(0);
    expect(query.stdout.trim()).toBe("1");

    const info = spawnSync(
      "sqlite3",
      [path, "SELECT database_revision FROM sql_v1_database_info"],
      { encoding: "utf8" },
    );
    expect(info.status).toBe(0);
    expect(Number(info.stdout.trim())).toBeGreaterThan(0);
  });

  it("lists stable views and writable tables in contract.json", () => {
    const contract = JSON.parse(
      readFileSync(join(process.cwd(), "spec/sql/contract.json"), "utf8"),
    );
    expect(contract).toEqual(buildSqlContractDocument());
    expect(contract.stable_views.map((view: { name: string }) => view.name)).toEqual(
      STABLE_VIEWS.map((view) => view.name),
    );
    expect(contract.stable_writable_tables).toEqual([...STABLE_WRITABLE_TABLES]);
  });

  it("sets application_id and user_version per the initialization contract", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    expect(await readApplicationId(database)).toBe(SQLITE_APPLICATION_ID);
    expect(await readUserVersion(database)).toBe(SQL_SCHEMA_VERSION);
  });

  it("resets defer_foreign_keys after commit and requires it inside each write transaction", async () => {
    const path = await tempDatabasePath();
    await createDatabase(path, { databaseId, createdAt }).then((database) =>
      database.close(),
    );

    const client = createClient({ url: pathToFileURL(path).href });
    await client.execute("PRAGMA foreign_keys = ON");
    await client.execute("PRAGMA busy_timeout = 5000");

    const outsideDefer = await client.execute("PRAGMA defer_foreign_keys");
    expect(Number(outsideDefer.rows[0]?.defer_foreign_keys)).toBe(0);

    const recordId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e21";
    const transaction = await client.transaction("write");
    await transaction.execute("PRAGMA foreign_keys = ON");
    await transaction.execute("PRAGMA defer_foreign_keys = ON");
    const insideDefer = await transaction.execute("PRAGMA defer_foreign_keys");
    expect(Number(insideDefer.rows[0]?.defer_foreign_keys)).toBe(1);

    const actorJson = JSON.stringify(toolActor);
    const sourceData = JSON.stringify({
      source_type: "wearable",
      data_writer: "WHOOP",
      data_route: "whoop_csv",
      vendor: "whoop",
    });
    await transaction.execute({
      sql: `
        INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted)
        VALUES (?, 'source', 1, ?, 0)
      `,
      args: [recordId, createdAt],
    });
    await transaction.execute({
      sql: `
        INSERT INTO record_revisions (
          record_id, revision, record_type, record_json, extensions_json,
          became_known_at, became_known_at_basis, changed_by_json, previous_revision
        ) VALUES (?, 1, 'source', ?, NULL, ?, 'local_write', ?, NULL)
      `,
      args: [recordId, sourceData, createdAt, actorJson],
    });
    await transaction.execute({
      sql: `
        INSERT INTO sources (
          record_id, record_type, source_type, data_writer, data_route, vendor, data_json
        ) VALUES (?, 'source', 'wearable', 'WHOOP', 'whoop_csv', 'whoop', ?)
      `,
      args: [recordId, sourceData],
    });
    await transaction.execute(
      "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
    );
    await transaction.commit();
    transaction.close();

    const afterDefer = await client.execute("PRAGMA defer_foreign_keys");
    expect(Number(afterDefer.rows[0]?.defer_foreign_keys)).toBe(0);

    const count = await client.execute({
      sql: "SELECT COUNT(*) AS count FROM sql_v1_active_records WHERE record_id = ?",
      args: [recordId],
    });
    expect(Number(count.rows[0]?.count)).toBe(1);
    await client.close();
  });

  it("replaces child collections on correction per the published recipe", async () => {
    const path = await tempDatabasePath();
    const database = await createDatabase(path, { databaseId, createdAt });
    openDatabases.push(database);
    await writeRecord(database, {
      record_type: "source",
      record_id: sourceId,
      created_at: createdAt,
      changed_by: toolActor,
      data: {
        source_type: "wearable",
        data_writer: "WHOOP",
        data_route: "whoop_csv",
        vendor: "whoop",
      },
    });

    const periodA = {
      period_type: "oura_activity_day",
      provider_date: "2026-08-08",
    };
    const periodB = {
      period_type: "oura_activity_day",
      provider_date: "2026-08-07",
    };
    await writeRecord(database, {
      record_type: "provider_summary",
      record_id: summaryId,
      created_at: createdAt,
      changed_by: toolActor,
      data: {
        source_id: sourceId,
        received_at: createdAt,
        received_at_basis: "import_time",
        summary_type: "activity",
        provider_reporting_periods: [periodA, periodB],
        value: {
          standard_value: 80,
          standard_unit: "score",
          input_value: 80,
          input_unit: "score",
        },
        input_timestamp: {
          timestamp_missing_reason: "not_provided_by_source",
          utc_offset_missing_reason: "not_provided_by_source",
        },
      },
    });

    const client = createClient({ url: pathToFileURL(path).href });
    await client.execute("PRAGMA foreign_keys = ON");
    let periodCount = await client.execute({
      sql: "SELECT COUNT(*) AS count FROM provider_reporting_periods WHERE owner_record_id = ?",
      args: [summaryId],
    });
    expect(Number(periodCount.rows[0]?.count)).toBe(2);

    const correctedData = {
      source_id: sourceId,
      received_at: createdAt,
      received_at_basis: "import_time",
      summary_type: "activity",
      provider_reporting_periods: [periodA],
      value: {
        standard_value: 81,
        standard_unit: "score",
        input_value: 81,
        input_unit: "score",
      },
      input_timestamp: {
        timestamp_missing_reason: "not_provided_by_source",
        utc_offset_missing_reason: "not_provided_by_source",
      },
    };
    const actorJson = JSON.stringify(toolActor);
    const correctedJson = JSON.stringify(correctedData);
    const becameKnownAt = "2026-08-09T13:00:00.000Z";

    const transaction = await client.transaction("write");
    await transaction.execute("PRAGMA foreign_keys = ON");
    await transaction.execute("PRAGMA defer_foreign_keys = ON");
    await transaction.execute({
      sql: `
        INSERT INTO record_revisions (
          record_id, revision, record_type, record_json, extensions_json,
          became_known_at, became_known_at_basis, changed_by_json, previous_revision
        ) VALUES (?, 2, 'provider_summary', ?, NULL, ?, 'local_write', ?, 1)
      `,
      args: [summaryId, correctedJson, becameKnownAt, actorJson],
    });
    await transaction.execute({
      sql: "UPDATE records SET current_revision = 2 WHERE record_id = ?",
      args: [summaryId],
    });
    await transaction.execute({
      sql: `
        INSERT INTO provider_summaries (
          record_id, record_type, source_id, summary_type, recorded_at,
          standard_value, standard_unit, provider_record_id, received_at, data_json
        ) VALUES (?, 'provider_summary', ?, 'activity', NULL, 81, 'score', NULL, ?, ?)
        ON CONFLICT(record_id) DO UPDATE SET
          source_id = excluded.source_id,
          summary_type = excluded.summary_type,
          recorded_at = excluded.recorded_at,
          standard_value = excluded.standard_value,
          standard_unit = excluded.standard_unit,
          provider_record_id = excluded.provider_record_id,
          received_at = excluded.received_at,
          data_json = excluded.data_json
      `,
      args: [summaryId, sourceId, createdAt, correctedJson],
    });
    await transaction.execute({
      sql: "DELETE FROM provider_reporting_periods WHERE owner_record_id = ?",
      args: [summaryId],
    });
    await transaction.execute({
      sql: `
        INSERT INTO provider_reporting_periods (
          owner_record_id, period_order, period_type, provider_period_id,
          provider_date, start_at, end_at, period_json
        ) VALUES (?, 0, ?, NULL, ?, NULL, NULL, ?)
      `,
      args: [summaryId, periodA.period_type, periodA.provider_date, JSON.stringify(periodA)],
    });
    await transaction.execute(
      "UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1",
    );
    await transaction.commit();
    transaction.close();

    periodCount = await client.execute({
      sql: "SELECT COUNT(*) AS count FROM provider_reporting_periods WHERE owner_record_id = ?",
      args: [summaryId],
    });
    expect(Number(periodCount.rows[0]?.count)).toBe(1);

    const official = await readRecord(database, summaryId);
    expect(official?.record_type).toBe("provider_summary");
    expect(official?.current_revision).toBe(2);
    if (official?.record_type !== "provider_summary") {
      throw new Error("expected provider_summary");
    }
    expect(official.data.provider_reporting_periods).toEqual([periodA]);
    expect(official.data.value.standard_value).toBe(81);

    const matchesOfficial = await correctRecord(database, {
      record_id: summaryId,
      expected_revision: 2,
      changed_by: toolActor,
      became_known_at: "2026-08-09T14:00:00.000Z",
      data: correctedData,
    });
    expect(matchesOfficial.record.current_revision).toBe(3);
    await client.close();
  });

  it("still enforces triggers when foreign keys are disabled on the connection", async () => {
    const path = await tempDatabasePath();
    await createDatabase(path, { databaseId, createdAt }).then((database) =>
      database.close(),
    );

    const client = createClient({ url: pathToFileURL(path).href });
    await client.execute("PRAGMA foreign_keys = OFF");
    await expect(
      client.execute({
        sql: `
          INSERT INTO record_revisions (
            record_id,
            revision,
            record_type,
            record_json,
            extensions_json,
            became_known_at,
            became_known_at_basis,
            changed_by_json,
            previous_revision
          ) VALUES (?, 1, 'athlete_report', ?, NULL, ?, 'local_write', ?, NULL)
        `,
        args: [
          "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e99",
          JSON.stringify({
            report_type: "soreness",
            reported_at: createdAt,
            source_id: sourceId,
          }),
          createdAt,
          JSON.stringify(toolActor),
        ],
      }),
    ).rejects.toThrow();
    await client.close();
  });
});
