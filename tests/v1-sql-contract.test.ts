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
  createDatabase,
  migrateDatabase,
  readDatabaseMetadata,
  SQL_SCHEMA_VERSION,
  writeRecord,
} from "../src/db/index.js";
import { readUserVersion } from "../src/db/client.js";
import type { AthleticDatabase } from "../src/db/index.js";

const databaseId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f10";
const createdAt = "2026-08-09T12:00:00.000Z";
const sourceId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02";

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
