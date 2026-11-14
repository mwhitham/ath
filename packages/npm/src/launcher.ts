/**
 * The `ath` command that npm, pnpm, and bun install.
 *
 * The launcher runs the native executable packed in this package. It passes the
 * same arguments, standard input, output, error, and environment, forwards
 * termination signals, and exits with the native status. It holds no command of
 * its own. Nothing is downloaded or compiled.
 *
 * `ath --update` ends with a handoff. A running executable cannot be replaced on
 * Windows, so the native program writes an update plan and exits. The launcher
 * checks the plan, runs the package manager it names without a shell command
 * string, and then runs the newly installed executable to refresh skills. The
 * same route is used on every system. native/src/runtime/update.rs defines the
 * plan and the environment variables.
 */
import { spawn, type ChildProcess } from "node:child_process";
import {
  chmodSync,
  lstatSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
} from "node:fs";
import { constants, tmpdir } from "node:os";
import { dirname, extname, isAbsolute, join } from "node:path";
import { AthInstallationError, packageRoot, resolveNativeExecutable } from "./native-binary.js";

/** Launcher and module paths for the native program to classify. */
export const INSTALL_PATHS_VARIABLE = "ATH_INSTALL_PATHS";

/** The file the native program writes an update plan to. */
export const HANDOFF_VARIABLE = "ATH_UPDATE_HANDOFF";

/** Which part of an update a native run performs. */
export const PHASE_VARIABLE = "ATH_UPDATE_PHASE";

/** The `ATH_UPDATE_PHASE` value for the run after a successful install. */
export const REFRESH_PHASE = "refresh-skills";

/** The update plan format this launcher reads. */
export const PLAN_FORMAT = 1;

const PACKAGE_NAME = "athleticstandard";
const PLAN_FILE_NAME = "update-plan.json";

// A plan is a few hundred bytes. A larger file is not a plan this ath wrote.
const PLAN_SIZE_LIMIT = 64 * 1024;

export type PackageManager = "npm" | "pnpm" | "bun";

/** The exact install arguments for each package manager. */
export const MANAGER_ARGS: Readonly<Record<PackageManager, readonly string[]>> = {
  npm: ["install", "-g", "athleticstandard@latest"],
  pnpm: ["add", "-g", "athleticstandard@latest"],
  bun: ["add", "-g", "athleticstandard@latest"],
};

/** The checked update plan. Field names match the native `UpdatePlan`. */
export interface UpdatePlan {
  ath_update_plan: typeof PLAN_FORMAT;
  manager: PackageManager;
  program: PackageManager;
  args: string[];
  current_version: string;
  latest_version: string;
  success_message: string;
  failure_message: string;
  refresh_args: string[];
  refresh_environment: Record<string, string>;
}

const PLAN_KEYS = [
  "ath_update_plan",
  "manager",
  "program",
  "args",
  "current_version",
  "latest_version",
  "success_message",
  "failure_message",
  "refresh_args",
  "refresh_environment",
] as const;

const REFRESH_ARGS = ["--update"] as const;

/** An update plan that is malformed or not one this ath writes. Nothing was run. */
export class UpdatePlanError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "UpdatePlanError";
  }
}

/** A failure the launcher reports before or instead of running a program. */
export class LauncherError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "LauncherError";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function sameStrings(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((item, index) => item === right[index]);
}

function isReleaseVersion(text: string): boolean {
  return /^\d+\.\d+\.\d+$/.test(text);
}

/**
 * Check a parsed plan the way the native `UpdatePlan::check` does. Every field
 * is required, no other field is allowed, and the program, arguments, refresh
 * arguments, and refresh environment must be the fixed values for the named
 * manager. The plan reaches no program until it passes.
 */
