/**
 * The `ath` launcher: forwarding, installation errors, and the update handoff.
 *
 * Each test runs the launcher as its own Node process from a temporary
 * package folder laid out like an installed copy. Fake native executables and
 * fake package managers are small Node scripts. They log what they received,
 * so a test can prove what ran and what did not.
 */
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import {
  batchInvocation,
  checkUpdatePlan,
  findWindowsProgram,
  isUpdateRun,
  managerInvocation,
  nativeEnvironment,
  parseUpdatePlan,
  quoteBatchArgument,
  readUpdatePlan,
  UpdatePlanError,
} from "../src/launcher.js";
import {
  AthInstallationError,
  NATIVE_EXECUTABLE_VARIABLE,
  nativeExecutableName,
  nativeTarget,
  resolveNativeExecutable,
  SUPPORTED_NATIVE_TARGETS,
} from "../src/native-binary.js";

const packageDirectory = fileURLToPath(new URL("..", import.meta.url));
const sourceDirectory = join(packageDirectory, "src");
const repositoryRoot = join(packageDirectory, "..", "..");
const packageVersion = (
  JSON.parse(readFileSync(join(packageDirectory, "package.json"), "utf8")) as { version: string }
).version;

const posix = process.platform !== "win32";
const hostTarget = nativeTarget();

// The launcher modules, compiled once. Compiling only these files keeps the
// launcher tests independent of the rest of the package.
const LAUNCHER_SOURCES = ["cli.ts", "launcher.ts", "native-binary.ts"];
const compiled = new Map<string, string>();

// Spaces and non-ASCII letters in every path the launcher handles.
let root = "";
let counter = 0;

beforeAll(() => {
  root = mkdtempSync(join(realpathSync(tmpdir()), "ath launcher ü-"));
  for (const file of LAUNCHER_SOURCES) {
    const source = readFileSync(join(sourceDirectory, file), "utf8");
    const output = ts.transpileModule(source, {
      fileName: file,
      compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
    });
    compiled.set(file.replace(/\.ts$/, ".js"), output.outputText);
  }
});

afterAll(() => {
  if (root !== "") {
    rmSync(root, { recursive: true, force: true });
  }
});

function freshDirectory(name: string): string {
  counter += 1;
  const directory = join(root, `${counter} ${name}`);
  mkdirSync(directory, { recursive: true });
  return directory;
}

interface FakePackage {
  root: string;
  cli: string;
}

/** A package folder with package.json and the compiled launcher in dist/. */
function makePackage(directory: string): FakePackage {
  mkdirSync(join(directory, "dist"), { recursive: true });
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({ name: "athleticstandard", version: packageVersion, type: "module" }),
  );
  for (const [file, text] of compiled) {
    writeFileSync(join(directory, "dist", file), text);
  }
  return { root: directory, cli: join(directory, "dist", "cli.js") };
}

/**
 * An executable Node script. It works whether Node loads it as CommonJS or as
 * an ES module, because a fake inside the package folder sits under
 * `"type": "module"` and has no file extension.
 */
function writeScript(path: string, body: string): string {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(
    path,
    `#!${process.execPath}\n(async () => {\nconst fs = await import("node:fs");\nconst path = await import("node:path");\n` +
      `const log = (entry) => fs.appendFileSync(process.env.FAKE_LOG, JSON.stringify(entry) + "\\n");\n` +
      `${body}\n})().catch((error) => { console.error(error); process.exit(99); });\n`,
  );
  chmodSync(path, 0o755);
  return path;
}

/** The environment of the test run without anything the launcher owns or reads. */
function cleanEnvironment(extra: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = { ...process.env };
  for (const name of [
    NATIVE_EXECUTABLE_VARIABLE,
    "ATH_INSTALL_PATHS",
    "ATH_UPDATE_HANDOFF",
    "ATH_UPDATE_PHASE",
    "NODE_OPTIONS",
  ]) {
    delete env[name];
  }
  return { ...env, ...extra };
}

function launch(
  cli: string,
  args: string[],
  env: NodeJS.ProcessEnv,
  options: { input?: string; cwd?: string } = {},
) {
  return spawnSync(process.execPath, [cli, ...args], {
    env,
    cwd: options.cwd ?? root,
    input: options.input ?? "",
    encoding: "utf8",
    timeout: 30_000,
  });
}

interface Running {
  child: ChildProcess;
  stdout: () => string;
  stderr: () => string;
  waitFor: (text: string) => Promise<void>;
  ended: Promise<{ code: number | null; signal: NodeJS.Signals | null }>;
}

// Launchers still running when a test ends are killed. Waiting fakes end
// themselves after 20 seconds, so a broken launcher leaves nothing behind.
const started = new Set<ChildProcess>();

afterEach(() => {
  for (const child of started) {
    if (child.exitCode === null && child.signalCode === null) {
      child.kill("SIGKILL");
    }
  }
  started.clear();
});

