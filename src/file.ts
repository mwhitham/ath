/**
 * Find the live SQLite athlete database.
 *
 * The JSON helpers remain private compatibility code while commands are moved to
 * the v1 database. New commands must not treat a JSON export as the live record.
 */
import { readdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { AthleticStandardFile, type AthleticStandardFileT } from "./schema.js";
import { validateAthleticStandardFile } from "./validate.js";

export const FILE_SUFFIX = ".ath.sqlite";
export const DEFAULT_FILENAME = `athlete${FILE_SUFFIX}`;
export const PORTABLE_FILE_SUFFIX = ".ath.json";

/**
 * Resolve which live database to operate on.
 *
 * Discovery intentionally uses only athlete.ath.sqlite. Another database needs an
 * explicit --file so a directory can safely hold exports and alternate records.
 */
export function findFile(explicitPath?: string, cwd = process.cwd()): string {
  if (explicitPath) {
    const p = resolve(cwd, explicitPath);
    if (!existsSync(p)) throw new Error(`file not found: ${p}`);
    if (p.endsWith(PORTABLE_FILE_SUFFIX)) {
      throw new Error(
        `Athletic Standard prerelease JSON files are not supported by v1. ` +
          `Keep ${p} unchanged and run \`ath init\` to create a v1 SQLite database.`,
      );
    }
    if (!p.endsWith(FILE_SUFFIX)) {
      throw new Error(
        `the live athlete record must be a ${FILE_SUFFIX} SQLite file, not ${p}`,
      );
    }
    return p;
  }

  const defaultPath = resolve(cwd, DEFAULT_FILENAME);
  if (existsSync(defaultPath)) {
    return defaultPath;
  }

  const portableFiles = readdirSync(cwd).filter((file) =>
    file.endsWith(PORTABLE_FILE_SUFFIX),
  );
  if (portableFiles.length === 1) {
    throw new Error(
      `found ${portableFiles[0]}, but Athletic Standard prerelease JSON files are not ` +
        `supported by v1. Keep it unchanged and run \`ath init\` to create ${DEFAULT_FILENAME}.`,
    );
  }

  throw new Error(
    `no ${DEFAULT_FILENAME} in ${cwd} — run \`ath init\` to create one, or pass --file`,
  );
}

/** Load without validating (callers that validate should use `check`-style flows). */
export function loadFileRaw(path: string): unknown {
  if (path.endsWith(FILE_SUFFIX)) {
    throw new Error(
      `this command has not been moved to the v1 SQLite database yet. Nothing was written.`,
    );
  }
  return JSON.parse(readFileSync(path, "utf8"));
}

/**
 * Load and parse through the schema; throws with a readable message on failure.
 *
 * The message comes from the same explanation `ath check` gives, because a bare Zod
 * union failure says only "Invalid input" — which tells the reader nothing about a
 * file they are expected to be able to edit by hand.
 */
export function loadFile(path: string): AthleticStandardFileT {
  const raw = loadFileRaw(path);
  const parsed = AthleticStandardFile.safeParse(raw);
  if (!parsed.success) {
    const first = validateAthleticStandardFile(raw).issues.find((i) => i.severity === "error");
    throw new Error(
      `${path} is not a valid Athletic Standard file.\n` +
        `  ${first?.path ?? "root"}: ${first?.message ?? "unreadable"}\n` +
        `Run \`ath check\` for the full list.`,
    );
  }
  return parsed.data;
}

/** Stable field order + trailing newline, so files diff cleanly under git. */
export function saveFile(path: string, file: AthleticStandardFileT): void {
  writeFileSync(path, JSON.stringify(file, null, 2) + "\n");
}
