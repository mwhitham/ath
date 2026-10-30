import { rm } from "node:fs/promises";
import { afterEach, describe, expect, it } from "vitest";
import { openDatabase, type AthleticDatabase } from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";
import { rebuildTypedRowFromCurrentRevision } from "../src/db/records.js";
import { createUuidV7 } from "../src/db/uuid.js";
import { materializeNpm100ProviderDatabase } from "./helpers/npm100-database.js";

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];

afterEach(async () => {
  for (const database of openDatabases.splice(0)) {
    database.close();
  }
  for (const directory of directories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

describe("a version 1.0.0 database has no route table", () => {
  it("refuses a route write and tells you to migrate", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const database = await openDatabase(databasePath, {
      allowOutdatedSchema: true,
      deferWal: true,
    });
    openDatabases.push(database);
    const client = databaseClient(database);
    const sources = await client.execute("SELECT record_id FROM sources LIMIT 1");
    const sourceId = String(sources.rows[0]?.record_id);
    expect(sourceId).toMatch(/^[0-9a-f-]{36}$/);
    const recordId = createUuidV7();
    const at = "2026-08-09T15:00:00.000Z";
    const data = {
      source_id: sourceId,
      received_at: "2026-08-09T16:00:00.000Z",
      received_at_basis: "import_time",
      start_at: at,
      end_at: "2026-08-09T15:10:00.000Z",
      input_start_timestamp: {
        timestamp_text: at,
        utc_offset_text: "Z",
        utc_offset_meaning: "documented_recording_offset",
        timestamp_basis: "source_instant",
      },
      input_end_timestamp: {
        timestamp_text: "2026-08-09T15:10:00.000Z",
        utc_offset_text: "Z",
        utc_offset_meaning: "documented_recording_offset",
        timestamp_basis: "source_instant",
      },
      source_reported_summaries: [],
      route: [{ recorded_at: at, latitude_deg: 40, longitude_deg: -105 }],
    };
    const transaction = await client.transaction("write");
    try {
      await transaction.execute("PRAGMA defer_foreign_keys = ON");
      await transaction.execute({
        sql: `
          INSERT INTO record_revisions (
            record_id, revision, record_type, record_json, extensions_json,
            became_known_at, became_known_at_basis, changed_by_json, previous_revision
          ) VALUES (?, 1, 'workout_session', ?, NULL, ?, 'provider_import', ?, NULL)
        `,
        args: [
          recordId,
          JSON.stringify(data),
          at,
          JSON.stringify({ author_type: "tool", tool: "ath", tool_version: "1.0.0" }),
        ],
      });
      await transaction.execute({
        sql: `
          INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted)
          VALUES (?, 'workout_session', 1, ?, 0)
        `,
        args: [recordId, at],
      });
      await expect(rebuildTypedRowFromCurrentRevision(transaction, recordId)).rejects.toThrow(
        /workout_route_points is missing/,
      );
    } finally {
      if (!transaction.closed) {
        await transaction.rollback();
      }
      transaction.close();
    }
  });
});
