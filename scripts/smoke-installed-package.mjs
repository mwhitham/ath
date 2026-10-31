import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const binaryName = process.platform === "win32" ? "ath-core.exe" : "ath-core";
const packedName = execFileSync("npm", ["pack", "--json", "--ignore-scripts"], {
  cwd: root,
  encoding: "utf8",
});
const filename = JSON.parse(packedName)[0].filename;
const tarball = join(root, filename);
const listed = execFileSync("tar", ["-tzf", tarball], { encoding: "utf8" });
const shipped = `vendor/ath-core/${process.platform}-${process.arch}/${binaryName}`;
if (!listed.split("\n").some((line) => line.endsWith(shipped))) {
  throw new Error(`packed tarball has no ${shipped}`);
}
for (const mapping of ["package/mappings/open-wearables.md", "package/mappings/junction.md"]) {
  if (!listed.split("\n").includes(mapping)) {
    throw new Error(`packed tarball has no ${mapping}`);
  }
}
const verbose = execFileSync("tar", ["-tvzf", tarball], { encoding: "utf8" });
const modeLine = verbose.split("\n").find((line) => line.includes(shipped));
if (modeLine === undefined || !/^-.{2}x/.test(modeLine)) {
  throw new Error(`ath-core is not executable in the packed tarball: ${modeLine}`);
}
const directory = mkdtempSync(join(tmpdir(), "ath-installed-"));
try {
  execFileSync("npm", ["install", "--ignore-scripts", tarball], { cwd: directory, stdio: "inherit" });
  const ath = join(directory, "node_modules", ".bin", "ath");
  const run = (args) => {
    console.log(`$ ath ${args.join(" ")}`);
    return execFileSync(ath, args, { cwd: directory, encoding: "utf8" });
  };
  const oldDatabase = join(directory, "version-1.ath.sqlite");
  copyFileSync(join(root, "tests/fixtures/npm-100-encoding/athlete.ath.sqlite"), oldDatabase);
  const preview = JSON.parse(run(["migrate", "--json", "--file", oldDatabase]));
  if (preview.migration_needed !== true) {
    throw new Error(`expected the synthetic 1.0.0 database to need migration: ${JSON.stringify(preview)}`);
  }
  console.log(run(["migrate", "--yes", "--confirm-migration", preview.database_id, "--file", oldDatabase]));
  console.log(run(["init", "-y", "--empty", "--no-skill"]));
  console.log(run(["log", "--yes", "easy 20 min run"]));
  const gpx = `<?xml version="1.0"?>
<gpx><trk><name>Park loop</name><trkseg>
<trkpt lat="40.0" lon="-105.0"><time>2026-08-09T15:00:00Z</time><extensions><hr>100</hr></extensions></trkpt>
<trkpt lat="40.001" lon="-105.001"><time>2026-08-09T15:00:05Z</time><extensions><hr>130</hr></extensions></trkpt>
<trkpt lat="40.002" lon="-105.002"><time>2026-08-09T15:00:10Z</time><extensions><hr>160</hr></extensions></trkpt>
</trkseg></trk></gpx>`;
  const gpxPath = join(directory, "park.gpx");
  writeFileSync(gpxPath, gpx);
  console.log(run(["import", gpxPath, "--yes"]));
  const records = JSON.parse(run(["records", "list", "--json"]));
  const workout = (Array.isArray(records) ? records : records.records).find(
    (record) => record.record_type === "workout_session" || record.type === "workout_session",
  );
  if (!workout) {
    throw new Error(`no workout in ${JSON.stringify(records)}`);
  }
  const id = workout.record_id ?? workout.id;
  console.log(run(["apply", "heart-rate-zones", "--record", id]));
  console.log(run(["import", join(root, "tests/fixtures/exports/garmin/namespaced-hr.gpx"), "--yes"]));
  console.log(run(["import", join(root, "tests/fixtures/exports/garmin/synthetic-position.fit"), "--yes"]));
  const mapped = run([
    "import",
    join(root, "tests/fixtures/exports/open-wearables/summaries.json"),
    "--mapping",
    "open-wearables",
    "--yes",
  ]);
  console.log(mapped);
  if (!mapped.includes("left out: workout ow-workout-manual: entry_source manual is a personal report")) {
    throw new Error("the installed package did not apply the shipped open-wearables mapping");
  }
  const workouts = JSON.parse(run(["records", "list", "--type", "workout_session", "--json"]));
  const shown = [];
  for (const record of workouts.records) {
    const body = JSON.parse(run(["records", "show", record.record_id, "--json"]));
    const data = body.data ?? body.record?.data;
    shown.push(data);
    const text = JSON.stringify(data);
    if (text.includes("1970-01-01") || text.includes("1989-12-31")) {
      throw new Error(`invented date in ${text}`);
    }
    for (const point of data.route ?? []) {
      if (point.latitude_deg > 90 || point.latitude_deg < -90 || point.longitude_deg > 180 || point.longitude_deg < -180) {
        throw new Error(`route coordinate is outside the valid range in ${text}`);
      }
    }
  }
  const samplePairs = (data) =>
    (data.heart_rate_samples ?? []).map((sample) => [sample.offset_ms, sample.bpm]);
  if (!shown.some((data) => JSON.stringify(samplePairs(data)) === JSON.stringify([[0, 100], [10000, 160]]))) {
    throw new Error("namespaced heart rate was not imported");
  }
  if (!shown.some((data) => data.start_at === "2026-08-09T15:00:00.000Z" && (data.route ?? []).length === 1)) {
    throw new Error("synthetic FIT import did not keep the timed valid point");
  }
  const preciseIndex = shown.findIndex((data) => (data.route ?? []).length > 0);
  if (preciseIndex < 0) {
    throw new Error("no workout with a route to correct");
  }
  const shownPrecise = JSON.parse(run(["records", "show", workouts.records[preciseIndex].record_id, "--json"]));
  const current = shownPrecise.record;
  const corrected = {
    ...current.data,
    maximum_heart_rate_bpm: 190,
    route: (current.data.route ?? []).map((point, index) =>
      index === 0 ? { ...point, latitude_deg: 14.384010205470487 } : point,
    ),
  };
  console.log(run([
    "correct",
    current.record_id,
    "--revision",
    String(current.current_revision),
    "--data",
    JSON.stringify(corrected),
    "--yes",
  ]));
  const check = run(["check"]);
  if (check.includes("INVALID")) {
    throw new Error(check);
  }
  console.log(check);
  console.log("installed package smoke test passed");
} finally {
  rmSync(directory, { recursive: true, force: true });
  if (process.env.ATH_KEEP_TARBALL !== "1") rmSync(tarball, { force: true });
}
