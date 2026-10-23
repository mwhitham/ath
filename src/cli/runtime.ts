import { createInterface } from "node:readline/promises";
import { findFile } from "../file.js";
import {
  executeConfirmedDatabaseMigration,
  MigrationRequiredError,
  openDatabase,
  releaseLocalDatabase,
  type AthleticDatabase,
  type MigrationPreview,
} from "../db/index.js";

let terminalAnswerForTests: ((question: string) => Promise<string | null>) | undefined;

/** Test hook. Production asks on the real terminal. */
export function useTerminalAnswer(
  answer: ((question: string) => Promise<string | null>) | undefined,
): void {
  terminalAnswerForTests = answer;
}

export function fail(message: string): never {
  console.error(`ath: ${message}`);
  process.exit(1);
}

export function findOrFail(fileArg?: string): string {
  try {
    return findFile(fileArg);
  } catch (error) {
    return fail((error as Error).message);
  }
}

export async function askOnTerminal(question: string): Promise<string | null> {
  if (terminalAnswerForTests) return terminalAnswerForTests(question);
  const { createReadStream } = await import("node:fs");
  let input: NodeJS.ReadableStream;
  try {
    input =
      process.stdin.isTTY && process.stdin.readable
        ? process.stdin
        : createReadStream("/dev/tty");
  } catch {
    return null;
  }
  const rl = createInterface({ input, output: process.stdout });
  try {
    return (await rl.question(question)).trim();
  } catch {
    return null;
  } finally {
    rl.close();
  }
}

export async function confirmWrite(
  opts: { yes?: boolean; dryRun?: boolean; json?: boolean },
  question: string,
): Promise<boolean> {
  if (opts.json || opts.dryRun) {
    return false;
  }
  if (opts.yes) {
    return true;
  }
  if (terminalAnswerForTests === undefined && !process.stdout.isTTY) {
    fail(
      "there is no terminal to ask on, so nothing was written. Pass --yes after reviewing the preview.",
    );
  }
  const answer = await askOnTerminal(`${question} [y] yes  [n] no `);
  return /^y(es)?$/i.test(answer ?? "");
}

export async function readStdin(): Promise<string> {
  if (process.stdin.isTTY) {
    process.stderr.write("Paste the entry, then press Ctrl-D:\n");
  }
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) chunks.push(Buffer.from(chunk));
  return Buffer.concat(chunks).toString("utf8");
}

export async function interactiveMigrationOnOpen(
  path: string,
  preview: MigrationPreview,
): Promise<AthleticDatabase> {
  console.log(JSON.stringify({ file: path, ...preview, write_confirmed: false }, null, 2));
  if (!preview.migration_needed) {
    return await openDatabase(path, {
      onSecurityWarning: (warning) => {
        console.warn(`ath: warning: ${warning}`);
      },
    });
  }
  if (preview.planned_steps.length === 0) {
    return fail(preview.compatibility_effect);
  }
  if (preview.planned_backup_path === null) {
    return fail("SQL schema migration requires a local database file");
  }
  if (!(await confirmWrite({}, "Create a verified backup and apply this migration?"))) {
    return fail("migration was not confirmed; run ath migrate or answer yes when prompted on open");
  }
  const secondConfirmation =
    (await askOnTerminal(`Type the database ID ${preview.database_id} to confirm migration: `)) ??
    undefined;
  if (secondConfirmation !== preview.database_id) {
    return fail(
      "second confirmation did not exactly match the database ID; nothing was migrated",
    );
  }

  const database = await openDatabase(path, {
    allowOutdatedSchema: true,
    deferWal: true,
    onSecurityWarning: (warning) => {
      console.warn(`ath: warning: ${warning}`);
    },
  });
  try {
    const migrationOptions = { backupPath: preview.planned_backup_path };
    const result = await executeConfirmedDatabaseMigration(database, migrationOptions);
    console.log(
      `migrated SQL schema ${preview.source_sql_schema_version} to ${preview.target_sql_schema_version} ` +
        `with verified backup at ${result.backup_path}`,
    );
    return database;
  } catch (error) {
    await releaseLocalDatabase(database);
    return fail((error as Error).message);
  }
}

export async function openReadOnlyOrFail(path: string, json = false): Promise<AthleticDatabase> {
  try {
    return await openDatabase(path, {
      deferWal: true,
      allowOutdatedSchema: true,
      onSecurityWarning: (warning) => {
        if (!json) console.warn(`ath: warning: ${warning}`);
      },
    });
  } catch (error) {
    if (error instanceof MigrationRequiredError) {
      return fail(error.message);
    }
    return fail((error as Error).message);
  }
}

export async function openOrFail(path: string, json = false): Promise<AthleticDatabase> {
  try {
    return await openDatabase(path, {
      onSecurityWarning: (warning) => {
        if (!json) console.warn(`ath: warning: ${warning}`);
      },
    });
  } catch (error) {
    if (error instanceof MigrationRequiredError) {
      if (json || !process.stdin.isTTY || !process.stdout.isTTY) {
        return fail(error.message);
      }
      return await interactiveMigrationOnOpen(path, error.preview);
    }
    return fail((error as Error).message);
  }
}