export function checkUpdatePlan(value: unknown): UpdatePlan {
  const invalid = (detail: string) =>
    new UpdatePlanError(`the update plan is not valid: ${detail}; nothing was installed`);
  if (!isRecord(value)) {
    throw invalid("expected a JSON object");
  }
  for (const key of Object.keys(value)) {
    if (!(PLAN_KEYS as readonly string[]).includes(key)) {
      throw invalid(`unknown field ${JSON.stringify(key)}`);
    }
  }
  for (const key of PLAN_KEYS) {
    if (!Object.hasOwn(value, key)) {
      throw invalid(`missing field ${JSON.stringify(key)}`);
    }
  }
  const format = value.ath_update_plan;
  if (typeof format !== "number" || !Number.isInteger(format) || format < 0) {
    throw invalid("ath_update_plan is not a whole number");
  }
  const manager = value.manager;
  if (manager !== "npm" && manager !== "pnpm" && manager !== "bun") {
    throw invalid("manager is not npm, pnpm, or bun");
  }
  for (const key of [
    "program",
    "current_version",
    "latest_version",
    "success_message",
    "failure_message",
  ] as const) {
    if (typeof value[key] !== "string") {
      throw invalid(`${key} is not a string`);
    }
  }
  for (const key of ["args", "refresh_args"] as const) {
    if (!isStringArray(value[key])) {
      throw invalid(`${key} is not a list of strings`);
    }
  }
  const environment = value.refresh_environment;
  if (!isRecord(environment) || !Object.values(environment).every((item) => typeof item === "string")) {
    throw invalid("refresh_environment is not a map of strings");
  }

  const program = value.program as string;
  const args = value.args as string[];
  const refreshArgs = value.refresh_args as string[];
  const currentVersion = value.current_version as string;
  const latestVersion = value.latest_version as string;
  const environmentKeys = Object.keys(environment);
  if (
    format !== PLAN_FORMAT ||
    program !== manager ||
    !sameStrings(args, MANAGER_ARGS[manager]) ||
    !sameStrings(refreshArgs, REFRESH_ARGS) ||
    environmentKeys.length !== 1 ||
    environment[PHASE_VARIABLE] !== REFRESH_PHASE ||
    !isReleaseVersion(currentVersion) ||
    !isReleaseVersion(latestVersion)
  ) {
    throw new UpdatePlanError("the update plan is not one this ath writes; nothing was installed");
  }
  return {
    ath_update_plan: PLAN_FORMAT,
    manager,
    program: manager,
    args: [...MANAGER_ARGS[manager]],
    current_version: currentVersion,
    latest_version: latestVersion,
    success_message: value.success_message as string,
    failure_message: value.failure_message as string,
    refresh_args: [...REFRESH_ARGS],
    refresh_environment: { [PHASE_VARIABLE]: REFRESH_PHASE },
  };
}

/** Parse and check plan text. */
export function parseUpdatePlan(text: string): UpdatePlan {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (error) {
    throw new UpdatePlanError(
      `the update plan is not valid: ${(error as Error).message}; nothing was installed`,
    );
  }
  return checkUpdatePlan(value);
}

/**
 * Read the plan file, or return null when the native program wrote none. Only
 * a small regular file counts. A link or a folder is refused.
 */
export function readUpdatePlan(path: string): UpdatePlan | null {
  let entry;
  try {
    entry = lstatSync(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return null;
    }
    throw error;
  }
  if (!entry.isFile()) {
    throw new UpdatePlanError("the update plan is not a regular file; nothing was installed");
  }
  if (entry.size > PLAN_SIZE_LIMIT) {
    throw new UpdatePlanError("the update plan is too large; nothing was installed");
  }
  return parseUpdatePlan(readFileSync(path, "utf8"));
}

/** True when the arguments ask for `ath --update`. Arguments after `--` are data. */
export function isUpdateRun(args: readonly string[]): boolean {
  for (const arg of args) {
    if (arg === "--") {
      return false;
    }
    if (arg === "--update") {
      return true;
    }
  }
  return false;
}

// Windows environment names ignore case. A copied environment object does not.
function sameVariable(name: string, wanted: string, platform: string): boolean {
  return platform === "win32" ? name.toUpperCase() === wanted.toUpperCase() : name === wanted;
}

/** Read an environment variable, ignoring case on Windows. */
export function environmentValue(
  env: NodeJS.ProcessEnv,
  name: string,
  platform: string = process.platform,
): string | undefined {
  for (const [key, value] of Object.entries(env)) {
    if (sameVariable(key, name, platform) && value !== undefined) {
      return value;
    }
  }
  return undefined;
}

/**
 * The environment for a native run. The launcher alone sets the update
 * variables, so values inherited from the caller are removed. The install
 * paths are the launcher script as started and this module's own file.
 */
export function nativeEnvironment(
  env: NodeJS.ProcessEnv,
  scriptPath: string | undefined,
  modulePath: string,
  platform: string = process.platform,
): NodeJS.ProcessEnv {
  const out: NodeJS.ProcessEnv = {};
  for (const [key, value] of Object.entries(env)) {
    const launcherOwned = [INSTALL_PATHS_VARIABLE, HANDOFF_VARIABLE, PHASE_VARIABLE].some((name) =>
      sameVariable(key, name, platform),
    );
    if (!launcherOwned) {
      out[key] = value;
    }
  }
  out[INSTALL_PATHS_VARIABLE] = [scriptPath, modulePath]
    .filter((path): path is string => path !== undefined && path !== "")
    .join(platform === "win32" ? ";" : ":");
  return out;
}

