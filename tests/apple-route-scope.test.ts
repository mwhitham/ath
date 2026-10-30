import { createWriteStream, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { once } from "node:events";
import { ZipFile } from "yazl";
import { afterEach, describe, expect, it } from "vitest";
import {
  createDatabase,
  importProviderRecords,
  listRecords,
  previewProviderImport,
  type AthleticDatabase,
} from "../src/db/index.js";
import { detectSharedImport } from "../src/shared-import.js";

const mismatch = resolve("tests/fixtures/exports/apple-route-mismatch");
const opened: AthleticDatabase[] = [];

afterEach(() => {
  for (const database of opened.splice(0)) database.close();
});

async function database(): Promise<AthleticDatabase> {
  const directory = mkdtempSync(join(tmpdir(), "ath-route-scope-"));
  const openedDatabase = await createDatabase(join(directory, "athlete.ath.sqlite"), {
    createdAt: "2026-10-13T11:00:00.000Z",
  });
  opened.push(openedDatabase);
  return openedDatabase;
}

function workoutXml(reference: string): string {
  return `<?xml version="1.0" encoding="UTF-8"?>
<HealthData>
 <Workout workoutActivityType="HKWorkoutActivityTypeRunning" duration="30" durationUnit="min" sourceName="Apple Watch" startDate="2026-06-14 08:00:00 -0500" endDate="2026-06-14 08:30:00 -0500">
  <WorkoutRoute sourceName="Demo Phone">
   <FileReference path="${reference}"/>
  </WorkoutRoute>
 </Workout>
</HealthData>
`;
}

function gpx(lat: string, lon: string): string {
  return `<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1"><trk><trkseg>
 <trkpt lat="${lat}" lon="${lon}"><time>2026-06-14T08:01:00-05:00</time></trkpt>
 <trkpt lat="${lat}" lon="${lon}"><time>2026-06-14T08:02:00-05:00</time></trkpt>
</trkseg></trk></gpx>
`;
}

describe("a FileReference route stays inside the export and the workout", () => {
  it("does not store a GPX whose times are outside the workout", async () => {
    const db = await database();
    const preview = await previewProviderImport(db, mismatch, {
      received_at: "2026-10-13T12:00:00.000Z",
    });
    expect(preview.skipped["workout route file is outside the workout's time"]).toEqual({
      count: 1,
      example: "/workout-routes/route_2026-07-01.gpx",
    });
    expect(preview.skipped["workout route file is already stored on another workout"]).toEqual({
      count: 1,
      example: "/workout-routes/route_2026-08-02.gpx",
    });
    expect(preview.skipped["GPX route points we could not store"]).toMatchObject({
      count: 1,
    });
    await importProviderRecords(db, preview);
    const workouts = await listRecords(db, { record_type: "workout_session", limit: 10 });
    const june = workouts.find(
      (record) => record.record_type === "workout_session" && record.data.activity_type === "running",
    );
    const august = workouts.find(
      (record) => record.record_type === "workout_session" && record.data.activity_type === "cycling",
    );
    const second = workouts.find(
      (record) => record.record_type === "workout_session" && record.data.activity_type === "walking",
    );
    expect(june?.record_type === "workout_session" && june.data.route).toBeUndefined();
    expect(august?.record_type === "workout_session" && august.data.route).toEqual([
      expect.objectContaining({ latitude_deg: 15.5, longitude_deg: 59.5 }),
      expect.objectContaining({ latitude_deg: 15.75, longitude_deg: 59.75 }),
    ]);
    expect(second?.record_type === "workout_session" && second.data.route).toBeUndefined();
  });

  it("does not read a sibling folder when the input is a bare export.xml", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-bare-xml-"));
    writeFileSync(join(directory, "export.xml"), workoutXml("/workout-routes/near.gpx"));
    mkdirSync(join(directory, "workout-routes"));
    writeFileSync(join(directory, "workout-routes", "near.gpx"), gpx("16.5", "60.5"));
    mkdirSync(join(directory, "other", "workout-routes"), { recursive: true });
    writeFileSync(join(directory, "other", "workout-routes", "near.gpx"), gpx("17.5", "61.5"));
    const db = await database();
    const preview = await previewProviderImport(db, join(directory, "export.xml"), {
      received_at: "2026-10-13T12:00:00.000Z",
    });
    await importProviderRecords(db, preview);
    const workouts = await listRecords(db, { record_type: "workout_session", limit: 5 });
    const run = workouts.find((record) => record.record_type === "workout_session");
    expect(run?.record_type === "workout_session" && run.data.route).toEqual([
      expect.objectContaining({ latitude_deg: 16.5, longitude_deg: 60.5 }),
      expect.objectContaining({ latitude_deg: 16.5, longitude_deg: 60.5 }),
    ]);
  });

  it("names a missing GPX when the export has no route files", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-missing-gpx-"));
    writeFileSync(join(directory, "export.xml"), workoutXml("/workout-routes/missing.gpx"));
    const db = await database();
    const preview = await previewProviderImport(db, join(directory, "export.xml"), {
      received_at: "2026-10-13T12:00:00.000Z",
    });
    expect(preview.skipped["workout route file named by a WorkoutRoute was not in the export"]).toEqual({
      count: 1,
      example: "/workout-routes/missing.gpx",
    });
  });

  it("does not treat a zip entry named settings_export.xml as Apple Health", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-settings-xml-"));
    const zipPath = join(directory, "activities.zip");
    const zip = new ZipFile();
    zip.addBuffer(Buffer.from("<settings/>"), "settings_export.xml");
    zip.addBuffer(Buffer.from(gpx("18.5", "62.5")), "activity.gpx");
    const output = createWriteStream(zipPath);
    zip.outputStream.pipe(output);
    zip.end();
    await once(output, "close");
    expect(await detectSharedImport(zipPath)).toBe("garmin");
  });
});
