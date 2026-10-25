/**
 * Fails when the published SQL contract does not match the live migration source
 * or a database created by createDatabase().
 */
import { readFileSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { createClient } from "@libsql/client";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  buildSqlContractDocument,
  MIGRATION_002_FILE_HEADER,
  MIGRATION_003_FILE_HEADER,
  publishedMigrationSql,
  readDatabaseFileHeader,
} from "../src/db/sql-contract.js";
import { PROVIDER_ENCODING_REPAIR_SQL } from "../src/db/migrations/002-provider-encoding-repair.js";
import { VERSION_2_UPGRADE_SQL } from "../src/db/migrations/003-version-2.js";
import { SQLITE_APPLICATION_ID } from "../src/db/client.js";
import {
  createDatabase,
  SQLITE_APPLICATION_MARKER,
  SQL_SCHEMA_VERSION,
  type AthleticDatabase,
} from "../src/db/index.js";
import { ATHLETIC_STANDARD_VERSION } from "../src/v1/schema.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function normalizeSql(sql: string | null | undefined): string {
  if (sql === null || sql === undefined) {
    return "";
  }
  return sql.replace(/\s+/g, " ").trim();
}

interface SchemaObject {
  type: string;
  name: string;
  sql: string;
}

async function listSchemaObjects(
  execute: (sql: string) => Promise<{ rows: Array<Record<string, unknown>> }>,
): Promise<SchemaObject[]> {
  const result = await execute(`
    SELECT type, name, sql
    FROM sqlite_master
    WHERE name NOT LIKE 'sqlite_%'
    ORDER BY type, name
  `);
  return result.rows.map((row) => ({
    type: String(row.type),
    name: String(row.name),
    sql: normalizeSql(row.sql as string | null),
  }));
}

function fingerprint(objects: SchemaObject[]): string {
  return JSON.stringify(
    objects.map((object) => [object.type, object.name, object.sql]),
  );
}

interface SchemaSnapshot {
  objects: SchemaObject[];
  header: Awaited<ReturnType<typeof readDatabaseFileHeader>>;
}

async function schemaFromCreateDatabase(): Promise<SchemaSnapshot> {
  const directory = await mkdtemp(join(tmpdir(), "ath-schema-drift-"));
  const path = join(directory, "athlete.ath.sqlite");
  const databaseId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f01";
  const createdAt = "2026-08-09T12:00:00.000Z";
  let database: AthleticDatabase | undefined;
  try {
    database = await createDatabase(path, { databaseId, createdAt });
    const client = createClient({ url: pathToFileURL(path).href });
    await client.execute("PRAGMA foreign_keys = ON");
    const objects = await listSchemaObjects((sql) => client.execute(sql));
    const header = await readDatabaseFileHeader((sql) => client.execute(sql));
    await client.close();
    return { objects, header };
  } finally {
    database?.close();
    await rm(directory, { recursive: true, force: true });
  }
}

async function schemaFromPublishedSqlFile(): Promise<SchemaSnapshot> {
  const directory = await mkdtemp(join(tmpdir(), "ath-schema-drift-sql-"));
  const path = join(directory, "athlete.ath.sqlite");
  const migrationFile = join(root, "spec", "sql", "migrations", "001-initial.sql");
  const apply = spawnSync(
    "sqlite3",
    [path],
    {
      input: readFileSync(migrationFile, "utf8"),
      encoding: "utf8",
    },
  );
  if (apply.status !== 0) {
    throw new Error(`sqlite3 failed to apply published migration: ${apply.stderr}`);
  }

  const databaseId = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f02";
  const createdAt = "2026-08-09T12:00:00.000Z";
  const seed = spawnSync(
    "sqlite3",
    [path],
    {
      input: `
        PRAGMA foreign_keys = ON;
        INSERT INTO database_metadata (
          singleton_id,
          application_marker,
          sql_schema_version,
          athleticstandard_version,
          database_id,
          created_at
        ) VALUES (
          1,
          '${SQLITE_APPLICATION_MARKER}',
          ${SQL_SCHEMA_VERSION},
          '${ATHLETIC_STANDARD_VERSION}',
          '${databaseId}',
          '${createdAt}'
        );
        INSERT INTO database_state (singleton_id, current_revision) VALUES (1, 0);
        PRAGMA application_id = ${SQLITE_APPLICATION_ID};
        PRAGMA user_version = ${SQL_SCHEMA_VERSION};
      `,
      encoding: "utf8",
    },
  );
  if (seed.status !== 0) {
    throw new Error(`sqlite3 failed to seed metadata: ${seed.stderr}`);
  }

  const client = createClient({ url: pathToFileURL(path).href });
  await client.execute("PRAGMA foreign_keys = ON");
  const objects = await listSchemaObjects((sql) => client.execute(sql));
  const header = await readDatabaseFileHeader((sql) => client.execute(sql));
  await client.close();
  await rm(directory, { recursive: true, force: true });
  return { objects, header };
}