// Only these can run without a shell: programs directly, batch files through cmd.exe.
const RUNNABLE_EXTENSIONS = [".com", ".exe", ".bat", ".cmd"];

function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/**
 * Find a program on a Windows PATH, trying each PATHEXT extension in order.
 * Only absolute PATH entries are searched, never the working folder, so a file
 * in the folder where `ath` runs cannot stand in for the package manager.
 */
export function findWindowsProgram(name: string, env: NodeJS.ProcessEnv): string | null {
  const pathValue = environmentValue(env, "PATH", "win32") ?? "";
  const extensions = (environmentValue(env, "PATHEXT", "win32") ?? ".COM;.EXE;.BAT;.CMD")
    .split(";")
    .map((extension) => extension.trim().toLowerCase())
    .filter((extension) => RUNNABLE_EXTENSIONS.includes(extension));
  for (const entry of pathValue.split(";")) {
    const directory = entry.trim().replace(/^"(.*)"$/, "$1");
    if (directory === "" || !isAbsolute(directory)) {
      continue;
    }
    for (const extension of extensions) {
      const candidate = join(directory, `${name}${extension}`);
      if (isFile(candidate)) {
        return candidate;
      }
    }
  }
  return null;
}

// cmd.exe treats these as syntax. Each is escaped with a caret.
const CMD_SYNTAX = /[ !%^&()<>|"]/g;

/**
 * Quote one argument for a batch file run as `cmd.exe /d /s /c "..."`.
 *
 * The argument is first quoted for the program the batch file starts, using the
 * Microsoft C runtime rules. Every cmd.exe syntax character is then escaped
 * twice: once for the cmd.exe command line and once for the batch file's `%*`
 * expansion. A line break cannot be passed through cmd.exe and is refused.
 */
export function quoteBatchArgument(argument: string): string {
  if (/[\r\n\0]/.test(argument)) {
    throw new LauncherError("an argument with a line break cannot be passed to a batch file");
  }
  let quoted: string;
  if (argument === "") {
    quoted = '""';
  } else if (!/[ \t\v"]/.test(argument)) {
    quoted = argument;
  } else {
    quoted = '"';
    let backslashes = 0;
    for (const character of argument) {
      if (character === "\\") {
        backslashes += 1;
        continue;
      }
      if (character === '"') {
        quoted += "\\".repeat(backslashes * 2 + 1) + '"';
      } else {
        quoted += "\\".repeat(backslashes) + character;
      }
      backslashes = 0;
    }
    quoted += "\\".repeat(backslashes * 2) + '"';
  }
  return quoted.replace(CMD_SYNTAX, "^$&").replace(CMD_SYNTAX, "^$&");
}

/** How to start a program: the file, its arguments, and whether they are passed as written. */
export interface Invocation {
  file: string;
  args: string[];
  windowsVerbatimArguments: boolean;
}

/**
 * Run a Windows batch file without a shell string built from its arguments.
 *
 * Node refuses to start a `.cmd` or `.bat` file directly, so cmd.exe runs it.
 * `/d` skips AutoRun commands, `/v:off` turns off `!` expansion, and `/s` makes
 * cmd.exe drop only the outer quotes. The file path is quoted as a whole. A
 * path holding `%` or `"` cannot be quoted safely and is refused.
 */
export function batchInvocation(file: string, args: readonly string[], comspec: string): Invocation {
  if (/[%"\r\n\0]/.test(file)) {
    throw new LauncherError(`cannot run ${file} safely through cmd.exe`);
  }
  const line = [`"${file}"`, ...args.map(quoteBatchArgument)].join(" ");
  return {
    file: comspec,
    args: ["/d", "/v:off", "/s", "/c", `"${line}"`],
    windowsVerbatimArguments: true,
  };
}

/** How to run a package manager by name with fixed arguments. */
export function managerInvocation(
  program: string,
  args: readonly string[],
  env: NodeJS.ProcessEnv,
  platform: string = process.platform,
): Invocation {
  if (platform !== "win32") {
    return { file: program, args: [...args], windowsVerbatimArguments: false };
  }
  const found = findWindowsProgram(program, env);
  if (found === null) {
    throw new LauncherError(`${program} was not found on PATH`);
  }
  const extension = extname(found).toLowerCase();
  if (extension === ".cmd" || extension === ".bat") {
    const systemRoot = environmentValue(env, "SystemRoot", "win32") ?? "C:\\Windows";
    const comspec = environmentValue(env, "ComSpec", "win32") ?? join(systemRoot, "System32", "cmd.exe");
    return batchInvocation(found, args, comspec);
  }
  return { file: found, args: [...args], windowsVerbatimArguments: false };
}

/** Walk up from `start` to the folder whose package.json names this package. */
export function findPackageRoot(start: string): string | null {
  let directory = start;
  for (let depth = 0; depth < 12; depth += 1) {
    try {
      const manifest = JSON.parse(readFileSync(join(directory, "package.json"), "utf8")) as {
        name?: unknown;
      };
      if (manifest.name === PACKAGE_NAME) {
        return directory;
      }
    } catch {
      // No package.json here, or not one this package wrote. Keep walking.
    }
    const parent = dirname(directory);
    if (parent === directory) {
      return null;
    }
    directory = parent;
  }
  return null;
}

/**
 * The native executable after an install. A package manager may replace the
 * package folder or point the command link at a new one, so the command path
 * as started is followed again first. This module's own folder comes next.
 */
export function installedExecutable(env: NodeJS.ProcessEnv, scriptPath: string | undefined): string {
  const roots: string[] = [];
  if (scriptPath !== undefined && scriptPath !== "") {
    try {
      const root = findPackageRoot(dirname(realpathSync(scriptPath)));
      if (root !== null) {
        roots.push(root);
      }
    } catch {
      // The command path no longer exists. Try this module's folder.
    }
  }
  roots.push(packageRoot());
  let failure: unknown = new AthInstallationError("athleticstandard could not find its native ath.");
  for (const root of roots) {
    try {
      return resolveNativeExecutable(env, process.platform, process.arch, root);
    } catch (error) {
      failure = error;
    }
  }
  throw failure;
}

// On Windows the console sends Ctrl-C, Ctrl-Break, and close events to every
// process attached to it, so the child already has them. Listening keeps the
// launcher alive until the child exits.
const RELAYED_SIGNALS: readonly NodeJS.Signals[] =
  process.platform === "win32" ? ["SIGINT", "SIGBREAK", "SIGHUP"] : ["SIGINT", "SIGTERM", "SIGHUP"];

/** Passes termination signals to the running child and remembers that one came. */
class SignalRelay {
  child: ChildProcess | null = null;
  received: NodeJS.Signals | null = null;

  private readonly listener = (signal: NodeJS.Signals): void => {
    this.received = signal;
    const child = this.child;
    if (process.platform === "win32" || child === null) {
      return;
    }
    if (child.exitCode === null && child.signalCode === null) {
      try {
        child.kill(signal);
      } catch {
        // The child exited between the check and the kill.
      }
    }
  };

  install(): void {
    for (const signal of RELAYED_SIGNALS) {
      process.on(signal, this.listener);
    }
  }

  uninstall(): void {
    for (const signal of RELAYED_SIGNALS) {
      process.off(signal, this.listener);
    }
  }
}

interface ChildEnd {
  code: number | null;
  signal: NodeJS.Signals | null;
  error?: Error;
}

function runChild(
  file: string,
  args: readonly string[],
  env: NodeJS.ProcessEnv,
  relay: SignalRelay,
  windowsVerbatimArguments = false,
): Promise<ChildEnd> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (end: ChildEnd): void => {
      if (!settled) {
        settled = true;
        relay.child = null;
        resolve(end);
      }
    };
    let child: ChildProcess;
    try {
      child = spawn(file, [...args], {
        stdio: "inherit",
        shell: false,
        env,
        windowsVerbatimArguments,
      });
    } catch (error) {
      finish({ code: null, signal: null, error: error as Error });
      return;
    }
    relay.child = child;
    child.on("error", (error) => {
      // A child that started reports its end through "exit". An error without
      // a process id means it never started.
      if (child.pid === undefined) {
        finish({ code: null, signal: null, error });
      }
    });
    child.once("exit", (code, signal) => finish({ code, signal }));
  });
}

