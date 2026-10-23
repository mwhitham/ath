import { access, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  cleanupStaleExportStagingDirectories,
  exportStagingDirectoryName,
  EXPORT_STAGING_MARKER,
  writeExportStagingMarker,
} from "../src/db/export-staging.js";

describe("export staging cleanup", () => {
  it("removes marked staging directories when the owning pid is dead", async () => {
    const parent = await mkdtemp(join(tmpdir(), "ath-export-staging-"));
    const deadPid = 99_999_999;
    const staleName = exportStagingDirectoryName(deadPid, Date.now(), "package");
    const stalePath = join(parent, staleName);
    await mkdir(stalePath, { recursive: true });
    await writeFile(
      join(stalePath, EXPORT_STAGING_MARKER),
      JSON.stringify({ pid: deadPid, created_at_ms: Date.now() - 2 * 60 * 60 * 1000 }),
      { mode: 0o600 },
    );
    await writeFile(join(stalePath, "athlete.ath.json"), "{}", { mode: 0o600 });

    await cleanupStaleExportStagingDirectories(parent);

    await expect(access(stalePath)).rejects.toMatchObject({ code: "ENOENT" });
    await rm(parent, { recursive: true, force: true });
  });

  it("keeps a marked staging directory when the owning pid is still alive", async () => {
    const parent = await mkdtemp(join(tmpdir(), "ath-export-staging-live-"));
    const liveName = exportStagingDirectoryName(process.pid, Date.now() - 2 * 60 * 60 * 1000, "package");
    const livePath = join(parent, liveName);
    await mkdir(livePath, { recursive: true });
    await writeFile(
      join(livePath, EXPORT_STAGING_MARKER),
      JSON.stringify({ pid: process.pid, created_at_ms: Date.now() - 2 * 60 * 60 * 1000 }),
      { mode: 0o600 },
    );

    await cleanupStaleExportStagingDirectories(parent);

    await access(livePath);
    await rm(parent, { recursive: true, force: true });
  });

  it("does not delete unmarked directories that match the export name pattern", async () => {
    const parent = await mkdtemp(join(tmpdir(), "ath-export-staging-unmarked-"));
    const userCopy = join(parent, ".ath-export-424242-old-copy");
    await mkdir(userCopy, { recursive: true });
    await writeFile(join(userCopy, "notes.txt"), "keep me", { mode: 0o600 });

    await cleanupStaleExportStagingDirectories(parent);

    await access(userCopy);
    await rm(parent, { recursive: true, force: true });
  });
});
