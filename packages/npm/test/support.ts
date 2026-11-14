/**
 * Shared test setup: the native executable under test and temporary folders.
 *
 * Tests run the executable named by ATH_NATIVE_EXECUTABLE. Without it they
 * use the debug build in native/target, which `cargo build` makes.
 */
import { existsSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const repositoryRoot = resolve(here, "..", "..", "..");

const executableName = process.platform === "win32" ? "ath.exe" : "ath";
const debugBuild = join(repositoryRoot, "native", "target", "debug", executableName);

if (process.env.ATH_NATIVE_EXECUTABLE === undefined || process.env.ATH_NATIVE_EXECUTABLE === "") {
  process.env.ATH_NATIVE_EXECUTABLE = debugBuild;
}

export const nativeExecutable = process.env.ATH_NATIVE_EXECUTABLE;

if (!existsSync(nativeExecutable)) {
  throw new Error(
    `The native ath executable is missing at ${nativeExecutable}. Run cargo build in native/ or set ATH_NATIVE_EXECUTABLE.`,
  );
}

/** A new private folder under the real temporary folder. */
export function temporaryFolder(prefix = "ath-npm-"): string {
  return mkdtempSync(join(realpathSync(tmpdir()), prefix));
}

export function removeFolder(path: string): void {
  rmSync(path, { recursive: true, force: true });
}

export const athlete = { author_type: "athlete" } as const;

/** A valid athlete note. */
export function noteData(reportedAt = "2026-01-02T03:04:05.678Z"): Record<string, unknown> {
  return {
    report_type: "note",
    reported_at: reportedAt,
    input_reported_at: {
      timestamp_missing_reason: "not_provided_by_source",
      utc_offset_missing_reason: "not_provided_by_source",
    },
    reported_via: "text",
  };
}