/** How the launcher ends: an exit status, or the signal that ended the child. */
export type LauncherExit = { code: number } | { signal: NodeJS.Signals };

function say(stream: NodeJS.WriteStream, line: string): void {
  stream.write(line.endsWith("\n") ? line : `${line}\n`);
}

function endOf(end: ChildEnd, file: string): LauncherExit {
  if (end.error !== undefined) {
    say(process.stderr, `ath: could not run ${file}: ${end.error.message}`);
    return { code: 1 };
  }
  if (end.signal !== null) {
    return { signal: end.signal };
  }
  return { code: end.code ?? 1 };
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function runUpdate(
  executable: string,
  args: readonly string[],
  env: NodeJS.ProcessEnv,
  native: NodeJS.ProcessEnv,
  scriptPath: string | undefined,
  relay: SignalRelay,
): Promise<LauncherExit> {
  const directory = mkdtempSync(join(realpathSync(tmpdir()), "ath-update-"));
  try {
    chmodSync(directory, 0o700);
    const handoff = join(directory, PLAN_FILE_NAME);
    const checkRun = await runChild(executable, args, { ...native, [HANDOFF_VARIABLE]: handoff }, relay);
    // A plan counts only after a clean exit. A failed or interrupted run
    // ends here with its own status.
    if (checkRun.error !== undefined || checkRun.signal !== null || checkRun.code !== 0) {
      return endOf(checkRun, executable);
    }
    let plan: UpdatePlan | null;
    try {
      plan = readUpdatePlan(handoff);
    } catch (error) {
      say(process.stderr, `ath: ${errorText(error)}`);
      return { code: 1 };
    }
    if (plan === null) {
      return { code: 0 };
    }

    let manager: Invocation;
    try {
      if (relay.received !== null) {
        throw new LauncherError(`the update was cancelled by ${relay.received}`);
      }
      manager = managerInvocation(plan.program, plan.args, env);
    } catch (error) {
      say(process.stderr, `ath: ${errorText(error)}`);
      say(process.stderr, plan.failure_message);
      return { code: 1 };
    }
    const install = await runChild(
      manager.file,
      manager.args,
      env,
      relay,
      manager.windowsVerbatimArguments,
    );
    if (install.error !== undefined) {
      say(process.stderr, `ath: could not run ${plan.program}: ${install.error.message}`);
    } else if (install.signal !== null) {
      say(process.stderr, `ath: ${plan.program} was stopped by ${install.signal}`);
    } else if (relay.received !== null) {
      say(process.stderr, `ath: the update was cancelled by ${relay.received}`);
    }
    if (install.error !== undefined || install.signal !== null || install.code !== 0 || relay.received !== null) {
      say(process.stderr, plan.failure_message);
      return { code: 1 };
    }
    say(process.stdout, plan.success_message);

    let refreshed: string;
    try {
      refreshed = installedExecutable(env, scriptPath);
    } catch (error) {
      say(process.stderr, `ath: ${errorText(error)}`);
      return { code: 1 };
    }
    const refresh = await runChild(refreshed, plan.refresh_args, { ...native, ...plan.refresh_environment }, relay);
    return endOf(refresh, refreshed);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

/**
 * Run `ath` with `args` and return how the launcher should end. `scriptPath`
 * is the command path as started (`process.argv[1]`), and `modulePath` is the
 * launcher module's own file.
 */
export async function main(
  args: readonly string[],
  scriptPath: string | undefined,
  modulePath: string,
  env: NodeJS.ProcessEnv = process.env,
): Promise<LauncherExit> {
  const relay = new SignalRelay();
  relay.install();
  try {
    let executable: string;
    try {
      executable = resolveNativeExecutable(env);
    } catch (error) {
      if (error instanceof AthInstallationError) {
        say(process.stderr, `ath: ${error.message}`);
        return { code: 1 };
      }
      throw error;
    }
    const native = nativeEnvironment(env, scriptPath, modulePath);
    if (isUpdateRun(args)) {
      return await runUpdate(executable, args, env, native, scriptPath, relay);
    }
    return endOf(await runChild(executable, args, native, relay), executable);
  } finally {
    relay.uninstall();
  }
}

/**
 * End this process the way the child ended. A signal is raised again on this
 * process so the caller sees it. Where that is not possible, the status is
 * 128 plus the signal number, as shells report it.
 */
export function exitLike(exit: LauncherExit): void {
  if ("code" in exit) {
    process.exitCode = exit.code;
    return;
  }
  const number = constants.signals[exit.signal] ?? 0;
  process.exitCode = 128 + number;
  if (process.platform !== "win32") {
    try {
      process.kill(process.pid, exit.signal);
    } catch {
      // The signal could not be raised. The exit status stands.
    }
  }
}
