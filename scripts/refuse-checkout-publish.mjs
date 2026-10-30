import { readFileSync } from "node:fs";

const version = JSON.parse(readFileSync("package.json", "utf8")).version;
const tarball = `athleticstandard-${version}.tgz`;
console.error(
  "Do not publish from a checkout. The release workflow packs this file, checks it, and publishes that same file:\n" +
    `  node scripts/assert-packed-binary.mjs ${tarball}\n` +
    `  npm publish ${tarball}`,
);
process.exit(1);
