import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  checkDatabase,
  createDatabase,
  importProviderRecords,
  listRecords,
  previewProviderImport,
  type AthleticDatabase,
} from "../src/db/index.js";

const fixture = resolve("tests/fixtures/exports/apple-inline-route");
const opened: AthleticDatabase[] = [];

afterEach(() => {
  for (const database of opened.splice(0)) database.close();
});

describe("an inline Apple WorkoutRoute", () => {
  it("stores the location points and counts the ones it cannot store", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-apple-route-"));
    const database = await createDatabase(join(directory, "athlete.ath.sqlite"), {
      createdAt: "2026-10-13T11:00:00.000Z",
    });
    opened.push(database);
    const preview = await previewProviderImport(database, fixture, {
      received_at: "2026-10-13T12:00:00.000Z",
    });
    expect(preview.skipped["inline WorkoutRoute points we could not store"]).toEqual({
      count: 2,
      example: 'Location latitude "120" is outside -90 to 90',
    });
    expect(preview.skipped["inline WorkoutRoute had no location points"]).toMatchObject({
      count: 1,
    });
    expect(preview.conflicts).toEqual([]);
    await importProviderRecords(database, preview);

    const workouts = await listRecords(database, { record_type: "workout_session", limit: 10 });
    const run = workouts.find(
      (record) =>
        record.record_type === "workout_session" && record.data.activity_type === "running",
    );
    expect(run).toMatchObject({
      record_type: "workout_session",
      data: {
        route: [
          {
            recorded_at: "2026-06-14T13:01:00.000Z",
            latitude_deg: 40.25,
            longitude_deg: -88.5,
            supplied_fields: { altitude: "200" },
          },
          {
            recorded_at: "2026-06-14T13:02:00.000Z",
            latitude_deg: 40.26,
            longitude_deg: -88.51,
            supplied_fields: { altitude: "201" },
          },
        ],
      },
    });
    const walk = workouts.find(
      (record) =>
        record.record_type === "workout_session" && record.data.activity_type === "walking",
    );
    expect(walk?.record_type === "workout_session" && walk.data.route).toBeUndefined();
    const check = await checkDatabase(database);
    expect(check.valid, JSON.stringify(check.issues, null, 2)).toBe(true);
  });
});
