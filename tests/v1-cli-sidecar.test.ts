import { chmod, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createClient } from "@libsql/client";
import { pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";
import { afterEach, describe, expect, it } from "vitest";
import { createDatabase } from "../src/db/index.js";
import type { AthleticDatabase } from "../src/db/index.js";
import { materializeNpm100ProviderDatabase } from "./helpers/npm100-database.js";
import { useDeleteJournalMode } from "./helpers/raw-database-hash.js";

const openDatabases: AthleticDatabase[] = [];
const directories: string[] = [];

async function sidecarPaths(databasePath: string): Promise<string[]> {
  const paths: string[] = [];
  for (const suffix of ["-wal", "-shm"]) {
    try {
      await import("node:fs/promises").then(({ access }) => access(`${databasePath}${suffix}`));
      paths.push(`${databasePath}${suffix}`);
    } catch {
      // absent
    }
  }
  return paths;
}

afterEach(async () => {
  for (const database of openDatabases.splice(0)) {
    database.close();
  }
  for (const directory of directories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

describe("CLI refusal paths and WAL sidecars", () => {
  it.runIf(process.platform !== "win32")(
    "does not leave WAL sidecars after refusing schema-1 check on a migrated-metadata mismatch",
    async () => {
      const { directory, databasePath } = materializeNpm100ProviderDatabase();
      directories.push(directory);
      await useDeleteJournalMode(databasePath);
      const client = createClient({ url: pathToFileURL(databasePath).href, intMode: "number" });
      await client.execute("UPDATE database_metadata SET sql_schema_version = 2");
      await client.close();

      const cli = join(import.meta.dirname, "../src/cli.ts");
      const tsx = join(import.meta.dirname, "../node_modules/.bin/tsx");
      try {
        execFileSync(tsx, [cli, "check"], { cwd: directory, encoding: "utf8", stdio: "pipe" });
      } catch {
        // expected refusal
      }
      expect(await sidecarPaths(databasePath)).toEqual([]);
    },
  );

  it.runIf(process.platform !== "win32")(
    "does not leave WAL sidecars after refusing a read-only database check",
    async () => {
      const directory = await mkdtemp(join(tmpdir(), "ath-cli-sidecar-"));
      directories.push(directory);
      const databasePath = join(directory, "athlete.ath.sqlite");
      const database = await createDatabase(databasePath);
      openDatabases.push(database);
      const { databaseClient } = await import("../src/db/client.js");
      await databaseClient(database).execute("PRAGMA journal_mode = DELETE");
      const { releaseLocalDatabase } = await import("../src/db/index.js");
      await releaseLocalDatabase(database);
      openDatabases.pop();
      await chmod(databasePath, 0o444);
      const cli = join(import.meta.dirname, "../src/cli.ts");
      const tsx = join(import.meta.dirname, "../node_modules/.bin/tsx");
      try {
        execFileSync(tsx, [cli, "check"], { cwd: directory, encoding: "utf8", stdio: "pipe" });
      } catch {
        // expected refusal
      }
      expect(await sidecarPaths(databasePath)).toEqual([]);
      await chmod(databasePath, 0o600);
    },
  );
});
