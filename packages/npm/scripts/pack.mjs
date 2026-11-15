#!/usr/bin/env node
/**
 * Pack the athleticstandard npm tarball from native executables that were
 * already built and tested. This script never builds or downloads an
 * executable, so the tarball holds exactly the files that were tested.
 *
 * Usage:
 *
 *   node packages/npm/scripts/pack.mjs --native-dir <dir> [--out <dir>]
 *   node packages/npm/scripts/pack.mjs --native <target>=<file> ... [--out <dir>]
 *   node packages/npm/scripts/pack.mjs --native darwin-arm64=target/release/ath --allow-missing-targets
 *
 * What CI must supply:
 *
 * 1. One executable per target, built at the repository root with
 *    `cargo build --release --locked` and copied from target/release/.
 *    With --native-dir the folder holds exactly this layout:
 *
 *      <dir>/linux-x64/ath         from ubuntu-latest   (x86_64-unknown-linux-gnu)
 *      <dir>/darwin-arm64/ath      from macos-latest    (aarch64-apple-darwin)
 *      <dir>/win32-x64/ath.exe     from windows-latest  (x86_64-pc-windows-msvc)
 *
 *    File modes do not matter. Artifact downloads drop them, and the packed
 *    copies get mode 0755. --native <target>=<file> names one file and wins
 *    over --native-dir for that target.
 * 2. The repository's JSON Schema files schema/2.1.1/athleticstandard.schema.json,
 *    export-manifest.schema.json, and measurement-series.schema.json. They are
 *    committed and checked by tests/generated.rs.
 * 3. This package's Node dependencies (`npm ci` in packages/npm), for the
 *    TypeScript compiler.
 * 4. Node with its bundled npm.
 *
 * Every target is required. --allow-missing-targets packs only the targets
 * given, for a local check on one machine. That tarball must not be published.
 *
 * Steps: check each executable is built for its target; build dist/ with tsc;
 * copy the executables to native/<target>/, the repository skill to
 * skills/athletic-standard/, and the repository's schema files to schema/; run
 * npm pack; read the tarball back. Every export
 * target, the `ath` bin, and each executable must be in it, and each packed
 * executable must equal its input byte for byte. The SHA-256 of the tarball and
 * of each packed executable is printed and written to pack-summary.json beside
 * the tarball. The copied executables, skill, and schema files are removed
 * afterwards, so a later run in the checkout cannot pick up a stale copy.
 */
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import {
  lifecycleScripts,
  npmScripts,
  packageDirectory,
  packedManifest,
  readTarball,
  repositoryRoot,
  sha256,
  TARGETS,
} from "./package-tools.mjs";

const SKILL_NAME = "athletic-standard";

/** A reason to stop. Staged files are removed before the script exits. */
class PackError extends Error {}

function fail(message) {
  throw new PackError(message);
}

function readOptions() {
  try {
    return parseArgs({
      options: {
        "native-dir": { type: "string" },
        native: { type: "string", multiple: true, default: [] },
        out: { type: "string", default: join(packageDirectory, "pack-output") },
        "allow-missing-targets": { type: "boolean", default: false },
        help: { type: "boolean", default: false },
      },
      strict: true,
      allowPositionals: false,
    }).values;
  } catch (error) {
    fail(error.message);
  }
}

/** The input file for each target, or null when none was given. */
function nativeInputs(options) {
  const inputs = new Map(TARGETS.map(({ target }) => [target, null]));
  if (options["native-dir"] !== undefined) {
    for (const { target, executable } of TARGETS) {
      const file = resolve(options["native-dir"], target, executable);
      if (existsSync(file)) {
        inputs.set(target, file);
      }
    }
  }
  for (const pair of options.native) {
    const equals = pair.indexOf("=");
    const target = pair.slice(0, equals);
    if (equals < 1 || !inputs.has(target)) {
      fail(`--native expects <target>=<file> with a target from ${TARGETS.map((item) => item.target).join(", ")}`);
    }
    inputs.set(target, resolve(pair.slice(equals + 1)));
  }
  return inputs;
}

// The start of each executable format, and where the machine type sits.
function executableFormatProblem(target, bytes) {
  if (target === "linux-x64") {
    const elf = bytes.length > 20 && bytes.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46]));
    if (!elf) return "is not an ELF executable";
    if (bytes[4] !== 2 || bytes[5] !== 1) return "is not a 64-bit little-endian ELF executable";
    if (bytes.readUInt16LE(18) !== 0x3e) return "is not built for x86-64";
    return null;
  }
  if (target === "darwin-arm64") {
    if (bytes.length < 8 || bytes.readUInt32LE(0) !== 0xfeedfacf) return "is not a 64-bit Mach-O executable";
    if (bytes.readUInt32LE(4) !== 0x0100000c) return "is not built for arm64";
    return null;
  }
  if (target === "win32-x64") {
    if (bytes.length < 0x40 || bytes.toString("latin1", 0, 2) !== "MZ") return "is not a Windows executable";
    const header = bytes.readUInt32LE(0x3c);
    if (header + 6 > bytes.length || bytes.toString("latin1", header, header + 4) !== "PE\0\0") {
      return "has no PE header";
    }
    if (bytes.readUInt16LE(header + 4) !== 0x8664) return "is not built for x64";
    return null;
  }
  return "has no known format";
}

