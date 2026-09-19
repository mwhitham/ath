/**
 * Installing the skill (D49 and D41 on the mechanism, D79 on where it goes).
 *
 * `skill/` is a directory copied whole, not a path to one file. It always goes to
 * `.agents/skills/`, which Claude Code, Cursor, Codex, and Gemini CLI all read, and
 * also into any harness folder already present. Each copy carries its version in
 * the `SKILL.md` frontmatter, so a copy left behind by an older `ath` can be told
 * from the shipped one and refreshed with `ath skill install`.
 */
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** The folder name the skill installs under, inside each host's skills directory. */
export const SKILL_NAME = "athletic-standard";

/** Always written. The cross-client location every major harness reads. */
export const ALWAYS_DIR = ".agents";

/** Written too, when the folder is already here. */
export const AGENT_DIRS = [".claude", ".cursor", ".codex", ".gemini", ".github"] as const;

export type SkillState = "current" | "stale" | "missing";

export interface SkillCopy {
  /** The installed folder, absolute. */
  path: string;
  /** The version in the installed copy's frontmatter, or null when there is no copy. */
  installed: string | null;
  /** The version this `ath` ships. */
  shipped: string;
  state: SkillState;
}

/**
 * Where the skill ships from.
 *
 * `skill/` sits beside `src/` in the repository and beside `dist/` in the published
 * package, so one step up from this module finds it either way.
 */
export function skillSource(): string {
  return resolve(dirname(fileURLToPath(import.meta.url)), "..", "skill");
}

/** The frontmatter of a SKILL.md: top-level `key: value` pairs and one nested level. */
export function readFrontmatter(text: string): Record<string, unknown> {
  const match = /^---\r?\n([\s\S]*?)\r?\n---/.exec(text);
  if (!match) return {};
  const out: Record<string, unknown> = {};
  let nested: Record<string, string> | null = null;
  for (const raw of match[1]!.split(/\r?\n/)) {
    if (!raw.trim()) continue;
    const indented = /^\s+/.test(raw);
    const [, key, value] = /^\s*([A-Za-z_][\w-]*):\s*(.*)$/.exec(raw) ?? [];
    if (!key) continue;
    if (indented && nested) {
      nested[key] = unquote(value ?? "");
    } else if ((value ?? "") === "") {
      nested = {};
      out[key] = nested;
    } else {
      nested = null;
      out[key] = unquote(value!);
    }
  }
  return out;
}

function unquote(value: string): string {
  const v = value.trim();
  return /^".*"$/.test(v) || /^'.*'$/.test(v) ? v.slice(1, -1) : v;
}

/** What a copy reads as when its SKILL.md predates the version field (before 0.4.0). */
export const UNVERSIONED = "unversioned";

/**
 * The version in a SKILL.md's frontmatter. Null when there is no SKILL.md;
 * `UNVERSIONED` when there is one without a version, which only an older `ath` wrote.
 */
export function skillVersion(skillDir: string): string | null {
  const file = join(skillDir, "SKILL.md");
  if (!existsSync(file)) return null;
  const meta = readFrontmatter(readFileSync(file, "utf8")).metadata;
  const version = meta && typeof meta === "object" ? (meta as Record<string, string>).version : undefined;
  return typeof version === "string" && version ? version : UNVERSIONED;
}

/** The version this `ath` ships, read from `skill/SKILL.md`. */
export function shippedSkillVersion(): string {
  return skillVersion(skillSource()) ?? UNVERSIONED;
}

/** Every folder the skill would be written to in `cwd`: `.agents` always, the rest when present. */
export function skillTargets(cwd: string): string[] {
  const targets = [join(cwd, ALWAYS_DIR, "skills", SKILL_NAME)];
  for (const agent of AGENT_DIRS) {
    if (existsSync(join(cwd, agent))) targets.push(join(cwd, agent, "skills", SKILL_NAME));
  }
  return targets;
}

/**
 * Copy the skill into `.agents/skills/` and every harness folder present in `cwd`.
 *
 * Returns the paths written. An existing copy is replaced whole, so a file the old
 * version shipped and the new one does not is gone afterwards.
 */
export function installSkill(cwd: string): string[] {
  const source = skillSource();
  if (!existsSync(source)) return [];

  const written: string[] = [];
  for (const destination of skillTargets(cwd)) {
    mkdirSync(dirname(destination), { recursive: true });
    rmSync(destination, { recursive: true, force: true });
    cpSync(source, destination, { recursive: true });
    written.push(destination);
  }
  return written;
}

/** Each place the skill would be installed in `cwd`, and whether the copy there is current. */
export function skillStatus(cwd: string): SkillCopy[] {
  const shipped = shippedSkillVersion();
  return skillTargets(cwd).map((path) => {
    const installed = skillVersion(path);
    const state: SkillState =
      installed === null ? "missing" : installed === shipped ? "current" : "stale";
    return { path, installed, shipped, state };
  });
}

/**
 * One line for stderr when an installed copy is older than the shipped one, else
 * null. Nothing is said when no copy exists: running without an agent is normal.
 */
export function staleSkillWarning(cwd: string): string | null {
  const stale = skillStatus(cwd).filter((c) => c.state === "stale");
  if (stale.length === 0) return null;
  const first = stale[0]!;
  const where = join(relativeAgentDir(cwd, first.path), "skills");
  const more = stale.length > 1 ? ` and ${stale.length - 1} more` : "";
  const age =
    first.installed === UNVERSIONED ? `is older than ${first.shipped}` : `is from ${first.installed}`;
  return `ath: the agent skill in ${where}${more} ${age}; run ath skill install`;
}

function relativeAgentDir(cwd: string, skillPath: string): string {
  const rel = skillPath.slice(cwd.length).replace(/^[\\/]+/, "");
  return rel.split(/[\\/]/)[0] ?? rel;
}
