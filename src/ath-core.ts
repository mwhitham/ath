/**
 * Call the shared Rust library. The command-line tool does not keep a second
 * copy of import or calculation rules.
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const debugBinary = resolve(here, "../crates/ath-core/target/debug/ath-core");
const releaseBinary = resolve(here, "../crates/ath-core/target/release/ath-core");

export function athCoreBinary(): string {
  if (existsSync(debugBinary)) return debugBinary;
  if (existsSync(releaseBinary)) return releaseBinary;
  const build = spawnSync(
    "cargo",
    ["build", "--manifest-path", resolve(here, "../crates/ath-core/Cargo.toml")],
    { encoding: "utf8" },
  );
  if (build.status !== 0 || !existsSync(debugBinary)) {
    throw new Error(
      `ath-core is not built. ${build.stderr || build.stdout || "cargo build failed"}`,
    );
  }
  return debugBinary;
}

export function runAthCore(args: string[], env: NodeJS.ProcessEnv = process.env): unknown {
  const result = spawnSync(athCoreBinary(), args, { encoding: "utf8", env });
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
