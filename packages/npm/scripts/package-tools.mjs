/**
 * Helpers shared by pack.mjs and smoke.mjs: the package layout, finding npm
 * and other programs, and reading a packed tarball. Plain Node with no
 * dependencies, so CI can run the scripts before anything is built.
 */
import { createHash } from "node:crypto";
import { accessSync, constants, existsSync, readFileSync, realpathSync, statSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

/** packages/npm in the repository. */
export const packageDirectory = resolve(fileURLToPath(new URL("..", import.meta.url)));

/** The repository root. */
export const repositoryRoot = resolve(packageDirectory, "..", "..");

/**
 * The packed targets. `target` is `${process.platform}-${process.arch}` and
 * must match SUPPORTED_NATIVE_TARGETS in src/native-binary.ts.
 */
export const TARGETS = [
  { target: "linux-x64", executable: "ath", rust: "x86_64-unknown-linux-gnu" },
  { target: "darwin-arm64", executable: "ath", rust: "aarch64-apple-darwin" },
  { target: "win32-x64", executable: "ath.exe", rust: "x86_64-pc-windows-msvc" },
];

export function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

/** Read an environment variable, ignoring case on Windows. */
export function environmentValue(env, name) {
  if (process.platform !== "win32") {
    return env[name];
  }
  const key = Object.keys(env).find((item) => item.toUpperCase() === name.toUpperCase());
  return key === undefined ? undefined : env[key];
}

function isExecutableFile(path) {
  try {
    if (!statSync(path).isFile()) {
      return false;
    }
    if (process.platform !== "win32") {
      accessSync(path, constants.X_OK);
    }
    return true;
  } catch {
    return false;
  }
}

/**
 * Find a program on a PATH value, as the system would run it. Windows tries
 * each PATHEXT extension. Relative PATH entries are skipped.
 */
export function findOnPath(name, pathValue, pathExt = ".COM;.EXE;.BAT;.CMD") {
  const separator = process.platform === "win32" ? ";" : ":";
  const extensions = process.platform === "win32" ? ["", ...pathExt.split(";").filter(Boolean)] : [""];
  for (const entry of (pathValue ?? "").split(separator)) {
    const directory = entry.replace(/^"(.*)"$/, "$1");
    if (directory === "" || !isAbsolute(directory)) {
      continue;
    }
    for (const extension of extensions) {
      const candidate = join(directory, `${name}${extension}`);
      if (isExecutableFile(candidate)) {
        return candidate;
      }
    }
  }
  return null;
}

/**
 * The npm and npx entry scripts. They run with this Node, so no npm shim or
 * shell is needed on any system.
 */
export function npmScripts(env = process.env) {
  const candidates = [];
  const onPath = findOnPath("npm", environmentValue(env, "PATH"), environmentValue(env, "PATHEXT"));
  if (onPath !== null) {
    candidates.push(
      process.platform === "win32"
        ? join(dirname(onPath), "node_modules", "npm", "bin")
        : dirname(realpathSync(onPath)),
    );
  }
  candidates.push(join(dirname(process.execPath), "node_modules", "npm", "bin"));
  candidates.push(join(dirname(process.execPath), "..", "lib", "node_modules", "npm", "bin"));
  for (const bin of candidates) {
    const npm = join(bin, "npm-cli.js");
    const npx = join(bin, "npx-cli.js");
    if (existsSync(npm) && existsSync(npx)) {
      return { npm, npx };
    }
  }
  throw new Error("npm was not found. Install a Node.js release that includes npm.");
}

function field(header, start, length) {
  const raw = header.subarray(start, start + length);
  const end = raw.indexOf(0);
  return raw.subarray(0, end === -1 ? length : end).toString("utf8");
}

// Extended header records are "<length> <key>=<value>\n", with byte lengths.
function paxRecords(body) {
  const records = {};
  let offset = 0;
  while (offset < body.length) {
    const space = body.indexOf(0x20, offset);
    if (space === -1) {
      break;
    }
    const length = Number(body.subarray(offset, space).toString("ascii"));
    if (!Number.isInteger(length) || length <= 0) {
      break;
    }
    const record = body.subarray(space + 1, offset + length - 1).toString("utf8");
    const equals = record.indexOf("=");
    records[record.slice(0, equals)] = record.slice(equals + 1);
    offset += length;
  }
  return records;
}

const ENTRY_TYPES = { 0: "file", 5: "directory", 2: "symlink", 1: "hardlink" };

/**
 * Read a .tgz into entries: path, mode, type, and the file bytes. Handles the
 * ustar prefix field, pax extended headers, and GNU long names.
 */
export function readTarball(file) {
  const data = gunzipSync(readFileSync(file));
  const entries = [];
  let offset = 0;
  let nextPath = null;
  while (offset + 512 <= data.length) {
    const header = data.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) {
      break;
    }
    const name = field(header, 0, 100);
    const mode = parseInt(field(header, 100, 8).trim() || "0", 8);
    const size = parseInt(field(header, 124, 12).trim() || "0", 8);
    const flag = header[156] === 0 ? "0" : String.fromCharCode(header[156]);
    const prefix = field(header, 257, 6).startsWith("ustar") ? field(header, 345, 155) : "";
    const body = data.subarray(offset + 512, offset + 512 + size);
    offset += 512 + Math.ceil(size / 512) * 512;
    if (flag === "x") {
      nextPath = paxRecords(body).path ?? null;
      continue;
    }
    if (flag === "g") {
      continue;
    }
    if (flag === "L") {
      nextPath = body.toString("utf8").replace(/\0[\s\S]*$/, "");
      continue;
    }
    const path = nextPath ?? (prefix === "" ? name : `${prefix}/${name}`);
    nextPath = null;
    entries.push({ path, mode, type: ENTRY_TYPES[flag] ?? `type ${flag}`, data: Buffer.from(body) });
  }
  return entries;
}

/** The parsed package.json inside a packed tarball. */
export function packedManifest(entries) {
  const entry = entries.find((item) => item.path === "package/package.json");
  if (entry === undefined) {
    throw new Error("the tarball has no package/package.json");
  }
  return JSON.parse(entry.data.toString("utf8"));
}

/** npm lifecycle scripts that would run on install or pack. The package must have none. */
export const LIFECYCLE_SCRIPTS = [
  "preinstall",
  "install",
  "postinstall",
  "prepublish",
  "preprepare",
  "prepare",
  "postprepare",
  "prepack",
  "postpack",
];

export function lifecycleScripts(manifest) {
  return LIFECYCLE_SCRIPTS.filter((name) => Object.hasOwn(manifest.scripts ?? {}, name));
}
