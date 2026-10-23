/**
 * Find the live SQLite athlete database.
 *
 * New commands must not treat a JSON export as the live record.
 */
import { readdirSync, existsSync } from "node:fs";
import { resolve } from "node:path";

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

