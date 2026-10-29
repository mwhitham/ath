import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const name = process.platform === "win32" ? "ath-core.exe" : "ath-core";
const built = resolve(root, "crates/ath-core/target/release", name);
const destinationDir = resolve(root, "vendor/ath-core", `${process.platform}-${process.arch}`);
const destination = resolve(destinationDir, name);
mkdirSync(destinationDir, { recursive: true });
copyFileSync(built, destination);
chmodSync(destination, 0o755);
console.log(`staged ${destination}`);
