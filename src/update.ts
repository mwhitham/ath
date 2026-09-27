/**
 * `ath --update` reinstalls a global ath, then replaces a skill already in the
 * folder you ran it from (D81).
 *
 * The program and the skill are different things. The program is installed once,
 * with npm, pnpm, or bun. The skill is written only into the folder where you
 * run `ath init`. This command updates the program with the same tool that
 * installed it, and replaces the skill only when one is already there.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readlinkSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { installSkill, skillStatus } from "./skill.js";

const PACKAGE_NAME = "athleticstandard";
const REGISTRY_LATEST = "https://registry.npmjs.org/athleticstandard/latest";

export interface InstallCommand {
  kind: "npm" | "pnpm" | "bun";
  command: string;
  args: string[];
}

export interface FetchedCopy {
  kind: "npx" | "pnpm-dlx" | "bunx";
  message: string;
}

export interface LocalCopy {
  kind: "local";
  message: string;
}

export type InstallClass = InstallCommand | FetchedCopy | LocalCopy;

const LOCAL_MESSAGE = "This ath was not installed globally, so nothing was changed.";

const COMMANDS: Record<InstallCommand["kind"], { command: string; args: string[] }> = {
  npm: { command: "npm", args: ["install", "-g", `${PACKAGE_NAME}@latest`] },
  pnpm: { command: "pnpm", args: ["add", "-g", `${PACKAGE_NAME}@latest`] },
  bun: { command: "bun", args: ["add", "-g", `${PACKAGE_NAME}@latest`] },
};

function normalize(path: string): string {
  return path.replace(/\\/g, "/");
}

function globalInstall(kind: InstallCommand["kind"]): InstallCommand {
  const spec = COMMANDS[kind];
  return { kind, command: spec.command, args: spec.args };
}

/**
 * Which kind of install a path is.
 *
 * The markers are the directories those tools actually use. A path that matches
 * none of them is left alone: guessing npm would install a second copy.
 */
export function classifyInstallPath(packagePath: string): InstallClass {
  const path = normalize(packagePath);

  if (path.includes("/_npx/")) {
    return {
      kind: "npx",
      message: "This ath was started with npx, which fetches the current version.",
    };
  }
  if (path.includes("/pnpm/dlx/") || path.includes("/pnpm-dlx/")) {
    return {
      kind: "pnpm-dlx",
      message: "This ath was started with pnpm dlx, which fetches the current version.",
    };
  }
  if (path.includes("/bunx/") || path.includes("/.bun/install/cache/")) {
    return {
      kind: "bunx",
      message: "This ath was started with bunx, which fetches the current version.",
    };
  }
  if (path.includes("/pnpm/global/")) return globalInstall("pnpm");
  if (path.includes("/.bun/install/global/")) return globalInstall("bun");
  if (
    path.endsWith("/lib/node_modules/athleticstandard") ||
    path.includes("/lib/node_modules/athleticstandard/") ||
    path.endsWith("/npm/node_modules/athleticstandard") ||
    path.includes("/npm/node_modules/athleticstandard/")
  ) {
    return globalInstall("npm");
  }
  return { kind: "local", message: LOCAL_MESSAGE };
}

/** A global install outranks a fetched copy, which outranks a project copy. */
function rank(found: InstallClass): number {
  if (found.kind === "npm" || found.kind === "pnpm" || found.kind === "bun") return 3;
  if (found.kind === "local") return 1;
  return 2;
}

/**
 * Pick one classification from every path that might be this process.
 *
 * pnpm and bun put the real files in a store and leave a symlink where you
 * typed the command. The store path does not say "global". The symlink does.
 * Checking both keeps a store path from hiding the global install.
 */
export function chooseInstall(paths: string[]): InstallClass {
  let best: InstallClass = { kind: "local", message: LOCAL_MESSAGE };
  for (const path of paths) {
    if (!path) continue;
    const found = classifyInstallPath(path);
    if (rank(found) > rank(best)) best = found;
  }
  return best;
}

/** Walk up to the athleticstandard package that holds `start`, if there is one. */
export function findPackageRoot(start: string): string | null {
  let dir = start;
  for (let i = 0; i < 12; i++) {
    const pkgPath = join(dir, "package.json");
    if (existsSync(pkgPath)) {
      try {
        const pkg = JSON.parse(readFileSync(pkgPath, "utf8")) as { name?: unknown };
        if (pkg.name === PACKAGE_NAME) return dir;
      } catch {
        // A broken package.json is not this package. Keep walking.
      }
    }
    const parent = dirname(dir);
    if (parent === dir) return null;
    dir = parent;
  }
  return null;
}

/**
 * Every path worth classifying for the process that is running.
 *
 * Includes the script path, where it points if it is a symlink, the real path,
 * and the package directory that holds each of those. A bin symlink such as
 * `/usr/local/bin/ath` does not itself say how it was installed. The target does.
 */