function start(cli: string, args: string[], env: NodeJS.ProcessEnv): Running {
  const child = spawn(process.execPath, [cli, ...args], { env, cwd: root, stdio: "pipe" });
  started.add(child);
  let out = "";
  let err = "";
  const waiters: Array<{ text: string; resolve: () => void }> = [];
  const check = () => {
    for (const waiter of [...waiters]) {
      if (out.includes(waiter.text)) {
        waiters.splice(waiters.indexOf(waiter), 1);
        waiter.resolve();
      }
    }
  };
  child.stdout.setEncoding("utf8").on("data", (chunk: string) => {
    out += chunk;
    check();
  });
  child.stderr.setEncoding("utf8").on("data", (chunk: string) => {
    err += chunk;
  });
  const ended = new Promise<{ code: number | null; signal: NodeJS.Signals | null }>((resolve) => {
    child.on("close", (code, signal) => resolve({ code, signal }));
  });
  return {
    child,
    stdout: () => out,
    stderr: () => err,
    waitFor: (text) =>
      new Promise<void>((resolve, reject) => {
        waiters.push({ text, resolve });
        check();
        void ended.then(() => reject(new Error(`ended before "${text}": ${out} ${err}`)));
      }),
    ended,
  };
}

interface LogEntry {
  role: string;
  name?: string;
  args: string[];
  phase?: string | null;
  handoff?: string | null;
  paths?: string | null;
  handoffExisted?: boolean;
  directoryMode?: number;
}

function readLog(path: string): LogEntry[] {
  if (!existsSync(path)) {
    return [];
  }
  return readFileSync(path, "utf8")
    .split("\n")
    .filter((line) => line !== "")
    .map((line) => JSON.parse(line) as LogEntry);
}

const EXPECTED_ARGS = {
  npm: ["install", "-g", "athleticstandard@latest"],
  pnpm: ["add", "-g", "athleticstandard@latest"],
  bun: ["add", "-g", "athleticstandard@latest"],
} as const;

type Manager = keyof typeof EXPECTED_ARGS;

function plan(manager: Manager, changes: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    ath_update_plan: 1,
    manager,
    program: manager,
    args: [...EXPECTED_ARGS[manager]],
    current_version: "2.1.0",
    latest_version: "2.1.1",
    success_message: "updated ath from 2.1.0 to 2.1.1.",
    failure_message: "ath: the install failed.",
    refresh_args: ["--update"],
    refresh_environment: { ATH_UPDATE_PHASE: "refresh-skills" },
    ...changes,
  };
}

// The update side of a fake native executable. The check run records its
// handoff and writes FAKE_PLAN there. The refresh run records its phase.
const UPDATE_NATIVE = `
const handoff = process.env.ATH_UPDATE_HANDOFF ?? null;
const phase = process.env.ATH_UPDATE_PHASE ?? null;
const entry = { role: "native", name: process.env.FAKE_NAME_OVERRIDE ?? NAME, args: process.argv.slice(2), phase, handoff, paths: process.env.ATH_INSTALL_PATHS ?? null };
if (handoff !== null) {
  entry.handoffExisted = fs.existsSync(handoff);
  entry.directoryMode = fs.statSync(path.dirname(handoff)).mode & 0o777;
}
log(entry);
if (phase === "refresh-skills") {
  process.stdout.write("refreshed skills by " + entry.name + "\\n");
  process.exit(Number(process.env.FAKE_REFRESH_STATUS ?? 0));
}
if (handoff !== null && process.env.FAKE_PLAN !== undefined) {
  fs.writeFileSync(handoff, process.env.FAKE_PLAN, { flag: "wx", mode: 0o600 });
}
process.exit(Number(process.env.FAKE_CHECK_STATUS ?? 0));
`;

function updateNative(path: string, name = "native"): string {
  return writeScript(path, `const NAME = ${JSON.stringify(name)};\n${UPDATE_NATIVE}`);
}

// A fake package manager. FAKE_MANAGER chooses how it ends. FAKE_REPOINT and
// FAKE_REPOINT_TO move a package link the way pnpm replaces a global install.
const FAKE_MANAGER = `
log({ role: "manager", name: path.basename(process.argv[1]), args: process.argv.slice(2) });
const mode = process.env.FAKE_MANAGER ?? "succeed";
if (mode === "fail") {
  process.stderr.write("fake manager failed\\n");
  process.exit(1);
}
if (mode === "kill") {
  process.kill(process.pid, "SIGKILL");
}
if (mode === "wait") {
  process.stdout.write("manager ready\\n");
  setTimeout(() => process.exit(98), 20_000);
  return;
}
if (process.env.FAKE_REPOINT !== undefined) {
  fs.unlinkSync(process.env.FAKE_REPOINT);
  fs.symlinkSync(process.env.FAKE_REPOINT_TO, process.env.FAKE_REPOINT);
  fs.rmSync(process.env.FAKE_REMOVE, { recursive: true, force: true });
}
process.exit(0);
`;

