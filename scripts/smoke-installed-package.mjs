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
  console.log("installed package smoke test passed");
} finally {
  rmSync(directory, { recursive: true, force: true });
  rmSync(tarball, { force: true });
}
