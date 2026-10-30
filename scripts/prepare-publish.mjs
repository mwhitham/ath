import { chmodSync, existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const vendor = join(root, "vendor/ath-core");
const binaryName = process.platform === "win32" ? "ath-core.exe" : "ath-core";
const current = join(vendor, `${process.platform}-${process.arch}`, binaryName);
if (!existsSync(current)) {
  console.error(
    `ath-core is not staged for ${process.platform}-${process.arch}. ` +
      "Run cargo build --release --manifest-path crates/ath-core/Cargo.toml " +
      "and node scripts/stage-ath-core.mjs before publishing. " +
      "The release workflow stages Linux, macOS, and Windows together.",
  );
  process.exit(1);
}

function markExecutable(path) {
  const stat = statSync(path);
  if (stat.isDirectory()) {
    for (const entry of readdirSync(path)) markExecutable(join(path, entry));
    return;
  }
  chmodSync(path, 0o755);
}

markExecutable(vendor);
console.log(`marked ${vendor} executable`);
