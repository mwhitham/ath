import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { buildProgram } from "../src/program.js";
import { ATHLETIC_STANDARD_VERSION } from "../src/schema.js";
import {
  AGENT_DIRS,
  ALWAYS_DIR,
  installSkill,
  readFrontmatter,
  SKILL_NAME,
  skillSource,
  skillStatus,
  staleSkillWarning,
} from "../src/skill.js";
import { commandPaths, renderCliReference } from "../src/skillcli.js";

const here = dirname(fileURLToPath(import.meta.url));
const CLI = resolve(here, "../src/cli.ts");
const TSX = resolve(here, "../node_modules/.bin/tsx");

function ath(args: string[], cwd: string): { stdout: string; stderr: string; code: number } {
  const res = spawnSync(TSX, [CLI, ...args], { cwd, encoding: "utf8" });
  return { stdout: res.stdout ?? "", stderr: res.stderr ?? "", code: res.status ?? 1 };
}

/** A working directory with the named agent folders already in it. */
function dirWithAgents(...agents: string[]): string {
  const dir = mkdtempSync(join(tmpdir(), "ath-skill-"));
  for (const agent of agents) mkdirSync(join(dir, agent));
  return dir;
}

const skillFile = (dir: string, agent: string, file: string) =>
  join(dir, agent, "skills", SKILL_NAME, file);

/** Rewrite one installed copy's frontmatter so it reads as from an older ath. */
function ageCopy(dir: string, agent: string, version: string): void {
  const file = skillFile(dir, agent, "SKILL.md");
  const text = readFileSync(file, "utf8").replace(/version: "[^"]*"/, `version: "${version}"`);
  writeFileSync(file, text);
}

const skillText = () => readFileSync(join(skillSource(), "SKILL.md"), "utf8");

describe("the skill", () => {
  it("names the format version it expects", () => {
    expect(skillText()).toContain(ATHLETIC_STANDARD_VERSION);
  });

  it("is one skill with its detail in references/, loaded when needed (D49, D80)", () => {
    for (const file of ["cli.md", "format.md", "logging.md", "predicting.md", "grading.md"]) {
      expect(existsSync(join(skillSource(), "references", file)), file).toBe(true);
    }
    for (const file of ["cli.md", "format.md", "logging.md", "predicting.md", "grading.md"]) {
      expect(skillText(), `SKILL.md should point at references/${file}`).toContain(
        `(references/${file})`,
      );
    }
  });

  it("stays short enough to load on every activation", () => {
    expect(skillText().split("\n").length).toBeLessThan(500);
  });

  it("carries the workout naming procedure and a reference list (D54)", () => {
    const text = readFileSync(join(skillSource(), "references", "logging.md"), "utf8");
    expect(text).toContain("--benchmark");
    expect(text).toContain("near variation");
    expect(text).toMatch(/\bfran\b/);
    expect(text).toMatch(/\bmurph\b/);
    expect(text).toMatch(/hyrox/i);
  });

  it("leaves to the tools what the tools enforce (D46)", () => {
    expect(skillText()).toContain("What you do not need to hold, because the tools do");
    expect(skillText()).toContain("never pooled");
  });
});

describe("the frontmatter follows the Agent Skills shape", () => {
  const meta = readFrontmatter(skillText());

  it("names the folder it installs under", () => {
    expect(meta.name).toBe(SKILL_NAME);
  });

  it("describes what it does and when, in the third person, under the length limit", () => {
    const description = meta.description as string;
    expect(description.length).toBeLessThan(1024);
    expect(description).not.toMatch(/^Use\b/);
    for (const trigger of ["workout", "HRV", "sleep", "soreness", "Fran", "backtest", "Apple Health", "WHOOP", "Oura", ".ath.json"]) {
      expect(description, trigger).toContain(trigger);
    }
  });

  it("carries a license, compatibility, and the shipped version", () => {
    expect(meta.license).toBe("MIT");
    expect(meta.compatibility).toContain("ath");
    expect((meta.metadata as Record<string, string>).version).toBe(ATHLETIC_STANDARD_VERSION);
  });
});

describe("the command reference is generated from the CLI's own help (D80)", () => {
  it("is committed as generated — regenerate with pnpm generate:skill-cli, do not hand-edit", () => {
    const committed = readFileSync(join(skillSource(), "references", "cli.md"), "utf8");
    expect(committed).toBe(renderCliReference(buildProgram()));
  });

  it("covers every command, including subcommands", () => {
    const reference = readFileSync(join(skillSource(), "references", "cli.md"), "utf8");
    const paths = commandPaths(buildProgram());
    expect(paths).toContain("ath key set");
    expect(paths).toContain("ath skill install");
    for (const path of paths) expect(reference, path).toContain(`## ${path}`);
  });

  it("has every top-level command in the SKILL.md table", () => {
    const top = new Set(commandPaths(buildProgram()).map((p) => p.split(" ")[1]!));
    expect(top.size).toBeGreaterThanOrEqual(14);
    for (const name of top) {
      expect(skillText(), `SKILL.md should list ath ${name}`).toMatch(new RegExp(`\\| \`ath ${name}\\b`));
    }
  });
});

