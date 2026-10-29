import { execFileSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";

const name = process.env.npm_package_name ?? "athleticstandard";
const version = process.env.npm_package_version ?? "2.0.0";
const filename = existsSync(`${name}-${version}.tgz`)
  ? `${name}-${version}.tgz`
  : readdirSync(".").find((entry) => entry.endsWith(".tgz"));
if (filename === undefined) {
  console.error("No packed tarball was found. Publish with npm pack, which the release workflow uses.");
  process.exit(1);
}
const listing = execFileSync("tar", ["-tvzf", filename], { encoding: "utf8" });
const binaries = listing
  .split("\n")
  .filter((line) => /\/ath-core(\.exe)?$/.test(line.trim()));
if (binaries.length === 0) {
  console.error(`${filename} has no ath-core binary. Stage vendor/ath-core before packing.`);
  process.exit(1);
}
const unexecutable = binaries.filter((line) => !/^-.{2}x/.test(line));
if (unexecutable.length > 0) {
  console.error(
    `${filename} stores ath-core without the executable bit:\n${unexecutable.join("\n")}\n` +
      "pnpm pack keeps that bit only on bin entries. Publish the tarball from npm pack. " +
      "The release workflow builds that tarball and the smoke test checks it.",
  );
  process.exit(1);
}
console.log(`${filename} ath-core binaries are executable`);
