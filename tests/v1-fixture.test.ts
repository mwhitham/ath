import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  listRecords,
  openDatabase,
  querySeries,
  readDatabaseMetadata,
  readRecordRevisions,
} from "../src/db/index.js";
import { RecordType } from "../src/v1/schema.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const fixtureDirectory = join(root, "examples", "demo-athlete");
const databasePath = join(fixtureDirectory, "athlete.ath.sqlite");

describe("deterministic v1 SQLite fixture", () => {
  it("contains every record type, a correction, a deletion, and dense data", async () => {
    const database = await openDatabase(databasePath, {
      onSecurityWarning: () => undefined,
    });
    try {
      const metadata = await readDatabaseMetadata(database);
      expect(metadata).toMatchObject({
        athleticstandard_version: "2.1.0",
        database_id: "01890f3e-e7d3-7cc0-88c8-000000000001",
        created_at: "2026-08-09T12:00:00.000Z",
      });

      const records = await listRecords(database, {
        include_deleted: true,
        limit: 1_000,
      });
      expect(records).toHaveLength(25);
      expect([...new Set(records.map((record) => record.record_type))].sort()).toEqual(
        [...RecordType.options].sort(),
      );
      expect(records.some((record) => record.is_deleted)).toBe(true);
      const attachment = records.find((record) => record.record_type === "attachment");
      expect(attachment?.data).toMatchObject({
        file_name: "meal-photo.png",
        sha256: "d3fe8e599fb1deea9821063c744434802875c26cceeb9f2d55e8566b15a012f4",
      });

      const pointRevisions = await readRecordRevisions(
        database,
        "01890f3e-e7d3-7cc0-88c8-000000000007",
      );
      expect(pointRevisions.map((revision) => revision.revision)).toEqual([1, 2]);

      const samples = await querySeries(database, {
        quantity: "heart_rate",
      });
      expect(samples).toHaveLength(1);
    } finally {
      database.close();
    }
  });

  it("keeps the referenced attachment bytes and hash together", async () => {
    expect(
      await readdir(join(fixtureDirectory, "attachments")),
    ).toEqual(["01890f3e-e7d3-7cc0-88c8-00000000000e.png"]);
    const attachment = await readFile(
      join(
        fixtureDirectory,
        "attachments",
        "01890f3e-e7d3-7cc0-88c8-00000000000e.png",
      ),
    );
    expect(createHash("sha256").update(attachment).digest("hex")).toBe(
      "d3fe8e599fb1deea9821063c744434802875c26cceeb9f2d55e8566b15a012f4",
    );
  });
});