/** A PATH folder holding fake npm, pnpm, bun, and a decoy named `evil`. */
function managerFolder(): string {
  const directory = freshDirectory("managers");
  for (const name of ["npm", "pnpm", "bun", "evil"]) {
    writeScript(join(directory, name), FAKE_MANAGER);
  }
  return directory;
}

describe("plan checks", () => {
  // The exact text the native program writes (tests/runtime_update.rs).
  const nativePlanText = `{
  "ath_update_plan": 1,
  "manager": "npm",
  "program": "npm",
  "args": [
    "install",
    "-g",
    "athleticstandard@latest"
  ],
  "current_version": "2.1.1",
  "latest_version": "2.1.2",
  "success_message": "updated ath from 2.1.1 to 2.1.2.",
  "failure_message": "ath: the install failed.",
  "refresh_args": [
    "--update"
  ],
  "refresh_environment": {
    "ATH_UPDATE_PHASE": "refresh-skills"
  }
}
`;

  it("accepts the plan the native program writes for each manager", () => {
    expect(parseUpdatePlan(nativePlanText)).toMatchObject({
      program: "npm",
      args: ["install", "-g", "athleticstandard@latest"],
      success_message: "updated ath from 2.1.1 to 2.1.2.",
    });
    for (const manager of ["npm", "pnpm", "bun"] as const) {
      expect(checkUpdatePlan(plan(manager)).args).toEqual(EXPECTED_ARGS[manager]);
    }
  });

  it("refuses every plan this ath would not write", () => {
    const forged: Array<Record<string, unknown>> = [
      plan("npm", { program: "sh" }),
      plan("npm", { program: "pnpm" }),
      plan("npm", { program: "npm; touch pwned" }),
      plan("npm", { program: "npm.cmd" }),
      plan("pnpm", { args: [...EXPECTED_ARGS.npm] }),
      plan("npm", { args: [...EXPECTED_ARGS.npm, "--ignore-scripts=false"] }),
      plan("npm", { args: ["install", "-g", "athleticstandard@latest; touch pwned"] }),
      plan("npm", { args: ["install", "-g", "athleticstandard@file:../evil"] }),
      plan("npm", { args: ["install", "-g"] }),
      plan("npm", { args: "install -g athleticstandard@latest" }),
      plan("npm", { ath_update_plan: 2 }),
      plan("npm", { ath_update_plan: "1" }),
      plan("npm", { manager: "yarn", program: "yarn" }),
      plan("npm", { latest_version: "2.1.2; rm -rf /" }),
      plan("npm", { current_version: "v2.1.0" }),
      plan("npm", { latest_version: "2.1.1-beta" }),
      plan("npm", { latest_version: "2.1.1\n" }),
      plan("npm", { refresh_args: ["init"] }),
      plan("npm", { refresh_args: ["--update", "--yes"] }),
      plan("npm", { refresh_environment: {} }),
      plan("npm", { refresh_environment: { ATH_UPDATE_PHASE: "check" } }),
      plan("npm", {
        refresh_environment: { ATH_UPDATE_PHASE: "refresh-skills", NODE_OPTIONS: "--require=evil" },
      }),
      plan("npm", { success_message: 1 }),
      plan("npm", { shell: true }),
    ];
    const missing = plan("npm");
    delete missing.failure_message;
    forged.push(missing);
    for (const value of forged) {
      expect(() => checkUpdatePlan(value), JSON.stringify(value)).toThrow(UpdatePlanError);
      expect(() => parseUpdatePlan(JSON.stringify(value))).toThrow(/nothing was installed/);
    }
    for (const text of ["", "not json", "[]", "null", nativePlanText.replace("{", '{\n  "shell": true,')]) {
      expect(() => parseUpdatePlan(text), text).toThrow(UpdatePlanError);
    }
  });

  it.skipIf(!posix)("reads only a small regular plan file", () => {
    const directory = freshDirectory("plan files");
    const real = join(directory, "real.json");
    writeFileSync(real, JSON.stringify(plan("npm")));
    expect(readUpdatePlan(join(directory, "absent.json"))).toBeNull();
    expect(readUpdatePlan(real)?.program).toBe("npm");
    const link = join(directory, "link.json");
    symlinkSync(real, link);
    expect(() => readUpdatePlan(link)).toThrow(/not a regular file/);
    const large = join(directory, "large.json");
    writeFileSync(large, " ".repeat(70 * 1024) + JSON.stringify(plan("npm")));
    expect(() => readUpdatePlan(large)).toThrow(/too large/);
    expect(() => readUpdatePlan(directory)).toThrow(/not a regular file/);
  });

  it("treats only a leading --update as an update run", () => {
    expect(isUpdateRun(["--update"])).toBe(true);
    expect(isUpdateRun(["--update", "log"])).toBe(true);
    expect(isUpdateRun(["log", "--", "--update"])).toBe(false);
    expect(isUpdateRun(["--version"])).toBe(false);
  });

  it("removes inherited update variables and sets the install paths", () => {
    const env = nativeEnvironment(
      { KEEP: "yes", ATH_UPDATE_HANDOFF: "/x", ath_update_phase: "refresh-skills", ATH_INSTALL_PATHS: "/y" },
      "C:\\bin\\ath",
      "C:\\pkg\\dist\\cli.js",
      "win32",
    );
    expect(env).toEqual({ KEEP: "yes", ATH_INSTALL_PATHS: "C:\\bin\\ath;C:\\pkg\\dist\\cli.js" });
    expect(nativeEnvironment({}, undefined, "/pkg/dist/cli.js", "linux")).toEqual({
      ATH_INSTALL_PATHS: "/pkg/dist/cli.js",
    });
  });
});

