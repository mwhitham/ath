#!/usr/bin/env node
/**
 * Install the packed athleticstandard tarball into an empty folder and use it
 * the way a user would. No compiler, Rust toolchain, or sqlite3 is on PATH, and
 * npm runs no install scripts because the package has none.
 *
 * Usage:
 *
 *   node packages/npm/scripts/smoke.mjs <tarball> [--offline-deps <node_modules>] [--keep]
 *
 * Without --offline-deps, npm fetches the runtime dependencies (zod) from the
 * registry. With it, each runtime dependency is packed from that installed
 * node_modules folder and npm installs everything with --offline, so a local
 * run needs no network. --keep leaves the temporary folder for inspection.
 *
 * Steps:
 *
 * 1. Make an empty project with a minimal package.json.
 * 2. Build a PATH holding only Node and system basics, and check that cargo,
 *    rustc, cc, gcc, clang, cl, and sqlite3 do not resolve on it.
 * 3. npm install the tarball. The installed package must have no install
 *    scripts, and its native executable must equal the packed one.
 * 4. `npx --no -- ath --version` prints the package version.
 * 5. The installed `ath --help` exits 0.
 * 6. smoke-library.mjs imports every export, parses a record with Zod, and
 *    creates, writes, reads, checks, and closes a database.
 *
 * Every step after the install runs even if an earlier one failed. The exit
 * status is 1 when any step failed.
 */
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import {
  environmentValue,
  findOnPath,
  npmScripts,
  packedManifest,
  readTarball,
  sha256,
  TARGETS,
} from "./package-tools.mjs";

const FORBIDDEN_TOOLS = ["cargo", "rustc", "cc", "gcc", "clang", "cl", "sqlite3"];
const windows = process.platform === "win32";

function usage(message) {
  console.error(`smoke: ${message}`);
  console.error("usage: node packages/npm/scripts/smoke.mjs <tarball> [--offline-deps <node_modules>] [--keep]");
  process.exit(2);
}

let parsed;
try {
  parsed = parseArgs({
    options: {
      "offline-deps": { type: "string" },
      keep: { type: "boolean", default: false },
    },
    allowPositionals: true,
    strict: true,
  });
} catch (error) {
  usage(error.message);
}
if (parsed.positionals.length !== 1) {
  usage("give exactly one tarball");
}
const tarball = resolve(parsed.positionals[0]);
if (!existsSync(tarball)) {
  usage(`${tarball} does not exist`);
}
const offlineDeps = parsed.values["offline-deps"] === undefined ? null : resolve(parsed.values["offline-deps"]);

const entries = readTarball(tarball);
const manifest = packedManifest(entries);
const hostTarget = TARGETS.find((item) => item.target === `${process.platform}-${process.arch}`);
if (hostTarget === undefined) {
  usage(`this system (${process.platform}-${process.arch}) is not a packed target`);
}
const { npm, npx } = npmScripts();

const results = [];
function record(step, ok, detail) {
  results.push({ step, ok, detail });
  console.log(`${ok ? "ok    " : "FAILED"} ${step}${detail ? `: ${detail}` : ""}`);
}

const work = mkdtempSync(join(realpathSync(tmpdir()), "ath-smoke-"));
const project = join(work, "project");
const tools = join(work, "tools");
const dependencies = join(work, "dependencies");
for (const directory of [project, tools, dependencies]) {
  mkdirSync(directory);
}

/**
 * PATH for every step. On Linux and macOS it is one folder of links to Node
 * and sh, because the folders that hold Node often hold compilers too. On
 * Windows it is Node's folder and System32; npm reaches cmd.exe through ComSpec.
 */
function cleanPath() {
  if (windows) {
    const systemRoot = environmentValue(process.env, "SystemRoot") ?? "C:\\Windows";
    return [dirname(process.execPath), join(systemRoot, "System32")].join(";");
  }
  symlinkSync(process.execPath, join(tools, "node"));
  symlinkSync("/bin/sh", join(tools, "sh"));
  return tools;
}

/**
 * The environment for every step: PATH replaced, the npm cache in the work
 * folder, and nothing that could point the launcher or npm elsewhere.
 */
function cleanEnvironment(path) {
  const env = {};
  for (const [key, value] of Object.entries(process.env)) {
    const name = key.toUpperCase();
    const dropped =
      name === "PATH" ||
      name.startsWith("ATH_") ||
      name.startsWith("NPM_") ||
      name === "NODE_OPTIONS" ||
      name === "NODE_PATH" ||
      name.startsWith("CARGO") ||
      name.startsWith("RUSTUP");
    if (!dropped) {
      env[key] = value;
    }
  }
  env.PATH = path;
  env.npm_config_cache = join(work, "npm-cache");
  env.npm_config_update_notifier = "false";
  env.npm_config_fund = "false";
  env.npm_config_audit = "false";
  return env;
}

function run(file, args, options = {}) {
  return spawnSync(file, args, { cwd: project, env, encoding: "utf8", timeout: 300_000, ...options });
}

function describe(result) {
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
  const end = result.error?.message ?? (result.signal !== null ? `signal ${result.signal}` : `status ${result.status}`);
  return output === "" ? end : `${end}\n${output}`;
}

const path = cleanPath();
const env = cleanEnvironment(path);
let exitCode = 0;

