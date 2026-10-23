import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { createClient } from "@libsql/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as clientModule from "../src/db/client.js";
import { createDatabase, openDatabase, type AthleticDatabase } from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";
import { withWriteTransaction } from "../src/db/transaction.js";
import { materializeNpm100ProviderDatabase } from "./helpers/npm100-database.js";
import { ath } from "./helpers/cli-exec.js";

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];

afterEach(async () => {
  vi.restoreAllMocks();
  for (const database of openDatabases.splice(0)) {
    database.close();
  }
  for (const directory of directories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

describe("journal revert after failed writes", () => {
  it("preserves the original write error when DELETE journal revert fails", async () => {
    const directory = await mkdtemp(join(tmpdir(), "ath-journal-revert-"));
    directories.push(directory);
    const databasePath = join(directory, "athlete.ath.sqlite");
    const database = await createDatabase(databasePath);
    openDatabases.push(database);
    await databaseClient(database).execute("PRAGMA journal_mode = DELETE");

    vi.spyOn(clientModule, "tryRevertDeleteJournalModeAfterFailedWal").mockRejectedValueOnce(
      new Error("revert exploded"),
    );

    await expect(
      withWriteTransaction(database, async (transaction) => {
        await transaction.execute("SELECT 1");
        throw new Error("write exploded");
      }),
    ).rejects.toThrow("write exploded");
  });

  it("still reports the migration backup path when journal revert fails", async () => {
    const { directory, databasePath } = materializeNpm100ProviderDatabase();
    directories.push(directory);
    const client = createClient({ url: pathToFileURL(databasePath).href, intMode: "number" });
    await client.execute("PRAGMA journal_mode = DELETE");
    const row = await client.execute(`
      SELECT records.record_id, record_revisions.record_json
      FROM records
      JOIN record_revisions
        ON record_revisions.record_id = records.record_id
       AND record_revisions.revision = records.current_revision
      JOIN point_measurements ON point_measurements.record_id = records.record_id
      WHERE records.record_type = 'point_measurement'
        AND point_measurements.quantity = 'hrv_rmssd'
      LIMIT 1
    `);
    const recordId = String(row.rows[0]!.record_id);
    const divergent = String(row.rows[0]!.record_json).replace("68.4", "999");
    await client.execute({
      sql: "UPDATE point_measurements SET data_json = ?, standard_value = 999 WHERE record_id = ?",
      args: [divergent, recordId],
    });
    const idRow = await client.execute(
      "SELECT database_id FROM database_metadata WHERE singleton_id = 1",
    );
    const databaseId = String(idRow.rows[0]?.database_id);
    await client.close();

    vi.spyOn(clientModule, "tryRevertDeleteJournalModeAfterFailedWal").mockRejectedValueOnce(
      new Error("revert exploded"),
    );

    const migrate = ath(["migrate", "-y", "--confirm-migration", databaseId], directory);
    expect(migrate.code).not.toBe(0);
    expect(migrate.stdout).toMatch(/migration-backup/i);
    expect(migrate.stdout).toMatch(/not key-order only/i);
  }, 180_000);
});