describe("native executable lookup", () => {
  it("names a specific installation error on an unsupported system", () => {
    for (const [platform, arch] of [
      ["freebsd", "x64"],
      ["linux", "arm64"],
      ["darwin", "x64"],
      ["win32", "arm64"],
      ["win32", "ia32"],
    ] as const) {
      expect(nativeTarget(platform, arch)).toBeNull();
      let thrown: unknown;
      try {
        resolveNativeExecutable({}, platform, arch, root);
      } catch (error) {
        thrown = error;
      }
      expect(thrown).toBeInstanceOf(AthInstallationError);
      expect((thrown as Error).message).toContain(`no native ath for ${platform}-${arch}`);
      expect((thrown as Error).message).toContain(SUPPORTED_NATIVE_TARGETS.join(", "));
      expect((thrown as Error).message).toContain("Nothing was downloaded or compiled");
    }
  });

  it("finds each packed target under native/<target>/", () => {
    const directory = freshDirectory("packed targets");
    for (const target of SUPPORTED_NATIVE_TARGETS) {
      const [platform, arch] = target.split("-") as [string, string];
      const file = join(directory, "native", target, nativeExecutableName(target));
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, "");
      expect(resolveNativeExecutable({}, platform, arch, directory)).toBe(file);
    }
    expect(nativeExecutableName("win32-x64")).toBe("ath.exe");
    expect(nativeExecutableName("linux-x64")).toBe("ath");
  });

  it("reports a missing packed executable and runs nothing", () => {
    const pkg = makePackage(freshDirectory("empty package"));
    const result = launch(pkg.cli, ["--version"], cleanEnvironment({}));
    expect(result.status).toBe(1);
    expect(result.stdout).toBe("");
    if (hostTarget === null) {
      expect(result.stderr).toContain("has no native ath for");
    } else {
      expect(result.stderr).toContain(`is missing its native ath for ${hostTarget}`);
      expect(result.stderr).toContain(join(pkg.root, "native", hostTarget));
    }
    expect(result.stderr).toContain("nothing was downloaded or compiled");
  });

  it.skipIf(!posix || hostTarget === null)("runs the packed executable for this system", () => {
    const pkg = makePackage(freshDirectory("packed package"));
    writeScript(
      join(pkg.root, "native", hostTarget!, nativeExecutableName(hostTarget!)),
      `process.stdout.write("packed " + process.argv.slice(2).join(" "));`,
    );
    const result = launch(pkg.cli, ["--version"], cleanEnvironment({}));
    expect(result.stderr).toBe("");
    expect(result.stdout).toBe("packed --version");
    expect(result.status).toBe(0);
  });
});