/** Every file below `directory`, refusing links so the tarball holds plain files. */
function listFiles(directory) {
  const files = [];
  for (const name of readdirSync(directory)) {
    const path = join(directory, name);
    const entry = lstatSync(path);
    if (entry.isSymbolicLink()) {
      fail(`${relative(repositoryRoot, path)} is a link. The packed skill must hold plain files.`);
    }
    if (entry.isDirectory()) {
      files.push(...listFiles(path));
    } else if (entry.isFile()) {
      files.push(path);
    }
  }
  return files;
}

/** The file targets of package.json exports and bin, without the leading "./". */
function manifestTargets(manifest) {
  const targets = new Set();
  const collect = (value) => {
    if (typeof value === "string") {
      targets.add(value.replace(/^\.\//, ""));
    } else if (value !== null && typeof value === "object") {
      Object.values(value).forEach(collect);
    }
  };
  collect(manifest.exports);
  collect(manifest.bin);
  return [...targets].sort();
}

function run(command, args, options) {
  const result = spawnSync(command, args, { stdio: ["ignore", "pipe", "inherit"], encoding: "utf8", ...options });
  if (result.error !== undefined) {
    fail(`could not run ${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`${[command, ...args].join(" ")} exited with ${result.status ?? result.signal}`);
  }
  return result.stdout;
}

async function main() {
  const options = readOptions();
  if (options.help) {
    console.log("usage: node packages/npm/scripts/pack.mjs --native-dir <dir> [--native <target>=<file>] [--out <dir>] [--allow-missing-targets]");
    return;
  }

  const manifestPath = join(packageDirectory, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const scripts = lifecycleScripts(manifest);
  if (scripts.length > 0) {
    fail(`package.json has lifecycle scripts (${scripts.join(", ")}). The package must not run anything on install.`);
  }

  // Inputs first, so a missing file fails before anything is built.
  const inputs = nativeInputs(options);
  const missing = [...inputs].filter(([, file]) => file === null).map(([target]) => target);
  if (missing.length === TARGETS.length) {
    fail("no native executables were given. Use --native-dir <dir> or --native <target>=<file>.");
  }
  if (missing.length > 0 && !options["allow-missing-targets"]) {
    fail(`missing native executables for ${missing.join(", ")}. Every target is required; see the header of this script.`);
  }
  const packed = [];
  for (const { target, executable } of TARGETS) {
    const file = inputs.get(target);
    if (file === null) continue;
    if (!existsSync(file) || !lstatSync(file).isFile()) {
      fail(`the ${target} executable ${file} is not a file`);
    }
    const bytes = readFileSync(file);
    const problem = executableFormatProblem(target, bytes);
    if (problem !== null) {
      fail(`the ${target} executable ${file} ${problem}`);
    }
    packed.push({ target, executable, input: file, sha256: sha256(bytes), size: bytes.length });
  }

  // The JSON Schema files are committed once, in the repository's schema/.
  const schemaTargets = manifestTargets(manifest).filter((target) => target.startsWith("schema/"));
  for (const target of schemaTargets) {
    if (!existsSync(join(repositoryRoot, target))) {
      fail(`missing ${target} in the repository. Run \`ATH_WRITE_GENERATED=1 cargo test --test generated\` first.`);
    }
  }
  const skillSource = join(repositoryRoot, "skills", SKILL_NAME);
  if (!existsSync(join(skillSource, "SKILL.md"))) {
    fail(`missing skills/${SKILL_NAME}/SKILL.md in the repository`);
  }
  const skillFiles = listFiles(skillSource);

  const staged = [join(packageDirectory, "native"), join(packageDirectory, "skills"), join(packageDirectory, "schema")];
  const licenseCopy = existsSync(join(packageDirectory, "LICENSE")) ? null : join(packageDirectory, "LICENSE");
  if (licenseCopy !== null) staged.push(licenseCopy);
  const dist = join(packageDirectory, "dist");
  for (const path of [dist, ...staged]) {
    rmSync(path, { recursive: true, force: true });
  }

  try {
    const tsc = createRequire(manifestPath).resolve("typescript/bin/tsc");
    run(process.execPath, [tsc, "-p", join(packageDirectory, "tsconfig.build.json")], { stdio: "inherit" });
    const cli = join(packageDirectory, manifest.bin.ath);
    if (!readFileSync(cli, "utf8").startsWith("#!/usr/bin/env node\n")) {
      fail(`${manifest.bin.ath} does not start with #!/usr/bin/env node`);
    }
    chmodSync(cli, 0o755);

    // The launcher's list of targets and this script's list must agree.
    const binary = await import(pathToFileURL(join(dist, "native-binary.js")).href);
    const launcherTargets = [...binary.SUPPORTED_NATIVE_TARGETS].sort().join(", ");
    const packTargets = TARGETS.map((item) => item.target).sort().join(", ");
    if (launcherTargets !== packTargets) {
      fail(`the launcher supports ${launcherTargets} but this script packs ${packTargets}`);
    }

    for (const item of packed) {
      const destination = join(packageDirectory, "native", item.target, item.executable);
      mkdirSync(join(packageDirectory, "native", item.target), { recursive: true });
      copyFileSync(item.input, destination);
      chmodSync(destination, 0o755);
    }
    cpSync(skillSource, join(packageDirectory, "skills", SKILL_NAME), { recursive: true });
    for (const target of schemaTargets) {
      mkdirSync(dirname(join(packageDirectory, target)), { recursive: true });
      copyFileSync(join(repositoryRoot, target), join(packageDirectory, target));
    }
    if (licenseCopy !== null) {
      copyFileSync(join(repositoryRoot, "LICENSE"), licenseCopy);
    }

    for (const target of manifestTargets(manifest)) {
      if (!existsSync(join(packageDirectory, target))) {
        fail(`package.json names ${target}, which the build did not produce`);
      }
    }

    const out = resolve(options.out);
    mkdirSync(out, { recursive: true });
    const { npm } = npmScripts();
    const report = JSON.parse(
      run(process.execPath, [npm, "pack", "--json", "--pack-destination", out], {
        cwd: packageDirectory,
        env: { ...process.env, npm_config_update_notifier: "false", npm_config_fund: "false" },
      }),
    );
    const tarball = join(out, report[0].filename);
    const entries = readTarball(tarball);
    const byPath = new Map(entries.map((entry) => [entry.path, entry]));

    const problems = [];
    for (const entry of entries) {
      if (!entry.path.startsWith("package/")) problems.push(`${entry.path} is outside package/`);
      if (entry.type !== "file" && entry.type !== "directory") problems.push(`${entry.path} is a ${entry.type}`);
      if (/^package\/(src|test|scripts)\//.test(entry.path)) problems.push(`${entry.path} is not part of the package`);
    }
    const packedPackage = packedManifest(entries);
    if (packedPackage.name !== manifest.name || packedPackage.version !== manifest.version) {
      problems.push(`the packed package.json is ${packedPackage.name}@${packedPackage.version}`);
    }
    for (const target of manifestTargets(manifest)) {
      if (!byPath.has(`package/${target}`)) problems.push(`the tarball has no ${target}`);
    }
    for (const file of skillFiles) {
      const path = `package/skills/${SKILL_NAME}/${relative(skillSource, file).split("\\").join("/")}`;
      if (!byPath.has(path)) problems.push(`the tarball has no ${path.slice("package/".length)}`);
    }
    const nativeEntries = entries.filter((entry) => entry.path.startsWith("package/native/"));
    for (const item of packed) {
      const path = `package/native/${item.target}/${item.executable}`;
      const entry = byPath.get(path);
      if (entry === undefined) {
        problems.push(`the tarball has no native/${item.target}/${item.executable}`);
        continue;
      }
      if ((entry.mode & 0o111) === 0) problems.push(`${path} is not executable (mode ${entry.mode.toString(8)})`);
      item.packedSha256 = sha256(entry.data);
      if (item.packedSha256 !== item.sha256) problems.push(`${path} differs from ${item.input}`);
    }
    for (const entry of nativeEntries) {
      if (!packed.some((item) => entry.path === `package/native/${item.target}/${item.executable}`)) {
        problems.push(`${entry.path} was not given as an input`);
      }
    }
    if (problems.length > 0) {
      fail(`the tarball is not complete:\n  ${problems.join("\n  ")}`);
    }

    const tarballSha256 = sha256(readFileSync(tarball));
    const summary = {
      package: `${manifest.name}@${manifest.version}`,
      tarball: report[0].filename,
      sha256: tarballSha256,
      files: entries.length,
      missing_targets: missing,
      native: packed.map((item) => ({
        target: item.target,
        path: `native/${item.target}/${item.executable}`,
        sha256: item.packedSha256,
        size: item.size,
        input: item.input,
      })),
    };
    writeFileSync(join(out, "pack-summary.json"), `${JSON.stringify(summary, null, 2)}\n`);

    console.log(`packed ${tarball} (${entries.length} files)`);
    console.log(`${tarballSha256}  ${report[0].filename}`);
    for (const item of summary.native) {
      console.log(`${item.sha256}  ${item.path}  (from ${item.input})`);
    }
    if (missing.length > 0) {
      console.error(`pack: WARNING: no executable for ${missing.join(", ")}. Do not publish this tarball.`);
    }
  } finally {
    for (const path of staged) {
      rmSync(path, { recursive: true, force: true });
    }
  }
}

try {
  await main();
} catch (error) {
  console.error(`pack: ${error instanceof PackError ? error.message : error.stack}`);
  process.exitCode = 1;
}
