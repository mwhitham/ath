/**
 * Find the native `ath` executable packed in this package.
 *
 * The tarball carries one prebuilt executable per supported target under
 * `native/<platform>-<arch>/`. Nothing is downloaded or compiled at install
 * or run time. An unsupported target fails with AthInstallationError.
 *
 * ATH_NATIVE_EXECUTABLE names another executable. Source checkouts and tests
 * use it to run a binary built with Cargo. It is never set by this package.
 */
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** The targets this package ships, as `${process.platform}-${process.arch}`. */
export const SUPPORTED_NATIVE_TARGETS = ["linux-x64", "darwin-arm64", "win32-x64"] as const;

export type NativeTarget = (typeof SUPPORTED_NATIVE_TARGETS)[number];

/** The environment variable that names a different native executable. */
export const NATIVE_EXECUTABLE_VARIABLE = "ATH_NATIVE_EXECUTABLE";

/** The installed package cannot run on this system or is missing its executable. */
export class AthInstallationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AthInstallationError";
  }
}

/** The package directory: the folder that holds package.json, dist/, and native/. */
export function packageRoot(): string {
  // This file is dist/native-binary.js in the package, or src/native-binary.ts in a checkout.
  return resolve(dirname(fileURLToPath(import.meta.url)), "..");
}

/** The packed target for a platform and architecture, or null when none is shipped. */
export function nativeTarget(
  platform: string = process.platform,
  arch: string = process.arch,
): NativeTarget | null {
  const name = `${platform}-${arch}`;
  return (SUPPORTED_NATIVE_TARGETS as readonly string[]).includes(name)
    ? (name as NativeTarget)
    : null;
}

/** The executable file name for a target. */
export function nativeExecutableName(target: NativeTarget): string {
  return target.startsWith("win32-") ? "ath.exe" : "ath";
}

/**
 * The native executable to run. Throws AthInstallationError when this system
 * has no packed executable or the packed file is missing.
 */
export function resolveNativeExecutable(
  env: NodeJS.ProcessEnv = process.env,
  platform: string = process.platform,
  arch: string = process.arch,
  root: string = packageRoot(),
): string {
  const override = env[NATIVE_EXECUTABLE_VARIABLE];
  if (override !== undefined && override !== "") {
    return override;
  }
  const target = nativeTarget(platform, arch);
  if (target === null) {
    throw new AthInstallationError(
      `athleticstandard has no native ath for ${platform}-${arch}. ` +
        `Supported systems: ${SUPPORTED_NATIVE_TARGETS.join(", ")}. ` +
        "Nothing was downloaded or compiled.",
    );
  }
  const path = join(root, "native", target, nativeExecutableName(target));
  if (!existsSync(path)) {
    throw new AthInstallationError(
      `athleticstandard is missing its native ath for ${target} at ${path}. ` +
        "Reinstall the package; nothing was downloaded or compiled.",
    );
  }
  return path;
}
