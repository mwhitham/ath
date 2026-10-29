import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { basename } from "node:path";

/**
 * Health-shaped files are allowed only when their exact synthetic contents were
 * reviewed. A changed hash is a prompt for another review, not proof of origin.
 */
const approvedSyntheticFixtures = new Map<string, string>([
  [
    "examples/demo-athlete/athlete.ath.json",
    "7a77fce18b600950cae51e745141f79d1aa14e5f6864847b99c560265a78fb27",
  ],
  [
    "examples/demo-athlete/athlete.ath.sqlite",
    "442e9a966711660c2a03d701b5de2c55479a78330702083a3a67a182992a7856",
  ],
  [
    "examples/demo-athlete/attachments/01890f3e-e7d3-7cc0-88c8-00000000000e.png",
    "d3fe8e599fb1deea9821063c744434802875c26cceeb9f2d55e8566b15a012f4",
  ],
  [
    "tests/fixtures/exports/apple/export.xml",
    "fbb3b4f8ee38f575f32387dc48b5d695caaf76f7de16d920659a1617efae7fcf",
  ],
  [
    "tests/fixtures/exports/apple/electrocardiograms/ecg_2026-08-07.csv",
    "f87620e6816813d01e743c5476d823373556ea3c58523161cb4ebceb0a05e409",
  ],
  [
    "tests/fixtures/exports/apple/electrocardiograms/ecg_2026-08-09.csv",
    "32768721c3d8a1433f598b36370e9670ce6119fb3a53ee63c8cf73c7182e95a9",
  ],
  [
    "tests/fixtures/exports/apple/workout-routes/route_2026-08-09_7.00pm.gpx",
    "8033b604f2bdb46358a7541ad462c9b9d851f315df6f1d1024698800b53e3f52",
  ],
  [
    "tests/fixtures/exports/oura/oura_daily_readiness.csv",
    "f096e2cf7bd4cbea7fb9b9e181c8f55540632f7e0299501ed380e150fb6fbe42",
  ],
  [
    "tests/fixtures/exports/oura/oura_sleep.csv",
    "ed59f11d1bb995278882238e26eea72830a114be67d74b2d223f96c8bfe6fb1b",
  ],
  [
    "tests/fixtures/exports/oura-member-portal/member-portal-dailyreadiness.csv",
    "1bf215c48dd4b8fc433d6e63dd067c1d905e373a4ce5f1a02c5ed990daaed0e8",
  ],
  [
    "tests/fixtures/exports/oura-member-portal/member-portal-dailysleep.csv",
    "b9ba34447d2aeb8fb9bd66a3dd90fdf9346aab4c516182578e4ebbad8ae105d0",
  ],
  [
    "tests/fixtures/exports/oura-spo2/oura_sleep_spo2_flat.csv",
    "3e7aaefc6b60bca8f9120bb60020aa61f15c3b989c80848935d6bac0e4429ce7",
  ],
  [
    "tests/fixtures/exports/oura-spo2/oura_sleep_spo2_json.csv",
    "949053c0c1503dbabd6773acd2813888c9d9ff360588fca2e253443476632902",
  ],
  [
    "tests/fixtures/exports/oura-offset/sleep.csv",
    "da6d0e3e3a6a36759c652fa9bda6e8e93f8673e2a3777cc2f96f70f7f17bdd23",
  ],
  [
    "tests/fixtures/exports/oura-portal/dailyreadiness.csv",
    "1bf215c48dd4b8fc433d6e63dd067c1d905e373a4ce5f1a02c5ed990daaed0e8",
  ],
  [
    "tests/fixtures/exports/oura-portal/dailysleep.csv",
    "b9ba34447d2aeb8fb9bd66a3dd90fdf9346aab4c516182578e4ebbad8ae105d0",
  ],
  [
    "tests/fixtures/exports/oura-portal/dailyactivity.csv",
    "d6ba261d94ccc36bce98d959ea17339b39f2642e241f584d0fdb8be18a6abc62",
  ],
  [
    "tests/fixtures/npm-100-encoding/athlete.ath.sqlite",
    "8469f3dfe29768f78b86c4d23e641b5070fd78683b77a7d342925ddc1b81a075",
  ],
  [
    "tests/fixtures/exports/whoop/journal_entries.csv",
    "db60258f8c69f115afe427fdc442e6c97c85b9cc96b3d086e5aabc52157701d9",
  ],
  [
    "tests/fixtures/exports/whoop/physiological_cycles.csv",
    "e5d0ea4de8c24ac4c9b7fd9795be62c038170f37f6b0c6be51b1ba367b8fb356",
  ],
  [
    "tests/fixtures/exports/whoop/sleeps.csv",
    "eb5825dea3f3c26bffb84281f4bac52daa410dd6dcba08cbd9a92442f531229c",
  ],
  [
    "tests/fixtures/exports/garmin/park-loop.json",
    "1a9a4204bfa965710d21f9bf96649e7817f0b71c56f1aa781a82d861d4a96c66",
  ],
  [
    "tests/fixtures/exports/strava/activities.csv",
    "9e23b604846202e075658f771bbef4fe433c694cf660d177f286b05e2acb44a7",
  ],
  [
    "tests/fixtures/exports/strava/activities/river.gpx",
    "b9708c7f22afda4fa8b18247a27459722e87a0e731f01c171960058c83cc2fdd",
  ],
  [
    "tests/fixtures/exports/whoop/workouts.csv",
    "4cb6248df5443456996386a02325e0d21532d4251bfa5649c02ea0ca0d4c4ce1",
  ],
]);

