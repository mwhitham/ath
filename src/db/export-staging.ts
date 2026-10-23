import { lstat, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

/** Marker file written into every export staging directory this tool creates. */
export const EXPORT_STAGING_MARKER = ".athletic-standard-export-staging";

/** Abandoned staging directories older than this may be removed. */
export const EXPORT_STAGING_STALE_MS = 60 * 60 * 1000;

export interface ExportStagingMarker {
  pid: number;
  created_at_ms: number;
}

export function exportStagingDirectoryName(pid: number, timestampMs: number, finalName: string): string {
  return `.ath-export-${pid}-${timestampMs}-${finalName}`;
}

export async function writeExportStagingMarker(directory: string): Promise<void> {
  const marker: ExportStagingMarker = { pid: process.pid, created_at_ms: Date.now() };
  await writeFile(join(directory, EXPORT_STAGING_MARKER), JSON.stringify(marker), { mode: 0o600 });
}

function isProcessAlive(pid: number): boolean {
  if (pid <= 0) {
    return false;
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function readExportStagingMarker(
  directory: string,
): Promise<ExportStagingMarker | undefined> {
  try {
    const raw = await readFile(join(directory, EXPORT_STAGING_MARKER), "utf8");
    const parsed = JSON.parse(raw) as ExportStagingMarker;
    if (
      typeof parsed.pid === "number" &&
      typeof parsed.created_at_ms === "number" &&
      Number.isFinite(parsed.pid) &&
      Number.isFinite(parsed.created_at_ms)
    ) {
      return parsed;
    }
  } catch {
    return undefined;
  }
  return undefined;
}

export async function cleanupStaleExportStagingDirectories(
  parentDirectory: string,
  exceptDirectory?: string,
): Promise<void> {
  for (const entry of await readdir(parentDirectory)) {
    if (!entry.startsWith(".ath-export-")) {
      continue;
    }
    const stalePath = join(parentDirectory, entry);
    if (exceptDirectory !== undefined && stalePath === exceptDirectory) {
      continue;
    }
    try {
      const stat = await lstat(stalePath);
      if (!stat.isDirectory()) {
        continue;
      }
      const marker = await readExportStagingMarker(stalePath);
      if (marker === undefined) {
        continue;
      }
      if (isProcessAlive(marker.pid)) {
        continue;
      }
      await rm(stalePath, { recursive: true, force: true });
    } catch {
      // Best-effort cleanup of abandoned staging directories.
    }
  }
}
