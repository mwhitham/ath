import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { isMappingPath, resolveMappingFile } from "../src/skill.js";

describe("the --mapping value", () => {
  it("reads a Windows path as a path", () => {
    expect(isMappingPath("C:\\mappings\\mine.md")).toBe(true);
    expect(isMappingPath("mappings\\mine.md")).toBe(true);
    expect(isMappingPath("c:mine.md")).toBe(true);
  });

  it("reads a POSIX path as a path and a bare word as a name", () => {
    expect(isMappingPath("./mine.md")).toBe(true);
    expect(isMappingPath("/tmp/mine.md")).toBe(true);
    expect(isMappingPath("open-wearables")).toBe(false);
    expect(isMappingPath("my-source")).toBe(false);
  });

  it("names a missing path and a missing name differently", () => {
    const cwd = mkdtempSync(join(tmpdir(), "ath-mapping-path-"));
    expect(() => resolveMappingFile("./missing.md", cwd)).toThrow(
      `no mapping file at ${join(cwd, "missing.md")}`,
    );
    expect(() => resolveMappingFile("missing", cwd)).toThrow("no mapping file named missing");
  });

  it("finds the shipped file by name", () => {
    const cwd = mkdtempSync(join(tmpdir(), "ath-mapping-path-"));
    expect(resolveMappingFile("open-wearables", cwd).endsWith(join("mappings", "open-wearables.md"))).toBe(
      true,
    );
  });
});
