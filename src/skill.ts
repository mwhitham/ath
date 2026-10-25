/**
 * Installing the skill (D49 and D41 on the mechanism, D79 on where it goes).
 *
 * `skill/` is a directory copied into `.agents/skills/`, which Claude Code, Cursor,
 * Codex, and Gemini CLI read, and also into any harness folder already present.
 * A file that is already installed is left alone. A new file is added.
 */
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** The folder name the skill installs under, inside each host's skills directory. */
export const SKILL_NAME = "athletic-standard";

/** Always written. The cross-client location every major harness reads. */
export const ALWAYS_DIR = ".agents";

/** Written too, when the folder is already here. */
export const AGENT_DIRS = [".claude", ".cursor", ".codex", ".gemini", ".github"] as const;

export type SkillState = "installed" | "missing";

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

function copyMissingFiles(source: string, destination: string): number {
  let added = 0;
  for (const entry of readdirSync(source)) {
    const from = join(source, entry);
    const to = join(destination, entry);
    if (statSync(from).isDirectory()) {
      mkdirSync(to, { recursive: true });
      added += copyMissingFiles(from, to);
      continue;
    }
    if (existsSync(to)) continue;
    cpSync(from, to);
    added += 1;
  }
  return added;
}

/**
 * Copy skill files that are not already installed.
 *
 * Returns the destination folders where at least one file was added.
 * An existing file is not replaced.
 */
export function installSkill(cwd: string): string[] {
  const source = skillSource();
  if (!existsSync(source)) return [];

  const written: string[] = [];
  for (const destination of skillTargets(cwd)) {
    mkdirSync(destination, { recursive: true });
    if (copyMissingFiles(source, destination) > 0) written.push(destination);
  }
  return written;
}

/** Each place the skill would be installed in `cwd`, and whether the copy there is current. */
export function skillStatus(cwd: string): SkillCopy[] {
  const shipped = shippedSkillVersion();
  return skillTargets(cwd).map((path) => {
    const installed = skillVersion(path);
    const state: SkillState = installed === null ? "missing" : "installed";
    return { path, installed, shipped, state };
  });
}

/** Installed skill files are not replaced, so there is no stale-skill warning. */
export function staleSkillWarning(_cwd: string): string | null {
  return null;
}