try {
  const found = FORBIDDEN_TOOLS.map((tool) => [tool, findOnPath(tool, path, environmentValue(process.env, "PATHEXT"))])
    .filter(([, location]) => location !== null)
    .map(([tool, location]) => `${tool} at ${location}`);
  if (found.length > 0) {
    throw new Error(`the smoke PATH reaches build tools: ${found.join(", ")}`);
  }
  record("PATH without build tools", true, path);

  writeFileSync(
    join(project, "package.json"),
    `${JSON.stringify({ name: "ath-smoke", version: "0.0.0", private: true, type: "module" }, null, 2)}\n`,
  );

  // Runtime dependencies come from the registry, or packed from a local folder.
  const installArgs = ["install", tarball, "--no-audit", "--no-fund"];
  if (offlineDeps !== null) {
    const queue = Object.keys(manifest.dependencies ?? {});
    const seen = new Set();
    while (queue.length > 0) {
      const name = queue.shift();
      if (seen.has(name)) continue;
      seen.add(name);
      const source = join(offlineDeps, name);
      if (!existsSync(join(source, "package.json"))) {
        throw new Error(`--offline-deps has no ${name} at ${source}`);
      }
      const packedDependency = run(process.execPath, [npm, "pack", source, "--json", "--ignore-scripts", "--pack-destination", dependencies]);
      if (packedDependency.status !== 0) {
        throw new Error(`could not pack ${name}: ${describe(packedDependency)}`);
      }
      installArgs.push(join(dependencies, JSON.parse(packedDependency.stdout)[0].filename));
      const dependencyManifest = JSON.parse(readFileSync(join(source, "package.json"), "utf8"));
      queue.push(...Object.keys(dependencyManifest.dependencies ?? {}));
    }
    installArgs.push("--offline");
  }
  const install = run(process.execPath, [npm, ...installArgs]);
  if (install.status !== 0) {
    throw new Error(`npm install failed: ${describe(install)}`);
  }
  record("npm install", true, `${manifest.name}@${manifest.version}${offlineDeps === null ? "" : " (offline)"}`);

  const installed = join(project, "node_modules", manifest.name);
  const installedManifest = JSON.parse(readFileSync(join(installed, "package.json"), "utf8"));
  const scripts = Object.keys(installedManifest.scripts ?? {}).filter((name) =>
    ["preinstall", "install", "postinstall", "prepare"].includes(name),
  );
  record("no install scripts", scripts.length === 0, scripts.join(", "));

  const nativePath = `native/${hostTarget.target}/${hostTarget.executable}`;
  const packedNative = entries.find((entry) => entry.path === `package/${nativePath}`);
  const installedNative = join(installed, nativePath);
  if (packedNative === undefined || !existsSync(installedNative)) {
    record("installed native executable", false, `${nativePath} is missing`);
  } else {
    const same = sha256(readFileSync(installedNative)) === sha256(packedNative.data);
    const executable = windows || (statSync(installedNative).mode & 0o111) !== 0;
    record(
      "installed native executable",
      same && executable,
      `${nativePath} ${sha256(packedNative.data)}${same ? "" : " differs from the tarball"}${executable ? "" : " is not executable"}`,
    );
  }

  const version = run(process.execPath, [npx, "--no", "--", "ath", "--version"]);
  record(
    "npx ath --version",
    version.status === 0 && version.stdout.trim() === manifest.version,
    version.status === 0 ? version.stdout.trim() : describe(version),
  );

  const help = windows
    ? run(
        environmentValue(process.env, "ComSpec") ?? "cmd.exe",
        ["/d", "/s", "/c", `""${join(project, "node_modules", ".bin", "ath.cmd")}" --help"`],
        { windowsVerbatimArguments: true },
      )
    : run(join(project, "node_modules", ".bin", "ath"), ["--help"]);
  record(
    "ath --help",
    help.status === 0 && help.stdout.trim() !== "",
    help.status === 0 ? `${help.stdout.split("\n").length} lines` : describe(help),
  );

  copyFileSync(new URL("./smoke-library.mjs", import.meta.url), join(project, "smoke-library.mjs"));
  const library = run(process.execPath, ["smoke-library.mjs"]);
  const lines = library.stdout
    .split("\n")
    .filter((line) => line.startsWith("{"))
    .map((line) => JSON.parse(line));
  for (const line of lines) {
    record(line.step, line.ok, line.ok ? line.detail : line.error.split("\n").slice(0, 3).join(" | "));
  }
  if (library.status !== 0 && lines.every((line) => line.ok)) {
    record("library steps", false, describe(library));
  }
} catch (error) {
  record("smoke test", false, error.message);
} finally {
  if (parsed.values.keep) {
    console.log(`kept ${work}`);
  } else {
    rmSync(work, { recursive: true, force: true });
  }
}

const failures = results.filter((result) => !result.ok);
if (failures.length > 0) {
  console.error(`smoke: ${failures.length} of ${results.length} steps failed: ${failures.map((item) => item.step).join(", ")}`);
  exitCode = 1;
} else {
  console.log(`smoke: all ${results.length} steps passed for ${manifest.name}@${manifest.version} on ${hostTarget.target}`);
}
process.exitCode = exitCode;
