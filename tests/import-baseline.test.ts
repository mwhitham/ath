import { execFile } from "node:child_process";
import { once } from "node:events";
import { createWriteStream, mkdtempSync, readdirSync, readFileSync, statSync } from "node:fs";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { ZipFile } from "yazl";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import {
  BASELINE_FROM,
  comparisonString,
  fieldBasisString,
  MEASURED_RECORD_TYPES,
  readImportSnapshot,
} from "./helpers/import-baseline.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");
const EXPORTS = resolve(here, "fixtures/exports");
const BASELINE = resolve(here, "fixtures/import-baseline");
const PACKAGE = resolve(here, "fixtures/version-2-0-0/package");
const run = promisify(execFile);

async function ath(
  args: string[],
  cwd: string,
  env: NodeJS.ProcessEnv = {},
): Promise<{ output: string; code: number }> {
  try {
    const { stdout, stderr } = await run(TSX, [CLI, ...args], {
      cwd,
      env: { ...process.env, ...env },
      maxBuffer: 64 * 1024 * 1024,
    });
    return { output: stdout + stderr, code: 0 };
  } catch (error) {
    const err = error as { code?: number; stdout?: string; stderr?: string };
    return { output: (err.stdout ?? "") + (err.stderr ?? ""), code: err.code ?? 1 };
  }
}

function filesUnder(root: string): string[] {
  return readdirSync(root).flatMap((name) => {
    const path = join(root, name);
    return statSync(path).isDirectory() ? filesUnder(path) : [path];
  });
}

async function zipAppleExport(directory: string): Promise<string> {
  const path = join(directory, "health.zip");
  const zip = new ZipFile();
  const root = join(EXPORTS, "apple");
  for (const file of filesUnder(root).sort()) {
    const name = relative(root, file).split("\\").join("/");
    zip.addBuffer(readFileSync(file), `apple_health_export/${name}`, { mtime: new Date(0) });
  }
  const output = createWriteStream(path);
  zip.outputStream.pipe(output);
  zip.end();
  await once(output, "close");
  return path;
}

const PERSONAL_DAY = "2026-08-09";
const PERSONAL_KEY = "invented-personal-key";
let personalServer: Server;
let personalBase = "";

function startPersonalServer(): Promise<void> {
  personalServer = createServer((request, response) => {
    const path = request.url ?? "";
    if (request.method === "POST" && path.endsWith("/personal-api-key/token")) {
      response.end("aaa.bbb.ccc");
      return;
    }
    if (path.startsWith("/activitylist-service/activities/search/activities")) {
      response.setHeader("content-type", "application/json");
      response.end(
        JSON.stringify([
          {
            activityId: 7001,
            activityName: "Invented personal loop",
            startTimeGMT: "2026-08-09T15:00:00.000Z",
            duration: 600,
            activityType: { typeKey: "running" },
            Feeling: "steady",
          },
        ]),
      );
      return;
    }
    if (path === `/hrv-service/hrv/${PERSONAL_DAY}`) {
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ LastNightAvg: 41, weeklyAvg: 44, status: "BALANCED" }));
      return;
    }
    response.statusCode = 404;
    response.end("not found");
  });
  return new Promise((resolveStart) => {
    personalServer.listen(0, "127.0.0.1", () => {
      personalBase = `http://127.0.0.1:${(personalServer.address() as AddressInfo).port}`;
      resolveStart();
    });
  });
}

type Row = {
  file: string;
  importInto: (directory: string) => Promise<{ output: string; code: number }>;
};

const importPath = (path: string) => (directory: string) =>
  ath(["import", path, "--yes"], directory);