describe.skipIf(!posix)("forwarding", () => {
  it("passes arguments, standard input, output, error, environment, and exit status", () => {
    const directory = freshDirectory("forwarding");
    const native = writeScript(
      join(directory, "ath"),
      `let input = "";
for await (const chunk of process.stdin) input += chunk;
process.stdout.write(JSON.stringify({
  args: process.argv.slice(2),
  paths: process.env.ATH_INSTALL_PATHS ?? null,
  handoff: process.env.ATH_UPDATE_HANDOFF ?? null,
  phase: process.env.ATH_UPDATE_PHASE ?? null,
  kept: process.env.FAKE_KEPT ?? null,
}));
process.stderr.write("error: " + input.toUpperCase());
process.exitCode = 7;`,
    );
    const pkg = makePackage(freshDirectory("package"));
    const args = ["log", "two words", "", "ünïcödé ✓", "$HOME", "*", 'a "quote"', "--json", "--", "--update"];
    const result = launch(
      pkg.cli,
      args,
      cleanEnvironment({
        [NATIVE_EXECUTABLE_VARIABLE]: native,
        ATH_UPDATE_HANDOFF: join(directory, "elsewhere.json"),
        ATH_UPDATE_PHASE: "refresh-skills",
        FAKE_KEPT: "kept",
      }),
      { input: "hello\nworld\n" },
    );
    expect(result.status).toBe(7);
    expect(result.signal).toBeNull();
    expect(JSON.parse(result.stdout)).toEqual({
      args,
      paths: `${pkg.cli}:${pkg.cli}`,
      handoff: null,
      phase: null,
      kept: "kept",
    });
    expect(result.stderr).toBe("error: HELLO\nWORLD\n");
  });

  it("names the command link and the launcher module in ATH_INSTALL_PATHS", () => {
    const directory = freshDirectory("linked");
    const native = writeScript(join(directory, "ath"), `process.stdout.write(process.env.ATH_INSTALL_PATHS);`);
    const pkg = makePackage(freshDirectory("package"));
    const link = join(directory, "bin", "ath");
    mkdirSync(dirname(link));
    symlinkSync(pkg.cli, link);
    const result = launch(link, [], cleanEnvironment({ [NATIVE_EXECUTABLE_VARIABLE]: native }));
    expect(result.stdout).toBe(`${link}:${pkg.cli}`);
    expect(result.status).toBe(0);
  });

  it("reports a native executable that cannot start", () => {
    const pkg = makePackage(freshDirectory("package"));
    const missing = join(root, "no such ath");
    const result = launch(pkg.cli, ["--version"], cleanEnvironment({ [NATIVE_EXECUTABLE_VARIABLE]: missing }));
    expect(result.status).toBe(1);
    expect(result.stderr).toContain(`ath: could not run ${missing}`);
  });

  for (const [index, signal] of (["SIGINT", "SIGTERM", "SIGHUP"] as const).entries()) {
    it(`forwards ${signal} to the native process and exits with its status`, async () => {
      const directory = freshDirectory("signals");
      const native = writeScript(
        join(directory, "ath"),
        `for (const [index, name] of ["SIGINT", "SIGTERM", "SIGHUP"].entries()) {
  process.on(name, () => {
    process.stdout.write("native got " + name + "\\n");
    process.exit(40 + index);
  });
}
process.stdout.write("native ready\\n");
setTimeout(() => process.exit(98), 20_000);`,
      );
      const pkg = makePackage(freshDirectory("package"));
      const run = start(pkg.cli, [], cleanEnvironment({ [NATIVE_EXECUTABLE_VARIABLE]: native }));
      await run.waitFor("native ready");
      run.child.kill(signal);
      const end = await run.ended;
      expect(run.stdout()).toContain(`native got ${signal}`);
      expect(end).toEqual({ code: 40 + index, signal: null });
    });
  }

  it("ends with the signal that ended the native process", async () => {
    const directory = freshDirectory("signal exit");
    const native = writeScript(
      join(directory, "ath"),
      `process.stdout.write("native ready\\n");
setTimeout(() => process.exit(98), 20_000);`,
    );
    const pkg = makePackage(freshDirectory("package"));
    const run = start(pkg.cli, [], cleanEnvironment({ [NATIVE_EXECUTABLE_VARIABLE]: native }));
    await run.waitFor("native ready");
    run.child.kill("SIGTERM");
    expect(await run.ended).toEqual({ code: null, signal: "SIGTERM" });

    const killed = writeScript(join(directory, "killed"), `process.kill(process.pid, "SIGKILL");`);
    const result = launch(pkg.cli, [], cleanEnvironment({ [NATIVE_EXECUTABLE_VARIABLE]: killed }));
    expect(result.signal).toBe("SIGKILL");
  });
});

