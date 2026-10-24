import { createClient } from "@libsql/client";
import { pathToFileURL } from "node:url";
import { ath } from "./cli-exec.js";
import { materializeNpm100ProviderDatabase } from "./npm100-database.js";

export async function prepareMigratedNpm100(): Promise<{
  directory: string;
  databasePath: string;
  databaseId: string;
}> {
  const { directory, databasePath } = materializeNpm100ProviderDatabase();
  const repair = ath(["repair", "-y"], directory);
  if (repair.code !== 0) {
    throw new Error(`repair failed: ${repair.stdout}`);
  }
  const client = createClient({ url: pathToFileURL(databasePath).href });
  const idRow = await client.execute(
    "SELECT database_id FROM database_metadata WHERE singleton_id = 1",
  );
  const databaseId = String(idRow.rows[0]?.database_id);
  await client.close();
  const migrate = ath(["migrate", "-y", "--confirm-migration", databaseId], directory);
  if (migrate.code !== 0) {
    throw new Error(`migrate failed: ${migrate.stdout}`);
  }
  return { directory, databasePath, databaseId };
}
