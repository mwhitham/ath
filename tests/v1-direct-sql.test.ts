import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import { afterEach, describe, expect, it } from "vitest";
import { createDatabase, writeRecord } from "../src/db/index.js";

const databaseId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f01";
const createdAt = "2026-08-09T12:00:00.000Z";
const sourceId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02";
const reportId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e04";

const timestampReceipt = {
  timestamp_text: "2026-08-09T06:12:00-05:00",
  utc_offset_text: "-05:00",
  utc_offset_meaning: "provider_cycle_offset" as const,
  timestamp_basis: "source_offset_timestamp" as const,
};

const toolActor = {
  author_type: "tool" as const,
  tool: "ath",
  tool_version: "1.0.0",
};

const toolActorJson = JSON.stringify(toolActor);

const directories: string[] = [];

async function tempPath(): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), "ath-v1-direct-sql-"));
  directories.push(directory);
  return join(directory, "athlete.ath.sqlite");
}

async function sqlClient(path: string) {
  const client = createClient({ url: pathToFileURL(path).href });
  await client.execute("PRAGMA foreign_keys = ON");
  return client;
}

function athleteReportData(extra: Record<string, unknown> = {}) {
  return {
    report_type: "soreness",
    reported_at: createdAt,
    input_reported_at: timestampReceipt,
    reported_via: "text" as const,
    ...extra,
  };
}

async function seededDatabase(): Promise<string> {
  const path = await tempPath();
  const database = await createDatabase(path, { databaseId, createdAt });
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
  await database.close();
  return path;
}

describe("v1 direct SQLite enforcement", () => {
  afterEach(async () => {
    for (const directory of directories.splice(0)) {
      await rm(directory, { recursive: true, force: true });
    }
  });

  async function expectBadRevisionInsert(
    path: string,
    recordJson: string,
    extensionsJson: string | null,
  ): Promise<void> {
    const client = await sqlClient(path);
    const transaction = await client.transaction("write");
    try {
      await transaction.execute({
        sql: `
          INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted)
          VALUES (?, 'athlete_report', 1, ?, 0)
        `,
        args: [reportId, createdAt],
      });
      await transaction.execute({
        sql: `
          INSERT INTO record_revisions (
            record_id, revision, record_type, record_json, extensions_json,
            became_known_at, became_known_at_basis, changed_by_json, previous_revision
          ) VALUES (?, 1, 'athlete_report', ?, ?, ?, 'local_write', ?, NULL)
        `,
        args: [reportId, recordJson, extensionsJson, createdAt, toolActorJson],
      });
      await transaction.commit();
      throw new Error("expected revision insert to fail");
    } catch (error) {
      await transaction.rollback();
      expect((error as Error).message).toMatch(/source_id/);
    } finally {
      transaction.close();
      await client.close();
    }
  }

  it("refuses athlete-reported record data that claims a source", async () => {
    const path = await seededDatabase();
    await expectBadRevisionInsert(
      path,
      JSON.stringify(athleteReportData({ source_id: sourceId })),
      null,
    );
  });

  it("refuses extensions that smuggle source_id onto athlete-reported records", async () => {
    const path = await seededDatabase();
    await expectBadRevisionInsert(
      path,
      JSON.stringify(athleteReportData()),
      JSON.stringify({ "ath.fit/demo-widget": { source_id: sourceId } }),
    );
  });

  it("refuses a revision whose record_type disagrees with the parent record", async () => {
    const path = await seededDatabase();
    const client = await sqlClient(path);
    await expect(
      client.execute({
        sql: `
          INSERT INTO record_revisions (
            record_id, revision, record_type, record_json, extensions_json,
            became_known_at, became_known_at_basis, changed_by_json, previous_revision
          ) VALUES (?, 2, 'athlete_report', ?, NULL, ?, 'local_write', ?, 1)
        `,
        args: [sourceId, JSON.stringify(athleteReportData()), createdAt, toolActorJson],
      }),
    ).rejects.toThrow(/record_type/);
    await client.close();
  });
});
