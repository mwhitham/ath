import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  truncateSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  createDatabase,
  exportRecords,
  importRecords,
  loadAthleticStandardPackage,
  openDatabase,
  previewPackageImport,
  writeRecord,
  type AthleticDatabase,
} from "../src/db/index.js";

const fixture = resolve("examples/demo-athlete/athlete.ath.sqlite");
const databases: AthleticDatabase[] = [];
const directories: string[] = [];

function temporaryDirectory(prefix: string): string {
  const directory = mkdtempSync(join(tmpdir(), prefix));
  directories.push(directory);
  return directory;
}

afterEach(() => {
  for (const database of databases.splice(0)) {
    database.close();
  }
  for (const directory of directories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

describe("v1 package input safety", () => {
  it("rejects malformed, deeply nested, oversized, and linked package documents", async () => {
    const directory = temporaryDirectory("ath-package-safety-");
    const malformed = join(directory, "malformed.json");
    writeFileSync(malformed, "{", { mode: 0o600 });
    await expect(loadAthleticStandardPackage(malformed)).rejects.toThrow(
      "package JSON is invalid",
    );

    const nested = join(directory, "nested.json");
    writeFileSync(nested, `${'{"child":'.repeat(129)}null${"}".repeat(129)}`, {
      mode: 0o600,
    });
    await expect(loadAthleticStandardPackage(nested)).rejects.toThrow(
      "package JSON exceeds nesting limit 128",
    );

    const oversized = join(directory, "oversized.json");
    writeFileSync(oversized, "{}", { mode: 0o600 });
    truncateSync(oversized, 512 * 1024 * 1024 + 1);
    await expect(loadAthleticStandardPackage(oversized)).rejects.toThrow(
      "package JSON exceeds",
    );

    const linked = join(directory, "linked.json");
    symlinkSync(malformed, linked);
    await expect(loadAthleticStandardPackage(linked)).rejects.toThrow(
      "symbolic link",
    );
  });

  it("rejects a series file outside the package directory", async () => {
    const directory = temporaryDirectory("ath-package-traversal-");
    const source = await openDatabase(fixture);
    databases.push(source);
    const packageDirectory = join(directory, "package");
    await exportRecords(source, packageDirectory);
    rmSync(join(packageDirectory, "export-manifest.json"));

    const documentPath = join(packageDirectory, "athlete.ath.json");
    const document = JSON.parse(readFileSync(documentPath, "utf8")) as {
      measurement_series_files: Array<{ file: string }>;
    };
    document.measurement_series_files[0]!.file = "../outside.json";
    writeFileSync(documentPath, `${JSON.stringify(document)}\n`, { mode: 0o600 });

    await expect(
      loadAthleticStandardPackage(packageDirectory),
    ).rejects.toThrow("package file escapes its directory");
  });

  it("removes prepared attachment files when the destination changes after preview", async () => {
    const directory = temporaryDirectory("ath-package-cleanup-");
    const source = await openDatabase(fixture);
    databases.push(source);
    const packageDirectory = join(directory, "package");
    await exportRecords(source, packageDirectory);
    const loaded = await loadAthleticStandardPackage(packageDirectory);

    const destinationPath = join(directory, "athlete.ath.sqlite");
    const destination = await createDatabase(destinationPath, {
      createdAt: "2026-10-13T12:00:00.000Z",
    });
    databases.push(destination);
    const preview = await previewPackageImport(destination, loaded);
    expect(preview.restore).toBe(true);

    await writeRecord(destination, {
      record_type: "athlete_profile",
      record_id: "01890f3e-e7d3-7cc0-88c8-0000000000aa",
      created_at: "2026-10-13T12:01:00.000Z",
      became_known_at: "2026-10-13T12:01:00.000Z",
      changed_by: {
        author_type: "tool",
        tool: "ath",
        tool_version: "1.0.0",
      },
      data: { name: "Changed after preview" },
    });

    await expect(importRecords(destination, preview)).rejects.toThrow(
      "destination database changed after preview",
    );
    const attachmentDirectory = join(directory, "attachments");
    mkdirSync(attachmentDirectory, { recursive: true });
    expect(readdirSync(attachmentDirectory)).toEqual([]);
  });
});
