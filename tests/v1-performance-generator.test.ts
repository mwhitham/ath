import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  generatePerformanceDatabase,
  PERFORMANCE_FIXTURE_SEED,
} from "../scripts/performance/generate-database.js";
import { openDatabase } from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";

const temporaryDirectories: string[] = [];

afterEach(async () => {
  await Promise.all(
    temporaryDirectories.splice(0).map((path) =>
      rm(path, { recursive: true, force: true }),
    ),
  );
});

describe("v1 performance database generator", () => {
  it("builds the requested deterministic mix without dropping fixture records", async () => {
    const root = await mkdtemp(join(tmpdir(), "ath-v1-performance-"));
    temporaryDirectories.push(root);
    const generated = await generatePerformanceDatabase({
      output_directory: join(root, "10k"),
      record_count: 10_000,
    });

    expect(generated).toMatchObject({
      record_count: 10_000,
      seed: PERFORMANCE_FIXTURE_SEED,
    });
    const database = await openDatabase(generated.database_path, {
      warn: () => undefined,
    });
    try {
      const client = databaseClient(database);
      const counts = await client.execute(`
        SELECT
          (SELECT COUNT(*) FROM records) AS records,
          (SELECT COUNT(*) FROM measurement_samples) AS samples,
          (SELECT COUNT(DISTINCT source_id) FROM measurement_samples) AS sample_sources,
          (SELECT COUNT(*) FROM record_revisions WHERE revision > 1) AS corrections,
          (SELECT COUNT(*) FROM record_deletions) AS deletions
      `);
      expect(Number(counts.rows[0]?.records)).toBe(10_000);
      expect(Number(counts.rows[0]?.samples)).toBe(generated.sample_count + 1);
      expect(Number(counts.rows[0]?.sample_sources)).toBe(3);
      expect(Number(counts.rows[0]?.corrections)).toBeGreaterThan(0);
      expect(Number(counts.rows[0]?.deletions)).toBeGreaterThan(0);

      const types = await client.execute(
        "SELECT COUNT(DISTINCT record_type) AS count FROM records",
      );
      expect(Number(types.rows[0]?.count)).toBe(24);

      const integrity = await client.execute("PRAGMA integrity_check");
      expect(integrity.rows).toEqual([{ integrity_check: "ok" }]);
      const foreignKeys = await client.execute("PRAGMA foreign_key_check");
      expect(foreignKeys.rows).toEqual([]);
    } finally {
      database.close();
    }
  });
});