export function candidatePaths(scriptPath: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  const queue: string[] = [];
  const push = (path: string) => {
    if (!path) return;
    const normal = normalize(path);
    if (seen.has(normal)) return;
    seen.add(normal);
    queue.push(path);
    out.push(normal);
  };

  push(scriptPath);
  push(fileURLToPath(import.meta.url));

  while (queue.length > 0) {
    const path = queue.shift()!;
    try {
      push(resolve(dirname(path), readlinkSync(path)));
    } catch {
      // Not a symlink.
    }
    try {
      push(realpathSync(path));
    } catch {
      // The path is not on disk. Classification still uses the path as given.
    }
    const root = findPackageRoot(path);
    if (root) push(root);
  }
  return out;
}

export function compareVersions(left: string, right: string): number {
  const a = left.split(".").map(Number);
  const b = right.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    const diff = (a[i] ?? 0) - (b[i] ?? 0);
    if (diff !== 0) return diff > 0 ? 1 : -1;
  }
  return 0;
}

/** One line naming the skill directories that were written. */
export function skillUpdateLine(cwd: string, written: string[]): string {
  const dirs = written.map((writtenPath) => {
    const rel = writtenPath.slice(cwd.length).replace(/^[\\/]+/, "");
    const agent = rel.split(/[\\/]/)[0] ?? rel;
    return join(agent, "skills");
  });
  if (dirs.length === 0) return "";
  if (dirs.length === 1) return `updated the skill in ${dirs[0]}`;
  const last = dirs[dirs.length - 1]!;
  return `updated the skill in ${dirs.slice(0, -1).join(", ")} and ${last}`;
}

export type Spawn = (command: string, args: string[]) => number;

export interface UpdateInput {
  packagePaths: string[];
  currentVersion: string;
  cwd: string;
  fetchLatest?: () => Promise<string>;
  spawn?: Spawn;
  log?: (line: string) => void;
  error?: (line: string) => void;
  skillPresent?: (cwd: string) => boolean;
  installSkill?: (cwd: string) => string[];
}

function defaultSpawn(command: string, args: string[]): number {
  const bin = process.platform === "win32" ? `${command}.cmd` : command;
  const result = spawnSync(bin, args, { stdio: "inherit" });
  if (result.error) return 1;
  return result.status ?? 1;
}

async function defaultFetchLatest(): Promise<string> {
  const response = await fetch(REGISTRY_LATEST, {
    headers: { accept: "application/json" },
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) throw new Error(`registry status ${response.status}`);
  const body = (await response.json()) as { version?: unknown };
  if (typeof body.version !== "string") throw new Error("registry version");
  return body.version;
}

function defaultSkillPresent(cwd: string): boolean {
  return skillStatus(cwd).some((copy) => copy.state !== "missing");
}

/**
 * Reinstall this ath when it is a global install, then replace a skill that is
 * already in `cwd`. Returns a process exit code.
 */
export async function runUpdate(input: UpdateInput): Promise<number> {
  const log = input.log ?? ((line) => console.log(line));
  const error = input.error ?? ((line) => console.error(line));
  const found = chooseInstall(input.packagePaths);

  if (found.kind === "local") {
    error(`ath: ${found.message}`);
    return 1;
  }

  if (found.kind === "npm" || found.kind === "pnpm" || found.kind === "bun") {
    let latest: string;
    try {
      latest = await (input.fetchLatest ?? defaultFetchLatest)();
    } catch {
      error("ath: the npm registry could not be reached, so nothing was changed.");
      return 1;
    }
    if (!/^\d+\.\d+\.\d+$/.test(latest)) {
      error("ath: the npm registry did not return a version, so nothing was changed.");
      return 1;
    }
    const cmp = compareVersions(input.currentVersion, latest);
    if (cmp === 0) {
      log(`ath ${input.currentVersion} is the current version.`);
    } else if (cmp > 0) {
      log(`This ath is ${input.currentVersion}, which is ahead of the published ${latest}.`);
    } else {
      const code = (input.spawn ?? defaultSpawn)(found.command, found.args);
      if (code !== 0) {
        error("ath: the install failed.");
        return 1;
      }
      log(`updated ath from ${input.currentVersion} to ${latest}.`);
    }
  } else if (found.kind === "npx" || found.kind === "pnpm-dlx" || found.kind === "bunx") {
    log(found.message);
  }

  // After the installer returns, the skill files on disk are the version just
  // installed. Copying them here picks up that version, not the one this
  // process loaded at startup.
  const present = (input.skillPresent ?? defaultSkillPresent)(input.cwd);
  if (!present) return 0;
  const written = (input.installSkill ?? installSkill)(input.cwd);
  const line = skillUpdateLine(input.cwd, written);
  if (line) log(line);
  return 0;
}
