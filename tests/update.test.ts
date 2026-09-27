import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { ATHLETIC_STANDARD_VERSION } from "../src/schema.js";
import { ALWAYS_DIR, installSkill, skillVersion, SKILL_NAME } from "../src/skill.js";
import {
  candidatePaths,
  chooseInstall,
  classifyInstallPath,
  compareVersions,
  runUpdate,
  skillUpdateLine,
  type Spawn,
} from "../src/update.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");

function ath(args: string[], cwd: string): { stdout: string; stderr: string; code: number } {
  const res = spawnSync(TSX, [CLI, ...args], { cwd, encoding: "utf8" });
  return { stdout: res.stdout ?? "", stderr: res.stderr ?? "", code: res.status ?? 1 };
}

const skillFile = (dir: string, agent: string) => join(dir, agent, "skills", SKILL_NAME, "SKILL.md");

function ageCopy(dir: string, agent: string, version: string): void {
  const file = skillFile(dir, agent);
  const text = readFileSync(file, "utf8").replace(/version: "[^"]*"/, `version: "${version}"`);
  writeFileSync(file, text);
}

function capture() {
  const lines: string[] = [];
  const errors: string[] = [];
  return {
    lines,
    errors,
    log: (line: string) => lines.push(line),
    error: (line: string) => errors.push(line),
  };
}

describe("classifyInstallPath", () => {
  it("recognises a global npm, pnpm, or bun install", () => {
    expect(classifyInstallPath("/usr/local/lib/node_modules/athleticstandard").kind).toBe("npm");
    expect(
      classifyInstallPath("/home/user/.nvm/versions/node/v22.0.0/lib/node_modules/athleticstandard").kind,
    ).toBe("npm");
    expect(
      classifyInstallPath("/home/user/.local/share/pnpm/global/5/node_modules/athleticstandard").kind,
    ).toBe("pnpm");
    expect(
      classifyInstallPath(
        "/Users/me/Library/pnpm/global/5/.pnpm/athleticstandard@0.4.1/node_modules/athleticstandard",
      ).kind,
    ).toBe("pnpm");
    expect(classifyInstallPath("/home/user/.bun/install/global/node_modules/athleticstandard").kind).toBe(
      "bun",
    );
  });

  it("recognises the same layouts when the path uses backslashes", () => {
    expect(
      classifyInstallPath("C:\\Users\\me\\AppData\\Roaming\\npm\\node_modules\\athleticstandard").kind,
    ).toBe("npm");
    expect(
      classifyInstallPath("C:\\Users\\me\\.bun\\install\\global\\node_modules\\athleticstandard").kind,
    ).toBe("bun");
  });

  it("recognises a fetched copy and does not treat it as a global install", () => {
    expect(classifyInstallPath("/home/user/.npm/_npx/abc/node_modules/athleticstandard").kind).toBe("npx");
    expect(
      classifyInstallPath("/home/user/Library/Caches/pnpm/dlx/abc/node_modules/athleticstandard").kind,
    ).toBe("pnpm-dlx");
    expect(classifyInstallPath("/home/user/.bun/install/cache/abc/node_modules/athleticstandard").kind).toBe(
      "bunx",
    );
  });

  it("leaves a project copy alone", () => {
    expect(classifyInstallPath("/home/user/proj/node_modules/athleticstandard").kind).toBe("local");
    expect(
      classifyInstallPath(
        "/home/user/proj/node_modules/.pnpm/athleticstandard@0.4.1/node_modules/athleticstandard",
      ).kind,
    ).toBe("local");
  });

  it("asks npm, pnpm, and bun for the latest package", () => {
    expect(classifyInstallPath("/usr/local/lib/node_modules/athleticstandard")).toMatchObject({
      command: "npm",
      args: ["install", "-g", "athleticstandard@latest"],
    });
    expect(
      classifyInstallPath("/home/user/.local/share/pnpm/global/5/node_modules/athleticstandard"),
    ).toMatchObject({
      command: "pnpm",
      args: ["add", "-g", "athleticstandard@latest"],
    });
    expect(classifyInstallPath("/home/user/.bun/install/global/node_modules/athleticstandard")).toMatchObject({
      command: "bun",
      args: ["add", "-g", "athleticstandard@latest"],
    });
  });
});