const ROWS: Row[] = [
  { file: "01-apple-dir.json", importInto: importPath(join(EXPORTS, "apple")) },
  { file: "02-apple-inline.json", importInto: importPath(join(EXPORTS, "apple-inline-route")) },
  { file: "03-apple-file.json", importInto: importPath(join(EXPORTS, "apple-file-reference")) },
  {
    file: "04-apple-mismatch.json",
    // One GPX point has no UTC offset, and the importer reads it in the machine's zone.
    importInto: (directory) =>
      ath(["import", join(EXPORTS, "apple-route-mismatch"), "--yes"], directory, { TZ: "UTC" }),
  },
  { file: "05-cycles.json", importInto: importPath(join(EXPORTS, "whoop")) },
  { file: "06-ring.json", importInto: importPath(join(EXPORTS, "oura")) },
  { file: "07-ring-portal.json", importInto: importPath(join(EXPORTS, "oura-member-portal")) },
  { file: "08-ring-offset.json", importInto: importPath(join(EXPORTS, "oura-offset/sleep.csv")) },
  { file: "09-ring-daily.json", importInto: importPath(join(EXPORTS, "oura-portal")) },
  { file: "10-ring-spo2.json", importInto: importPath(join(EXPORTS, "oura-spo2")) },
  { file: "11-park-loop.json", importInto: importPath(join(EXPORTS, "garmin/park-loop.json")) },
  { file: "12-namespaced-hr.json", importInto: importPath(join(EXPORTS, "garmin/namespaced-hr.gpx")) },
  { file: "13-position.json", importInto: importPath(join(EXPORTS, "garmin/synthetic-position.fit")) },
  { file: "14-bulk.json", importInto: importPath(join(EXPORTS, "strava")) },
  {
    file: "15-personal-day.json",
    importInto: (directory) =>
      ath(["pull", "garmin", "--date", PERSONAL_DAY, "--base-url", personalBase], directory, {
        GARMIN_PERSONAL_API_KEY: PERSONAL_KEY,
      }),
  },
  { file: "16-package.json", importInto: importPath(PACKAGE) },
  {
    file: "17-apple-zip.json",
    importInto: async (directory) => ath(["import", await zipAppleExport(directory), "--yes"], directory),
  },
  { file: "18-apple-bare-xml.json", importInto: importPath(join(EXPORTS, "apple/export.xml")) },
];

describe(`import baseline from ${BASELINE_FROM.slice(0, 7)}`, () => {
  beforeAll(startPersonalServer);
  afterAll(() => {
    personalServer.close();
  });

  for (const row of ROWS) {
    it(`${row.file} is byte-for-byte the committed comparison`, async () => {
      const directory = mkdtempSync(join(tmpdir(), "ath-baseline-"));
      const init = await ath(["init", "-y", "--empty", "--no-skill"], directory);
      expect(init.code, init.output).toBe(0);
      const imported = await row.importInto(directory);
      expect(imported.code, imported.output).toBe(0);
      const snapshot = await readImportSnapshot(join(directory, "athlete.ath.sqlite"));
      const text = comparisonString(snapshot, { withoutKey: "field_basis" });
      expect(text.includes(directory)).toBe(false);
      expect(text).toBe(readFileSync(join(BASELINE, row.file), "utf8"));

      const basis = fieldBasisString(snapshot);
      expect(basis).toBe(readFileSync(join(BASELINE, "field-basis", row.file), "utf8"));
      for (const record of snapshot.records) {
        const fieldBasis = (record.data as { field_basis?: { value?: string } }).field_basis;
        if (!MEASURED_RECORD_TYPES.has(record.record_type)) {
          expect(fieldBasis, record.record_type).toBeUndefined();
        } else if (row.file === "16-package.json") {
          expect(fieldBasis, "a restored 2.0.0 record keeps no field_basis").toBeUndefined();
        } else {
          expect(fieldBasis?.value, record.record_type).toBeDefined();
        }
      }
    }, 120_000);
  }

  it("keeps LastNightAvg as its own summary type", () => {
    const body = JSON.parse(readFileSync(join(BASELINE, "15-personal-day.json"), "utf8")) as {
      records: Array<{ record_type: string; data: { summary_type?: string } }>;
    };
    expect(
      body.records.some(
        (record) =>
          record.record_type === "provider_summary" && record.data.summary_type === "LastNightAvg",
      ),
    ).toBe(true);
  });
});
