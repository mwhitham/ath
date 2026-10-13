import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  readdirSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  AthleticStandardPackage,
  ExportManifest,
  MeasurementSeriesFile,
} from "../src/v1/schema.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const FIXTURE = resolve(here, "../examples/demo-athlete/athlete.ath.sqlite");

function ath(args: string[], cwd: string): { stdout: string; code: number } {
  try {
    return {
      stdout: execFileSync(TSX, [CLI, ...args], {
        cwd,
        encoding: "utf8",
        stdio: "pipe",
      }),
      code: 0,
    };
  } catch (error) {
    const failure = error as { status: number; stdout?: string; stderr?: string };
    return {
      stdout: `${failure.stdout ?? ""}${failure.stderr ?? ""}`,
      code: failure.status,
    };
  }
}

function filesBelow(directory: string, root = directory): Array<[string, Buffer]> {
  const files: Array<[string, Buffer]> = [];
  for (const name of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, name.name);
    if (name.isDirectory()) {
      files.push(...filesBelow(path, root));
    } else {
      files.push([relative(root, path), readFileSync(path)]);
    }
  }
  return files.sort(([left], [right]) => left.localeCompare(right));
}

describe("v1 deterministic package export", () => {
  it("writes schema-valid package, series, attachment, and manifest files", () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-v1-export-"));
    const output = join(directory, "package");
    const result = ath(["export", output, "--file", FIXTURE, "--json"], directory);
    expect(result.code).toBe(0);

    const packageDocument = AthleticStandardPackage.parse(
      JSON.parse(readFileSync(join(output, "athlete.ath.json"), "utf8")),
    );
    expect(packageDocument.database_id).toBe(
      "01890f3e-e7d3-7cc0-88c8-000000000001",
    );
    expect(packageDocument.records).toHaveLength(23);
    expect(packageDocument.measurement_series_files).toHaveLength(1);

    const seriesReference = packageDocument.measurement_series_files[0]!;
    const series = MeasurementSeriesFile.parse(
      JSON.parse(readFileSync(join(output, seriesReference.file), "utf8")),
    );
    expect(series.records).toHaveLength(1);
    expect(series.quantity).toBe("heart_rate");

    const manifest = ExportManifest.parse(
      JSON.parse(readFileSync(join(output, "export-manifest.json"), "utf8")),
    );
    expect(manifest).not.toHaveProperty("exported_at");
    expect(manifest.database_revision).toBe(3);
    expect(
      readdirSync(join(output, "attachments")),
    ).toEqual(["01890f3e-e7d3-7cc0-88c8-00000000000e.png"]);
  });

  it("produces byte-identical files from the same database revision", () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-v1-export-"));
    const first = join(directory, "first");
    const second = join(directory, "second");
    expect(ath(["export", first, "--file", FIXTURE], directory).code).toBe(0);
    expect(ath(["export", second, "--file", FIXTURE], directory).code).toBe(0);
    const firstFiles = filesBelow(first);
    const secondFiles = filesBelow(second);
    expect(firstFiles.map(([path]) => path)).toEqual(secondFiles.map(([path]) => path));
    for (let index = 0; index < firstFiles.length; index++) {
      expect(firstFiles[index]![1].equals(secondFiles[index]![1])).toBe(true);
    }
  });
});