describe("chooseInstall", () => {
  it("prefers the global path when the real files live in a store", () => {
    const store = "/home/user/.local/share/pnpm/store/v10/links/athleticstandard/node_modules/athleticstandard";
    const global = "/home/user/.local/share/pnpm/global/5/node_modules/athleticstandard";
    expect(chooseInstall([store]).kind).toBe("local");
    expect(chooseInstall([store, global]).kind).toBe("pnpm");
  });

  it("follows a bin symlink to the global package", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    const pkg = join(dir, "lib", "node_modules", "athleticstandard");
    mkdirSync(join(pkg, "dist"), { recursive: true });
    writeFileSync(join(pkg, "package.json"), JSON.stringify({ name: "athleticstandard" }));
    writeFileSync(join(pkg, "dist", "cli.js"), "");
    const bin = join(dir, "bin");
    mkdirSync(bin);
    symlinkSync(join(pkg, "dist", "cli.js"), join(bin, "ath"));

    expect(chooseInstall(candidatePaths(join(bin, "ath"))).kind).toBe("npm");
  });
});

describe("compareVersions", () => {
  it("orders dotted versions", () => {
    expect(compareVersions("0.4.1", "0.4.2")).toBe(-1);
    expect(compareVersions("0.4.2", "0.4.2")).toBe(0);
    expect(compareVersions("0.4.3", "0.4.2")).toBe(1);
  });
});