function expectedMigrationFileContents(): string {
  const header = `-- Athletic Standard SQL schema migration 001 (sql_schema_version 1).
-- Generated by pnpm generate:sql-contract. Do not edit by hand.
--
-- After this file runs, the migration runner must insert database_metadata and
-- database_state rows. See spec/sql/README.md and spec/sql/contract.json.

`;
  return `${header}${publishedMigrationSql()}\n`;
}

function expectedMigration002FileContents(): string {
  return `${MIGRATION_002_FILE_HEADER}${PROVIDER_ENCODING_REPAIR_SQL}\n`;
}

function expectedMigration003FileContents(): string {
  return `${MIGRATION_003_FILE_HEADER}${VERSION_2_UPGRADE_SQL}\n`;
}

function checkGeneratedFilesFresh(): void {
  const migrationPath = join(root, "spec", "sql", "migrations", "001-initial.sql");
  const onDisk = readFileSync(migrationPath, "utf8");
  const expected = expectedMigrationFileContents();
  if (onDisk !== expected) {
    throw new Error(
      "spec/sql/migrations/001-initial.sql is stale; run pnpm generate:sql-contract",
    );
  }

  const contractPath = join(root, "spec", "sql", "contract.json");
  const contractOnDisk = readFileSync(contractPath, "utf8");
  const contractExpected = JSON.stringify(buildSqlContractDocument(), null, 2) + "\n";
  if (contractOnDisk !== contractExpected) {
    throw new Error("spec/sql/contract.json is stale; run pnpm generate:sql-contract");
  }

  const migration002Path = join(root, "spec", "sql", "migrations", "002-provider-encoding-repair.sql");
  const migration002OnDisk = readFileSync(migration002Path, "utf8");
  const migration002Expected = expectedMigration002FileContents();
  if (migration002OnDisk !== migration002Expected) {
    throw new Error(
      "spec/sql/migrations/002-provider-encoding-repair.sql is stale; run pnpm generate:sql-contract",
    );
  }

  const migration003Path = join(root, "spec", "sql", "migrations", "003-version-2.sql");
  const migration003OnDisk = readFileSync(migration003Path, "utf8");
  if (migration003OnDisk !== expectedMigration003FileContents()) {
    throw new Error(
      "spec/sql/migrations/003-version-2.sql is stale; run pnpm generate:sql-contract",
    );
  }
}

export async function checkSchemaDrift(): Promise<void> {
  checkGeneratedFilesFresh();

  const fromNode = await schemaFromCreateDatabase();
  const fromSqlFile = await schemaFromPublishedSqlFile();

  if (fingerprint(fromNode.objects) !== fingerprint(fromSqlFile.objects)) {
    throw new Error(
      "Published SQL migration does not match createDatabase() schema; regenerate and review drift",
    );
  }

  if (JSON.stringify(fromNode.header) !== JSON.stringify(fromSqlFile.header)) {
    throw new Error(
      "Published SQL seed does not match createDatabase() file header pragmas (application_id, user_version)",
    );
  }

  if (fromNode.header.application_id !== SQLITE_APPLICATION_ID) {
    throw new Error(
      `createDatabase() application_id is ${fromNode.header.application_id}, expected ${SQLITE_APPLICATION_ID}`,
    );
  }
  if (fromNode.header.user_version !== SQL_SCHEMA_VERSION) {
    throw new Error(
      `createDatabase() user_version is ${fromNode.header.user_version}, expected ${SQL_SCHEMA_VERSION}`,
    );
  }
}

async function main(): Promise<void> {
  await checkSchemaDrift();
  console.log("SQL contract drift check passed");
}

const isMain =
  process.argv[1] !== undefined &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isMain) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  });
}
