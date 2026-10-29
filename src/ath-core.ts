/**
 * Call the shared Rust library. The command-line tool does not keep a second
 * copy of import or calculation rules.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const packageRoot = resolve(here, "..");
const crateRoot = resolve(packageRoot, "crates/ath-core");
const debugBinary = resolve(crateRoot, "target/debug/ath-core");
const releaseBinary = resolve(crateRoot, "target/release/ath-core");

function binaryName(): string {
  return process.platform === "win32" ? "ath-core.exe" : "ath-core";
}

function shippedBinary(): string {
  return resolve(packageRoot, "vendor/ath-core", `${process.platform}-${process.arch}`, binaryName());
}

function newestMtime(path: string): number {
  if (!existsSync(path)) return 0;
  const stat = statSync(path);
  if (!stat.isDirectory()) return stat.mtimeMs;
  let newest = stat.mtimeMs;
  for (const name of readdirSync(path)) {
    newest = Math.max(newest, newestMtime(join(path, name)));
  }
  return newest;
}

function rustSourceMtime(): number | null {
  if (!existsSync(join(crateRoot, "Cargo.toml"))) return null;
  return Math.max(
    newestMtime(join(crateRoot, "Cargo.toml")),
    newestMtime(join(crateRoot, "Cargo.lock")),
    newestMtime(join(crateRoot, "src")),
  );
}

function isOlderThanSource(binary: string, sourceMtime: number): boolean {
  return statSync(binary).mtimeMs < sourceMtime;
}

function rebuildDebugBinary(): string {
  const build = spawnSync(
    "cargo",
    ["build", "--manifest-path", join(crateRoot, "Cargo.toml")],
    { encoding: "utf8" },
  );
  if (build.status !== 0 || !existsSync(debugBinary)) {
    throw new Error(
      `ath-core binary is older than the Rust source and could not be rebuilt. ${build.stderr || build.stdout || "cargo build failed"}`,
    );
  }
  return debugBinary;
}

export function athCoreBinary(): string {
  const sourceMtime = rustSourceMtime();
  if (sourceMtime !== null) {
    const candidates = [debugBinary, releaseBinary, shippedBinary()].filter((path) => existsSync(path));
    const current = candidates.find((path) => !isOlderThanSource(path, sourceMtime));
    if (current !== undefined) return current;
    return rebuildDebugBinary();
  }
  const shipped = shippedBinary();
  if (existsSync(shipped)) return shipped;
  throw new Error(
    `ath-core is not built for ${process.platform}-${process.arch}. The installed package has no binary at ${shipped}.`,
  );
}

export function runAthCore(
  args: string[],
  env: NodeJS.ProcessEnv = process.env,
  stdin?: string,
): unknown {
  const result = spawnSync(athCoreBinary(), args, {
    encoding: "utf8",
    env,
    input: stdin,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) {
    throw new Error(result.error.message);
  }
  if ((result.status ?? 1) !== 0) {
    throw new Error((result.stderr || result.stdout || "ath-core failed").trim());
  }
  const text = result.stdout.trim();
  if (text === "") return {};
  return JSON.parse(text) as unknown;
}
