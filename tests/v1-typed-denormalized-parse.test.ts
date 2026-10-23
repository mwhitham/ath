import { copyFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { checkDatabase, openDatabase } from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";
import type { AthleticDatabase } from "../src/db/index.js";
import { ath } from "./helpers/cli-exec.js";
import { prepareMigratedNpm100 } from "./helpers/npm100-prepared.js";

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

describe("typed denormalized parse failures", () => {
  it("check reports a schema issue instead of throwing when record_json is invalid", async () => {
    const prepared = await prepareMigratedNpm100();
    directories.push(prepared.directory);
    const database = await openDatabase(prepared.databasePath);
    openDatabases.push(database);
    const row = await databaseClient(database).execute(`
      SELECT records.record_id
      FROM records
      JOIN point_measurements ON point_measurements.record_id = records.record_id
      WHERE records.record_type = 'point_measurement'
      LIMIT 1
    `);
    const recordId = String(row.rows[0]!.record_id);
    await databaseClient(database).execute({
      sql: `
        UPDATE record_revisions SET record_json = '{}'
        WHERE record_id = ? AND revision = (
          SELECT current_revision FROM records WHERE record_id = ?
        )
      `,
      args: [recordId, recordId],
    });
    database.close();
    openDatabases.pop();

    const reopened = await openDatabase(prepared.databasePath);
    openDatabases.push(reopened);
    const check = await checkDatabase(reopened);
    expect(check.valid).toBe(false);
    expect(
      check.issues.some(
        (issue) => issue.check === "typed_denormalized" && issue.record_id === recordId,
      ),
    ).toBe(true);
  });

  it("repair --from-revision rebuilds typed rows from record_revisions without parsing damaged data_json", async () => {
    const prepared = await prepareMigratedNpm100();
    const directory = await mkdtemp(join(tmpdir(), "ath-denorm-parse-repair-"));
    directories.push(directory);
    const databasePath = join(directory, "athlete.ath.sqlite");
    await copyFile(prepared.databasePath, databasePath);

    const database = await openDatabase(databasePath);
    openDatabases.push(database);
    const row = await databaseClient(database).execute(`
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
    await databaseClient(database).execute({
      sql: "UPDATE point_measurements SET data_json = ?, standard_value = 999 WHERE record_id = ?",
      args: [divergent, recordId],
    });
    const idRow = await databaseClient(database).execute(
      "SELECT database_id FROM database_metadata WHERE singleton_id = 1",
    );
    const databaseId = String(idRow.rows[0]?.database_id);
    database.close();
    openDatabases.pop();

    const repair = ath(
      ["repair", "-y", "--from-revision", "--confirm-revision-align", databaseId],
      directory,
    );
    expect(repair.code, repair.stdout).toBe(0);
    const check = ath(["check"], directory);
    expect(check.code, check.stdout).toBe(0);
  }, 180_000);
});