describe("runUpdate", () => {
  const npmPath = "/usr/local/lib/node_modules/athleticstandard";

  it("does not install when this copy is not global, and does not write a skill", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    installSkill(dir);
    ageCopy(dir, ALWAYS_DIR, "0.0.1");
    const out = capture();
    const spawn: Spawn = () => {
      throw new Error("spawn");
    };

    return runUpdate({
      packagePaths: ["/home/user/proj/node_modules/athleticstandard"],
      currentVersion: ATHLETIC_STANDARD_VERSION,
      cwd: dir,
      fetchLatest: async () => "9.9.9",
      spawn,
      ...out,
    }).then((code) => {
      expect(code).toBe(1);
      expect(out.errors.join("\n")).toContain("not installed globally");
      expect(skillVersion(join(dir, ALWAYS_DIR, "skills", SKILL_NAME))).toBe("0.0.1");
    });
  });

  it("says when this ath is already the published version and does not run the installer", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    const out = capture();
    const code = await runUpdate({
      packagePaths: [npmPath],
      currentVersion: ATHLETIC_STANDARD_VERSION,
      cwd: dir,
      fetchLatest: async () => ATHLETIC_STANDARD_VERSION,
      spawn: () => {
        throw new Error("spawn");
      },
      ...out,
    });
    expect(code).toBe(0);
    expect(out.lines).toContain(`ath ${ATHLETIC_STANDARD_VERSION} is the current version.`);
    expect(existsSync(join(dir, ALWAYS_DIR))).toBe(false);
  });

  it("reinstalls with the tool that installed this copy", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    const out = capture();
    const calls: string[][] = [];
    const code = await runUpdate({
      packagePaths: ["/home/user/.bun/install/global/node_modules/athleticstandard"],
      currentVersion: "0.4.1",
      cwd: dir,
      fetchLatest: async () => "9.9.9",
      spawn: (command, args) => {
        calls.push([command, ...args]);
        return 0;
      },
      ...out,
    });
    expect(code).toBe(0);
    expect(calls).toEqual([["bun", "add", "-g", "athleticstandard@latest"]]);
    expect(out.lines).toContain("updated ath from 0.4.1 to 9.9.9.");
  });

  it("does not replace the skill when the install fails", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    installSkill(dir);
    ageCopy(dir, ALWAYS_DIR, "0.0.1");
    const out = capture();
    const code = await runUpdate({
      packagePaths: [npmPath],
      currentVersion: "0.4.1",
      cwd: dir,
      fetchLatest: async () => "9.9.9",
      spawn: () => 1,
      ...out,
    });
    expect(code).toBe(1);
    expect(out.errors.join("\n")).toContain("install failed");
    expect(skillVersion(join(dir, ALWAYS_DIR, "skills", SKILL_NAME))).toBe("0.0.1");
  });

  it("stops when the registry cannot be reached", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    installSkill(dir);
    ageCopy(dir, ALWAYS_DIR, "0.0.1");
    const out = capture();
    const code = await runUpdate({
      packagePaths: [npmPath],
      currentVersion: ATHLETIC_STANDARD_VERSION,
      cwd: dir,
      fetchLatest: async () => {
        throw new Error("offline");
      },
      spawn: () => {
        throw new Error("spawn");
      },
      ...out,
    });
    expect(code).toBe(1);
    expect(out.errors.join("\n")).toContain("registry could not be reached");
    expect(skillVersion(join(dir, ALWAYS_DIR, "skills", SKILL_NAME))).toBe("0.0.1");
  });

  it("says when this ath is ahead of the published version", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    const out = capture();
    const code = await runUpdate({
      packagePaths: [npmPath],
      currentVersion: "0.4.3",
      cwd: dir,
      fetchLatest: async () => "0.4.2",
      spawn: () => {
        throw new Error("spawn");
      },
      ...out,
    });
    expect(code).toBe(0);
    expect(out.lines.join("\n")).toContain("ahead of the published 0.4.2");
  });

  it("replaces a skill that is already in the folder you ran it from", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    mkdirSync(join(dir, ".cursor"));
    installSkill(dir);
    ageCopy(dir, ALWAYS_DIR, "0.0.1");
    ageCopy(dir, ".cursor", "0.0.1");
    const out = capture();
    const code = await runUpdate({
      packagePaths: [npmPath],
      currentVersion: ATHLETIC_STANDARD_VERSION,
      cwd: dir,
      fetchLatest: async () => ATHLETIC_STANDARD_VERSION,
      spawn: () => {
        throw new Error("spawn");
      },
      ...out,
    });
    expect(code).toBe(0);
    expect(skillVersion(join(dir, ALWAYS_DIR, "skills", SKILL_NAME))).toBe(ATHLETIC_STANDARD_VERSION);
    expect(skillVersion(join(dir, ".cursor", "skills", SKILL_NAME))).toBe(ATHLETIC_STANDARD_VERSION);
    expect(out.lines.join("\n")).toContain(skillUpdateLine(dir, [
      join(dir, ALWAYS_DIR, "skills", SKILL_NAME),
      join(dir, ".cursor", "skills", SKILL_NAME),
    ]));
  });

  it("leaves the folder alone when no skill is there", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    mkdirSync(join(dir, ".cursor"));
    const out = capture();
    const code = await runUpdate({
      packagePaths: ["/home/user/.npm/_npx/abc/node_modules/athleticstandard"],
      currentVersion: ATHLETIC_STANDARD_VERSION,
      cwd: dir,
      fetchLatest: async () => {
        throw new Error("should not fetch");
      },
      spawn: () => {
        throw new Error("spawn");
      },
      ...out,
    });
    expect(code).toBe(0);
    expect(out.lines.join("\n")).toContain("started with npx");
    expect(out.lines.join("\n")).not.toContain("skill");
    expect(existsSync(join(dir, ALWAYS_DIR))).toBe(false);
    expect(existsSync(join(dir, ".cursor", "skills"))).toBe(false);
  });

  it("replaces the skill from an npx launch without installing a global copy", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    installSkill(dir);
    ageCopy(dir, ALWAYS_DIR, "0.0.1");
    const out = capture();
    const code = await runUpdate({
      packagePaths: ["/home/user/.npm/_npx/abc/node_modules/athleticstandard"],
      currentVersion: ATHLETIC_STANDARD_VERSION,
      cwd: dir,
      spawn: () => {
        throw new Error("spawn");
      },
      ...out,
    });
    expect(code).toBe(0);
    expect(skillVersion(join(dir, ALWAYS_DIR, "skills", SKILL_NAME))).toBe(ATHLETIC_STANDARD_VERSION);
    expect(out.lines.join("\n")).toContain("updated the skill in");
  });
});

describe("ath --update", () => {
  it("refuses when this ath is the one in the repository", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    const res = ath(["--update"], dir);
    expect(res.code).toBe(1);
    expect(res.stderr).toContain("not installed globally");
    expect(existsSync(join(dir, ALWAYS_DIR))).toBe(false);
  });

  it("is not a flag on other commands", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    const res = ath(["log", "--update"], dir);
    expect(res.code).toBe(1);
    expect(res.stderr).toContain("unknown option");
  });

  it("is listed in the help", () => {
    const dir = mkdtempSync(join(tmpdir(), "ath-update-"));
    const res = ath(["--help"], dir);
    expect(res.code).toBe(0);
    expect(res.stdout).toContain("--update");
  });
});
