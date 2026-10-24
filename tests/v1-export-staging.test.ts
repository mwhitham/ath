import { access, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { exportRecords, openDatabase, releaseLocalDatabase } from "../src/db/index.js";
import type { AthleticDatabase } from "../src/db/index.js";
import {
  cleanupStaleExportStagingDirectories,
  exportStagingDirectoryName,
  EXPORT_STAGING_MARKER,
} from "../src/db/export-staging.js";

const DEMO_FIXTURE = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../examples/demo-athlete/athlete.ath.sqlite",
);

const openDatabases: AthleticDatabase[] = [];

afterEach(async () => {
  for (const database of openDatabases.splice(0)) {
    await releaseLocalDatabase(database);
  }
});

describe("export staging cleanup", () => {
  it("removes marked staging directories when the owning pid is dead", async () => {
    const parent = await mkdtemp(join(tmpdir(), "ath-export-staging-"));
    const deadPid = 99_999_999;
    const staleName = exportStagingDirectoryName(deadPid, Date.now(), "package");
    const stalePath = join(parent, staleName);
    await mkdir(stalePath, { recursive: true });
    await writeFile(
      join(stalePath, EXPORT_STAGING_MARKER),
      JSON.stringify({ pid: deadPid }),
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
      JSON.stringify({ pid: process.pid }),
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

  it("successful export leaves no staging marker in the finished package", async () => {
    const parent = await mkdtemp(join(tmpdir(), "ath-export-finished-"));
    const database = await openDatabase(DEMO_FIXTURE);
    openDatabases.push(database);
    const outputDirectory = join(parent, "package");
    await exportRecords(database, outputDirectory);
    const files = await readdir(outputDirectory);
    expect(files).not.toContain(EXPORT_STAGING_MARKER);
    await rm(parent, { recursive: true, force: true });
  });

  it("cleanup after a simulated post-rename export does not delete the finished package", async () => {
    const parent = await mkdtemp(join(tmpdir(), "ath-post-rename-"));
    const finishedExport = join(parent, "package");
    await mkdir(finishedExport, { recursive: true });
    await writeFile(join(finishedExport, "athlete.ath.json"), "{}", { mode: 0o600 });
    const deadPid = 99_999_998;
    const staleStaging = join(
      parent,
      exportStagingDirectoryName(deadPid, Date.now(), "other-package"),
    );
    await mkdir(staleStaging, { recursive: true });
    await writeFile(
      join(staleStaging, EXPORT_STAGING_MARKER),
      JSON.stringify({ pid: deadPid }),
      { mode: 0o600 },
    );
    await cleanupStaleExportStagingDirectories(parent);
    await access(finishedExport);
    await expect(access(staleStaging)).rejects.toMatchObject({ code: "ENOENT" });
    const finishedFiles = await readdir(finishedExport);
    expect(finishedFiles).not.toContain(EXPORT_STAGING_MARKER);
    await rm(parent, { recursive: true, force: true });
  });

  it("removes a marked staging directory left by a crash before rename", async () => {
    const parent = await mkdtemp(join(tmpdir(), "ath-export-crash-before-"));
    const deadPid = 99_999_996;
    const staging = join(parent, exportStagingDirectoryName(deadPid, Date.now(), "package"));
    await mkdir(staging, { recursive: true });
    await writeFile(
      join(staging, EXPORT_STAGING_MARKER),
      JSON.stringify({ pid: deadPid }),
      { mode: 0o600 },
    );
    await writeFile(join(staging, "athlete.ath.json"), "{}", { mode: 0o600 });
    await cleanupStaleExportStagingDirectories(parent);
    await expect(access(staging)).rejects.toMatchObject({ code: "ENOENT" });
    await rm(parent, { recursive: true, force: true });
  });

  it("post-rename finished export survives the next export cleanup pass", async () => {
    const parent = await mkdtemp(join(tmpdir(), "ath-export-post-rename-"));
    const database = await openDatabase(DEMO_FIXTURE);
    openDatabases.push(database);
    const finishedExport = join(parent, "package");
    await exportRecords(database, finishedExport);
    const finishedFiles = await readdir(finishedExport);
    expect(finishedFiles).not.toContain(EXPORT_STAGING_MARKER);
    expect(finishedFiles).toContain("athlete.ath.json");

    const deadPid = 99_999_997;
    const staleStaging = join(
      parent,
      exportStagingDirectoryName(deadPid, Date.now(), "abandoned"),
    );
    await mkdir(staleStaging, { recursive: true });
    await writeFile(
      join(staleStaging, EXPORT_STAGING_MARKER),
      JSON.stringify({ pid: deadPid }),
      { mode: 0o600 },
    );

    const secondExport = join(parent, "package-two");
    await exportRecords(database, secondExport);

    await access(finishedExport);
    const stillFinished = await readdir(finishedExport);
    expect(stillFinished).toContain("athlete.ath.json");
    expect(stillFinished).not.toContain(EXPORT_STAGING_MARKER);
    await expect(access(staleStaging)).rejects.toMatchObject({ code: "ENOENT" });
    await rm(parent, { recursive: true, force: true });
  });
});