const requiredIgnoreRules = [
  ".private-data/",
  "private-validation/",
  "validation-data/",
  "*.ath.json",
  "*.ath.sqlite",
  "*.ath.sqlite-wal",
  "*.ath.sqlite-shm",
  "*.fit",
  "*.gpx",
  "*.xml",
  "*.zip",
  "*.csv",
];

function trackedFiles(): string[] {
  return execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" })
    .split("\0")
    .filter(Boolean);
}

function isHealthDataFile(path: string): boolean {
  const lower = path.toLowerCase();
  if (lower.startsWith("schema/")) return false;

  const sensitiveEndings = [
    ".ath.json",
    ".ath.sqlite",
    ".csv",
    ".fit",
    ".gpx",
    ".tcx",
    ".xml",
    ".zip",
  ];
  if (sensitiveEndings.some((ending) => lower.endsWith(ending))) return true;

  const name = basename(lower);
  return (
    name.endsWith(".json") &&
    /(apple[-_ ]?health|athlete|export|garmin|healthkit|oura|whoop)/.test(name)
  );
}

function isFixturePath(path: string): boolean {
  return path.startsWith("examples/demo-athlete/") || path.startsWith("tests/fixtures/exports/");
}

function sha256(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

const errors: string[] = [];
const tracked = new Set(trackedFiles());
const ignoreLines = new Set(
  readFileSync(".gitignore", "utf8")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean),
);

for (const path of tracked) {
  if ((isHealthDataFile(path) || isFixturePath(path)) && !approvedSyntheticFixtures.has(path)) {
    errors.push(
      `${path}: health-data file is not on the synthetic-fixture allowlist. ` +
        "Remove it from Git. If it is invented test data, review it and add its exact path and hash.",
    );
  }
}

for (const [path, expectedHash] of approvedSyntheticFixtures) {
  if (!tracked.has(path)) {
    errors.push(`${path}: approved synthetic fixture is missing or is not tracked.`);
    continue;
  }

  const actualHash = sha256(path);
  if (actualHash !== expectedHash) {
    errors.push(
      `${path}: approved synthetic fixture changed (expected ${expectedHash}, got ${actualHash}). ` +
        "Confirm every value is invented, then update the reviewed hash.",
    );
  }

  if (isHealthDataFile(path) && !ignoreLines.has(`!${path}`)) {
    errors.push(`${path}: add an explicit !${path} exception to .gitignore.`);
  }
}

for (const rule of requiredIgnoreRules) {
  if (!ignoreLines.has(rule)) {
    errors.push(`.gitignore: required private-data rule is missing: ${rule}`);
  }
}

if (errors.length > 0) {
  console.error("Public-data check failed:\n");
  for (const error of errors) console.error(`- ${error}`);
  console.error("\nReal health exports belong in encrypted storage outside Git.");
  process.exitCode = 1;
} else {
  console.log(
    `Public-data check passed: ${approvedSyntheticFixtures.size} approved synthetic fixtures; no other health-data files tracked.`,
  );
}
