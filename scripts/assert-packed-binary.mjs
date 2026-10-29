import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";

const filename = process.argv[2];
if (filename === undefined || filename === "") {
  console.error(
    "Pass the tarball path. The release workflow runs:\n" +
      "  node scripts/assert-packed-binary.mjs athleticstandard-<version>.tgz\n" +
      "  npm publish athleticstandard-<version>.tgz",
  );
  process.exit(1);
}
if (!existsSync(filename)) {
  console.error(`No packed tarball at ${filename}.`);
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
      "Do not publish this file. The release workflow packs with npm pack, checks this path, then runs npm publish on it.",
  );
  process.exit(1);
}
console.log(`${filename} ath-core binaries are executable`);