describe.skipIf(!posix)("update handoff", () => {
  interface UpdateRun {
    result: ReturnType<typeof launch>;
    log: LogEntry[];
    cwd: string;
  }

  function runUpdate(env: NodeJS.ProcessEnv, native?: string): UpdateRun {
    const directory = freshDirectory("update");
    const logPath = join(directory, "log.jsonl");
    const cwd = join(directory, "work");
    mkdirSync(cwd);
    const pkg = makePackage(join(directory, "package"));
    const executable = native ?? updateNative(join(directory, "native", "ath"));
    const result = launch(
      pkg.cli,
      ["--update"],
      cleanEnvironment({
        PATH: managerFolder(),
        FAKE_LOG: logPath,
        [NATIVE_EXECUTABLE_VARIABLE]: executable,
        ...env,
      }),
      { cwd },
    );
    return { result, log: readLog(logPath), cwd };
  }

  function expectRemovedHandoff(log: LogEntry[]): void {
    const check = log[0]!;
    expect(check.role).toBe("native");
    expect(check.handoff).toMatch(/update-plan\.json$/);
    expect(check.handoffExisted).toBe(false);
    expect(check.directoryMode).toBe(0o700);
    expect(existsSync(dirname(check.handoff!))).toBe(false);
  }

  for (const manager of ["npm", "pnpm", "bun"] as const) {
    it(`installs with ${manager} and refreshes skills with the new executable`, () => {
      const { result, log } = runUpdate({ FAKE_PLAN: JSON.stringify(plan(manager)) });
      expect(result.stderr).toBe("");
      expect(result.stdout).toBe("updated ath from 2.1.0 to 2.1.1.\nrefreshed skills by native\n");
      expect(result.status).toBe(0);
      expect(log.map((entry) => entry.role)).toEqual(["native", "manager", "native"]);
      expect(log[1]).toEqual({ role: "manager", name: manager, args: EXPECTED_ARGS[manager] });
      expect(log[0]!.phase).toBeNull();
      expect(log[2]).toMatchObject({ args: ["--update"], phase: "refresh-skills", handoff: null });
      expect(log[2]!.paths).toBe(log[0]!.paths);
      expectRemovedHandoff(log);
    });
  }

  it("exits with the refresh run's status", () => {
    const { result, log } = runUpdate({ FAKE_PLAN: JSON.stringify(plan("npm")), FAKE_REFRESH_STATUS: "3" });
    expect(result.status).toBe(3);
    expect(log).toHaveLength(3);
  });

  it("exits with the native status when no plan was written", () => {
    for (const status of [0, 1]) {
      const { result, log } = runUpdate({ FAKE_CHECK_STATUS: String(status) });
      expect(result.status).toBe(status);
      expect(result.stdout).toBe("");
      expect(log.map((entry) => entry.role)).toEqual(["native"]);
      expectRemovedHandoff(log);
    }
  });

  it("ignores a plan from a native run that failed", () => {
    const { result, log } = runUpdate({ FAKE_PLAN: JSON.stringify(plan("npm")), FAKE_CHECK_STATUS: "2" });
    expect(result.status).toBe(2);
    expect(log.map((entry) => entry.role)).toEqual(["native"]);
    expectRemovedHandoff(log);
  });

  it("reports a failed install and does not refresh", () => {
    const { result, log } = runUpdate({ FAKE_PLAN: JSON.stringify(plan("npm")), FAKE_MANAGER: "fail" });
    expect(result.status).toBe(1);
    expect(result.stdout).toBe("");
    expect(result.stderr).toBe("fake manager failed\nath: the install failed.\n");
    expect(log.map((entry) => entry.role)).toEqual(["native", "manager"]);
    expectRemovedHandoff(log);
  });

  it("reports a manager ended by a signal and does not refresh", () => {
    const { result, log } = runUpdate({ FAKE_PLAN: JSON.stringify(plan("pnpm")), FAKE_MANAGER: "kill" });
    expect(result.status).toBe(1);
    expect(result.stdout).toBe("");
    expect(result.stderr).toBe("ath: pnpm was stopped by SIGKILL\nath: the install failed.\n");
    expect(log.map((entry) => entry.role)).toEqual(["native", "manager"]);
    expectRemovedHandoff(log);
  });

  it("passes Ctrl-C to the manager, reports failure, and does not refresh", async () => {
    const directory = freshDirectory("cancel");
    const logPath = join(directory, "log.jsonl");
    const pkg = makePackage(join(directory, "package"));
    const run = start(
      pkg.cli,
      ["--update"],
      cleanEnvironment({
        PATH: managerFolder(),
        FAKE_LOG: logPath,
        FAKE_PLAN: JSON.stringify(plan("npm")),
        FAKE_MANAGER: "wait",
        [NATIVE_EXECUTABLE_VARIABLE]: updateNative(join(directory, "native", "ath")),
      }),
    );
    await run.waitFor("manager ready");
    run.child.kill("SIGINT");
    expect(await run.ended).toEqual({ code: 1, signal: null });
    expect(run.stdout()).toBe("manager ready\n");
    expect(run.stderr()).toBe("ath: npm was stopped by SIGINT\nath: the install failed.\n");
    const log = readLog(logPath);
    expect(log.map((entry) => entry.role)).toEqual(["native", "manager"]);
    expectRemovedHandoff(log);
  });

  const forgedPlans: Array<[string, Record<string, unknown>]> = [
    ["another program", plan("npm", { program: "evil" })],
    ["another manager's program", plan("npm", { program: "pnpm" })],
    ["an extra argument", plan("npm", { args: [...EXPECTED_ARGS.npm, "--registry=http://example.invalid"] })],
    ["shell syntax in an argument", plan("npm", { args: ["install", "-g", "athleticstandard@latest; touch pwned"] })],
    ["shell syntax in the program", plan("npm", { program: "npm && touch pwned" })],
    ["an extra refresh variable", plan("npm", { refresh_environment: { ATH_UPDATE_PHASE: "refresh-skills", NODE_OPTIONS: "-r ./pwned" } })],
    ["an unknown field", plan("npm", { shell: "touch pwned" })],
  ];
  for (const [name, forged] of forgedPlans) {
    it(`refuses a plan with ${name} and runs nothing`, () => {
      const { result, log, cwd } = runUpdate({ FAKE_PLAN: JSON.stringify(forged) });
      expect(result.status).toBe(1);
      expect(result.stdout).toBe("");
      expect(result.stderr).toMatch(/^ath: the update plan is not (valid|one this ath writes).*nothing was installed\n$/);
      expect(log.map((entry) => entry.role)).toEqual(["native"]);
      expect(existsSync(join(cwd, "pwned"))).toBe(false);
      expectRemovedHandoff(log);
    });
  }

  it.skipIf(hostTarget === null)("refreshes with the executable from a replaced package folder", () => {
    // pnpm keeps each version in its own store folder and moves the global
    // link. The launcher loaded from the old folder must run the new executable.
    const directory = freshDirectory("replaced");
    const logPath = join(directory, "log.jsonl");
    const executableName = nativeExecutableName(hostTarget!);
    const oldPackage = makePackage(join(directory, "store", "old"));
    updateNative(join(oldPackage.root, "native", hostTarget!, executableName), "old");
    const newPackage = makePackage(join(directory, "store", "new"));
    updateNative(join(newPackage.root, "native", hostTarget!, executableName), "new");
    const link = join(directory, "global", "node_modules", "athleticstandard");
    mkdirSync(dirname(link), { recursive: true });
    symlinkSync(oldPackage.root, link);
    const command = join(directory, "global", "bin", "ath");
    mkdirSync(dirname(command), { recursive: true });
    symlinkSync(join(link, "dist", "cli.js"), command);

    const result = launch(
      command,
      ["--update"],
      cleanEnvironment({
        PATH: managerFolder(),
        FAKE_LOG: logPath,
        FAKE_PLAN: JSON.stringify(plan("pnpm")),
        FAKE_REPOINT: link,
        FAKE_REPOINT_TO: newPackage.root,
        FAKE_REMOVE: oldPackage.root,
      }),
    );
    expect(result.stderr).toBe("");
    expect(result.stdout).toBe("updated ath from 2.1.0 to 2.1.1.\nrefreshed skills by new\n");
    expect(result.status).toBe(0);
    const log = readLog(logPath);
    expect(log.map((entry) => `${entry.role}:${entry.name}`)).toEqual(["native:old", "manager:pnpm", "native:new"]);
    expect(log[0]!.paths).toBe(`${command}:${join(oldPackage.root, "dist", "cli.js")}`);
    expectRemovedHandoff(log);
    unlinkSync(command);
  });
});