describe("v1 package restore", () => {
  it("previews without writing, then restores IDs, revisions, samples, and attachments", () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-v1-restore-"));
    const packageDirectory = join(directory, "package");
    expect(
      ath(["export", packageDirectory, "--file", FIXTURE], directory).code,
    ).toBe(0);

    const destination = join(directory, "restored.ath.sqlite");
    expect(
      ath([
        "init",
        "-y",
        "--empty",
        "--no-skill",
        "--file",
        destination,
      ], directory).code,
    ).toBe(0);

    const preview = ath([
      "import",
      packageDirectory,
      "--file",
      destination,
      "--json",
      "--yes",
    ], directory);
    expect(preview.code).toBe(0);
    expect(JSON.parse(preview.stdout)).toMatchObject({
      restore: true,
      new_records: expect.any(Array),
      matching_records: [],
      conflicts: [],
      write_confirmed: false,
    });
    expect(
      JSON.parse(
        ath([
          "records",
          "list",
          "--file",
          destination,
          "--json",
        ], directory).stdout,
      ).records,
    ).toHaveLength(0);

    const applied = ath([
      "import",
      packageDirectory,
      "--file",
      destination,
      "--yes",
    ], directory);
    expect(applied.code).toBe(0);
    expect(applied.stdout).toContain("restored package at database revision 3");
    expect(ath(["check", destination], directory).code).toBe(0);
    expect(
      JSON.parse(
        ath([
          "records",
          "list",
          "--include-deleted",
          "--limit",
          "100",
          "--file",
          destination,
          "--json",
        ], directory).stdout,
      ).records,
    ).toHaveLength(24);
    expect(
      JSON.parse(
        ath([
          "series",
          "heart_rate",
          "--raw",
          "--file",
          destination,
          "--json",
        ], directory).stdout,
      ).samples,
    ).toHaveLength(1);

    const restoredExport = join(directory, "restored-package");
    expect(
      ath(["export", restoredExport, "--file", destination], directory).code,
    ).toBe(0);
    const originalFiles = filesBelow(packageDirectory);
    const restoredFiles = filesBelow(restoredExport);
    expect(originalFiles.map(([path]) => path)).toEqual(
      restoredFiles.map(([path]) => path),
    );
    for (let index = 0; index < originalFiles.length; index++) {
      expect(originalFiles[index]![1].equals(restoredFiles[index]![1])).toBe(true);
    }
  }, 20_000);

  it("shows and applies an explicit package conflict resolution", () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-v1-conflict-"));
    const basePackage = join(directory, "base-package");
    expect(ath(["export", basePackage, "--file", FIXTURE], directory).code).toBe(0);

    const local = join(directory, "local.ath.sqlite");
    const imported = join(directory, "imported.ath.sqlite");
    for (const database of [local, imported]) {
      expect(
        ath(["init", "-y", "--empty", "--no-skill", "--file", database], directory)
          .code,
      ).toBe(0);
      expect(
        ath(["import", basePackage, "--file", database, "--yes"], directory).code,
      ).toBe(0);
    }

    const profileId = "01890f3e-e7d3-7cc0-88c8-000000000002";
    const profile = JSON.parse(
      ath(
        ["records", "show", profileId, "--file", local, "--json"],
        directory,
      ).stdout,
    ).record as { data: Record<string, unknown> };
    for (const [database, name] of [
      [local, "Local name"],
      [imported, "Imported name"],
    ] as const) {
      expect(
        ath(
          [
            "correct",
            profileId,
            "--revision",
            "1",
            "--data",
            JSON.stringify({ ...profile.data, name }),
            "--file",
            database,
            "--yes",
          ],
          directory,
        ).code,
      ).toBe(0);
    }

    const incomingPackage = join(directory, "incoming-package");
    expect(
      ath(["export", incomingPackage, "--file", imported], directory).code,
    ).toBe(0);
    const preview = ath(
      ["import", incomingPackage, "--file", local, "--json"],
      directory,
    );
    expect(preview.code).toBe(0);
    expect(JSON.parse(preview.stdout).conflicts).toMatchObject([
      { record_id: profileId, message: "record revision history differs" },
    ]);

    const shown = ath(
      ["conflicts", "show", profileId, "--file", local, "--json"],
      directory,
    );
    expect(shown.code).toBe(0);
    expect(JSON.parse(shown.stdout).conflicts[0]).toMatchObject({
      record_id: profileId,
      local: { data: { name: "Local name" } },
      imported: { data: { name: "Imported name" } },
      resolution: null,
    });

    expect(
      ath(
        [
          "conflicts",
          "resolve",
          profileId,
          "--choice",
          "imported",
          "--file",
          local,
          "--json",
        ],
        directory,
      ).code,
    ).toBe(0);
    expect(
      ath(["import", incomingPackage, "--file", local, "--yes"], directory).code,
    ).toBe(0);
    expect(
      JSON.parse(
        ath(
          ["records", "show", profileId, "--file", local, "--json"],
          directory,
        ).stdout,
      ).record,
    ).toMatchObject({
      current_revision: 3,
      data: { name: "Imported name" },
    });
  }, 30_000);

  it("refuses a different nonempty database and every prerelease JSON file", () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-v1-restore-"));
    const packageDirectory = join(directory, "package");
    expect(
      ath(["export", packageDirectory, "--file", FIXTURE], directory).code,
    ).toBe(0);
    expect(ath(["init", "-y", "--no-skill"], directory).code).toBe(0);
    const conflict = ath(["import", packageDirectory, "--json"], directory);
    expect(conflict.code).toBe(0);
    expect(JSON.parse(conflict.stdout).conflicts[0].message).toContain(
      "live databases are not merged",
    );

    const prerelease = ath([
      "import",
      resolve(here, "../examples/demo-athlete/athlete.ath.json"),
      "--json",
    ], directory);
    expect(prerelease.code).toBe(1);
    expect(prerelease.stdout).toContain("prerelease JSON format");
  });
});