describe("ath init installs it (D79)", () => {
  it("always writes to .agents/skills, even with no agent folder here", () => {
    const dir = dirWithAgents();
    const res = ath(["init", "-y"], dir);
    expect(res.code).toBe(0);
    expect(existsSync(skillFile(dir, ALWAYS_DIR, "SKILL.md"))).toBe(true);
    expect(existsSync(skillFile(dir, ALWAYS_DIR, "references/cli.md"))).toBe(true);
    expect(res.stdout).toContain(join(ALWAYS_DIR, "skills", SKILL_NAME));
    expect(res.stdout).toContain("Claude Code, Cursor, Codex, and Gemini CLI");
    for (const agent of AGENT_DIRS) expect(existsSync(join(dir, agent))).toBe(false);
  });

  it("also copies into every harness folder it finds", () => {
    const dir = dirWithAgents(...AGENT_DIRS);
    const res = ath(["init", "-y"], dir);
    expect(res.code).toBe(0);
    for (const agent of [ALWAYS_DIR, ...AGENT_DIRS]) {
      expect(existsSync(skillFile(dir, agent, "SKILL.md")), agent).toBe(true);
      expect(existsSync(skillFile(dir, agent, "references/grading.md")), agent).toBe(true);
      expect(res.stdout).toContain(join(agent, "skills", SKILL_NAME));
    }
  });

  it("skips the install when asked to", () => {
    const dir = dirWithAgents(".cursor");
    const res = ath(["init", "-y", "--no-skill"], dir);
    expect(res.code).toBe(0);
    expect(existsSync(join(dir, ALWAYS_DIR))).toBe(false);
    expect(existsSync(skillFile(dir, ".cursor", "SKILL.md"))).toBe(false);
  });

  it("reports what it installed under --json", () => {
    const dir = dirWithAgents(".claude");
    const res = ath(["init", "-y", "--json"], dir);
    const out = JSON.parse(res.stdout);
    expect(out.skill_installed).toEqual([
      join(dir, ALWAYS_DIR, "skills", SKILL_NAME),
      join(dir, ".claude", "skills", SKILL_NAME),
    ]);
  });
});

describe("ath skill (D79)", () => {
  it("reports each copy as current, stale, or missing", () => {
    const dir = dirWithAgents(".codex", ".gemini");
    installSkill(dir);
    ageCopy(dir, ".codex", "0.3.0");
    writeFileSync(skillFile(dir, ".gemini", "SKILL.md"), "");
    // A .github folder that appears after init has no copy yet.
    mkdirSync(join(dir, ".github"));

    const byAgent = Object.fromEntries(
      skillStatus(dir).map((c) => [c.path.slice(dir.length + 1).split("/")[0], c]),
    );
    expect(byAgent[ALWAYS_DIR]!.state).toBe("current");
    expect(byAgent[".codex"]).toMatchObject({ state: "stale", installed: "0.3.0" });
    expect(byAgent[".gemini"]).toMatchObject({ state: "stale", installed: "unversioned" });
    expect(byAgent[".github"]).toMatchObject({ state: "missing", installed: null });

    const res = ath(["skill"], dir);
    expect(res.code).toBe(0);
    expect(res.stdout).toContain("current");
    expect(res.stdout).toContain("stale: 0.3.0 installed");
    expect(res.stdout).toContain("missing");
    expect(res.stdout).toContain("ath skill install");

    const json = JSON.parse(ath(["skill", "--json"], dir).stdout);
    expect(json.shipped).toBe(ATHLETIC_STANDARD_VERSION);
    expect(json.copies.map((c: { state: string }) => c.state).sort()).toEqual([
      "current",
      "missing",
      "stale",
      "stale",
    ]);
  });

  it("refreshes every copy with ath skill install", () => {
    const dir = dirWithAgents(".codex");
    installSkill(dir);
    ageCopy(dir, ".codex", "0.3.0");
    writeFileSync(skillFile(dir, ".codex", "left-over.md"), "from an older version\n");

    const res = ath(["skill", "install"], dir);
    expect(res.code).toBe(0);
    expect(res.stdout).toContain(join(".codex", "skills", SKILL_NAME));
    expect(skillStatus(dir).every((c) => c.state === "current")).toBe(true);
    expect(existsSync(skillFile(dir, ".codex", "left-over.md"))).toBe(false);

    const json = JSON.parse(ath(["skill", "install", "--json"], dir).stdout);
    expect(json.skill_installed).toHaveLength(2);
  });
});

describe("the stale warning (D79)", () => {
  it("is one stderr line on an ordinary command when a copy is stale", () => {
    const dir = dirWithAgents(".cursor");
    expect(ath(["init", "-y"], dir).code).toBe(0);
    ageCopy(dir, ".cursor", "0.3.0");

    expect(staleSkillWarning(dir)).toBe(
      "ath: the agent skill in .cursor/skills is from 0.3.0; run ath skill install",
    );
    const res = ath(["stats"], dir);
    expect(res.code).toBe(0);
    expect(res.stderr.trim().split("\n")).toEqual([
      "ath: the agent skill in .cursor/skills is from 0.3.0; run ath skill install",
    ]);
  });

  it("stays out of --json output, init, and skill itself", () => {
    const dir = dirWithAgents(".cursor");
    expect(ath(["init", "-y"], dir).code).toBe(0);
    ageCopy(dir, ".cursor", "0.3.0");

    expect(ath(["stats", "--json"], dir).stderr).toBe("");
    expect(ath(["skill"], dir).stderr).toBe("");
    expect(ath(["init", "-y", "--file", "other.ath.json", "--no-skill"], dir).stderr).toBe("");
  });

  it("says nothing when no copy is installed or every copy is current", () => {
    const none = dirWithAgents();
    expect(ath(["init", "-y", "--no-skill"], none).code).toBe(0);
    expect(staleSkillWarning(none)).toBeNull();
    expect(ath(["stats"], none).stderr).toBe("");

    const current = dirWithAgents(".claude");
    expect(ath(["init", "-y"], current).code).toBe(0);
    expect(ath(["stats"], current).stderr).toBe("");
  });
});