/** The Microsoft C runtime command-line rules, as the started program parses them. */
function parseRuntimeArguments(line: string): string[] {
  const out: string[] = [];
  let index = 0;
  while (index < line.length) {
    while (line[index] === " " || line[index] === "\t") index += 1;
    if (index >= line.length) break;
    let current = "";
    let quoted = false;
    while (index < line.length && (quoted || (line[index] !== " " && line[index] !== "\t"))) {
      let backslashes = 0;
      while (line[index] === "\\") {
        backslashes += 1;
        index += 1;
      }
      if (line[index] === '"') {
        current += "\\".repeat(Math.floor(backslashes / 2));
        if (backslashes % 2 === 1) {
          current += '"';
        } else {
          quoted = !quoted;
        }
        index += 1;
      } else {
        current += "\\".repeat(backslashes);
        if (index < line.length && (quoted || (line[index] !== " " && line[index] !== "\t"))) {
          current += line[index];
          index += 1;
        }
      }
    }
    out.push(current);
  }
  return out;
}

// One cmd.exe pass outside quotes: a caret makes the next character literal.
function removeCarets(text: string): string {
  return text.replace(/\^(.)/gs, "$1");
}

// What cmd.exe would read as syntax in one pass: characters without a caret.
function unescapedSyntax(text: string): string {
  return text.replace(/\^./gs, "").replace(/[^!%^&()<>|"]/g, "");
}

/** The environment with PATH replaced, whatever case its name has. */
function withPath(env: NodeJS.ProcessEnv, value: string): NodeJS.ProcessEnv {
  const out: NodeJS.ProcessEnv = {};
  for (const [key, item] of Object.entries(env)) {
    if (key.toUpperCase() !== "PATH") {
      out[key] = item;
    }
  }
  out.Path = value;
  return out;
}

describe("Windows batch files", () => {
  const tricky = [
    "install",
    "-g",
    "athleticstandard@latest",
    "",
    "two words",
    'say "hi"',
    "trailing\\",
    "C:\\Program Files\\x\\",
    'back\\"slash',
    "a&b|c<d>e^f",
    "(group)",
    "%PATH%",
    "!delayed!",
    "tab\there",
  ];

  it("quotes each argument so two cmd.exe passes and the C runtime give it back", () => {
    for (const argument of tricky) {
      const quoted = quoteBatchArgument(argument);
      // In both cmd.exe passes every syntax character carries a caret, so
      // no quote mode starts and no operator or expansion takes effect.
      expect(unescapedSyntax(quoted), argument).toBe("");
      expect(unescapedSyntax(removeCarets(quoted)), argument).toBe("");
      expect(parseRuntimeArguments(removeCarets(removeCarets(quoted))), argument).toEqual([argument]);
    }
    expect(quoteBatchArgument("athleticstandard@latest")).toBe("athleticstandard@latest");
    expect(quoteBatchArgument("two words")).toBe('^^^"two^^^ words^^^"');
    expect(quoteBatchArgument("a&b")).toBe("a^^^&b");
    expect(() => quoteBatchArgument("line\nbreak")).toThrow(/line break/);
  });

  it("runs a batch file through cmd.exe with the path quoted and no shell string from data", () => {
    const file = "C:\\Program Files (x86)\\nodejs\\npm.cmd";
    expect(batchInvocation(file, EXPECTED_ARGS.npm, "C:\\Windows\\System32\\cmd.exe")).toEqual({
      file: "C:\\Windows\\System32\\cmd.exe",
      args: ["/d", "/v:off", "/s", "/c", `""${file}" install -g athleticstandard@latest"`],
      windowsVerbatimArguments: true,
    });
    expect(() => batchInvocation("C:\\%TEMP%\\npm.cmd", [], "cmd.exe")).toThrow(/safely/);
  });

  it("finds a manager on PATH with PATHEXT and never in the working folder", () => {
    const first = freshDirectory("path one");
    const second = freshDirectory("path two");
    const third = freshDirectory("path three");
    writeFileSync(join(first, "npm"), "");
    writeFileSync(join(first, "npm.ps1"), "");
    writeFileSync(join(first, "pnpm.js"), "");
    writeFileSync(join(second, "npm.cmd"), "");
    writeFileSync(join(second, "pnpm.cmd"), "");
    writeFileSync(join(third, "bun.exe"), "");
    const env = { Path: `.;relative;${first};"${second}";;${third}`, PathExt: ".COM;.EXE;.JS;.BAT;.CMD" };
    expect(findWindowsProgram("npm", env)).toBe(join(second, "npm.cmd"));
    expect(findWindowsProgram("pnpm", env)).toBe(join(second, "pnpm.cmd"));
    expect(findWindowsProgram("bun", env)).toBe(join(third, "bun.exe"));
    expect(findWindowsProgram("yarn", env)).toBeNull();

    const npm = managerInvocation("npm", EXPECTED_ARGS.npm, { ...env, ComSpec: "C:\\cmd.exe" }, "win32");
    expect(npm.file).toBe("C:\\cmd.exe");
    expect(npm.args.at(-1)).toBe(`""${join(second, "npm.cmd")}" install -g athleticstandard@latest"`);
    const bun = managerInvocation("bun", EXPECTED_ARGS.bun, env, "win32");
    expect(bun).toEqual({ file: join(third, "bun.exe"), args: EXPECTED_ARGS.bun, windowsVerbatimArguments: false });
    expect(() => managerInvocation("yarn", [], env, "win32")).toThrow(/not found on PATH/);
    expect(managerInvocation("npm", EXPECTED_ARGS.npm, {}, "linux")).toEqual({
      file: "npm",
      args: EXPECTED_ARGS.npm,
      windowsVerbatimArguments: false,
    });
  });

  it.skipIf(posix)("passes arguments through a real batch file intact", () => {
    const directory = freshDirectory("tools (x86) & co");
    writeFileSync(
      join(directory, "echo-args.cjs"),
      "process.stdout.write(JSON.stringify(process.argv.slice(2)));\n",
    );
    writeFileSync(join(directory, "npm.cmd"), `@"${process.execPath}" "%~dp0echo-args.cjs" %*\r\n`);
    const env = withPath(cleanEnvironment({}), directory);
    const args = [...EXPECTED_ARGS.npm, "two words", 'say "hi"', "a&b|c<d>e", ""];
    const invocation = managerInvocation("npm", args, env, "win32");
    const result = spawnSync(invocation.file, invocation.args, {
      env,
      encoding: "utf8",
      windowsVerbatimArguments: invocation.windowsVerbatimArguments,
    });
    expect(result.stderr).toBe("");
    expect(JSON.parse(result.stdout)).toEqual(args);
    expect(result.status).toBe(0);
  });
});

describe("the real native executable", () => {
  const executableName = process.platform === "win32" ? "ath.exe" : "ath";
  const configured = process.env[NATIVE_EXECUTABLE_VARIABLE];
  const built = ["debug", "release"]
    .map((profile) => join(repositoryRoot, "target", profile, executableName))
    .find((path) => existsSync(path));
  // CI names the tested binary. A checkout uses a Cargo build when one exists.
  const executable = configured !== undefined && configured !== "" ? configured : built;

  it.skipIf(executable === undefined)("prints its version and help through the launcher", () => {
    const pkg = makePackage(freshDirectory("real"));
    const env = cleanEnvironment({ [NATIVE_EXECUTABLE_VARIABLE]: executable! });
    const version = launch(pkg.cli, ["--version"], env);
    expect(version.stderr).toBe("");
    expect(version.stdout).toBe(`${packageVersion}\n`);
    expect(version.status).toBe(0);
    const help = launch(pkg.cli, ["--help"], env);
    expect(help.status).toBe(0);
    expect(help.stdout).toContain("--update");
  });
});
