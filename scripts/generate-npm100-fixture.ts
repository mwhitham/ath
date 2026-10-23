/**
 * Regenerates tests/fixtures/npm-100-encoding/athlete.ath.sqlite from npm 1.0.0.
 * Requires network once; commit the output and update scripts/check-public-data.ts.
 */
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "tests/fixtures/npm-100-encoding");
const exportsDir = join(root, "tests/fixtures/exports");

mkdirSync(outDir, { recursive: true });
const run = (args: string[]) => {
  execFileSync("npx", ["--yes", "athleticstandard@1.0.0", ...args], {
    cwd: outDir,
    encoding: "utf8",
    stdio: "inherit",
  });
};
run(["init", "-y", "--no-skill"]);
for (const provider of ["whoop", "oura", "apple"]) {
  run(["import", "-y", resolve(exportsDir, provider)]);
}
console.log(`wrote ${join(outDir, "athlete.ath.sqlite")}`);
