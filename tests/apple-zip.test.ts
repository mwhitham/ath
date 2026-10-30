import { execFileSync } from "node:child_process";
import { createWriteStream, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { once } from "node:events";
import { ZipFile } from "yazl";
import { describe, expect, it } from "vitest";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const fixture = resolve(here, "fixtures/exports/apple-file-reference");

function ath(args: string[], cwd: string): { stdout: string; code: number } {
  try {
    const stdout = execFileSync(TSX, [CLI, ...args], { cwd, encoding: "utf8", stdio: "pipe" });
    return { stdout, code: 0 };
  } catch (error) {
    const failed = error as { status: number; stdout?: string; stderr?: string };
    return { stdout: `${failed.stdout ?? ""}${failed.stderr ?? ""}`, code: failed.status ?? 1 };
  }
}

async function writeZip(path: string, entries: Array<{ name: string; bytes: Buffer }>): Promise<void> {
  const zip = new ZipFile();
  for (const entry of entries) zip.addBuffer(entry.bytes, entry.name);
  const output = createWriteStream(path);
  zip.outputStream.pipe(output);
  zip.end();
  await once(output, "close");
}

describe("an Apple export.zip that also contains a GPX route", () => {
  it("imports the health data and the route named by FileReference", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-apple-zip-"));
    const zipPath = join(directory, "export.zip");
    await writeZip(zipPath, [
      {
        name: "apple_health_export/export.xml",
        bytes: readFileSync(join(fixture, "export.xml")),
      },
      {
        name: "apple_health_export/workout-routes/route_2026-06-14.gpx",
        bytes: readFileSync(join(fixture, "workout-routes/route_2026-06-14.gpx")),
      },
    ]);
    const database = join(directory, "athlete.ath.sqlite");
    const created = ath(["init", "-y", "--empty", "--no-skill", "--file", database], directory);
    expect(created.code, created.stdout).toBe(0);

    const imported = ath(["import", zipPath, "--yes", "--file", database], directory);
    expect(imported.code, imported.stdout).toBe(0);
    expect(imported.stdout).toContain("Apple Health export preview");
    expect(imported.stdout).toContain("route points stored: 2");
    expect(imported.stdout).toContain("GPX route points we could not store");
    expect(imported.stdout).not.toContain("not a training or recovery file");
    expect(imported.stdout).not.toContain("had no location points");

    const listed = ath(
      ["records", "list", "--json", "--limit", "20", "--file", database],
      directory,
    );
    expect(listed.code, listed.stdout).toBe(0);
    const body = JSON.parse(listed.stdout) as {
      records: Array<{ record_type: string; data: Record<string, unknown> }>;
    };
    expect(
      body.records.some(
        (record) =>
          record.record_type === "point_measurement" &&
          record.data.quantity === "resting_heart_rate",
      ),
    ).toBe(true);
    const workout = body.records.find((record) => record.record_type === "workout_session");
    expect(workout?.data.route).toEqual([
      expect.objectContaining({
        recorded_at: "2026-06-14T13:01:00.000Z",
        latitude_deg: 13.5,
        longitude_deg: 57.5,
        input_timestamp: expect.objectContaining({ utc_offset_text: "-05:00" }),
      }),
      expect.objectContaining({
        recorded_at: "2026-06-14T13:02:00.000Z",
        latitude_deg: 13.75,
        longitude_deg: 57.75,
        input_timestamp: expect.objectContaining({ utc_offset_text: "-05:00" }),
      }),
    ]);
    expect(workout?.data.supplied_fields).toMatchObject({
      WorkoutRoute: [
        {
          sourceName: "Demo Phone",
          creationDate: "2026-06-14 08:05:00 -0500",
          startDate: "2026-06-14 08:00:00 -0500",
          endDate: "2026-06-14 08:30:00 -0500",
          FileReference: "/workout-routes/route_2026-06-14.gpx",
          MetadataEntry: [{ key: "HKMetadataKeySyncVersion", value: "4" }],
        },
      ],
    });
    const checked = ath(["check", "--file", database], directory);
    expect(checked.code, checked.stdout).toBe(0);
    expect(checked.stdout).toContain("valid");
  });
});
