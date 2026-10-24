import { describe, expect, it } from "vitest";
import { importOura } from "../src/import/oura.js";
import type { SourceBook } from "../src/import/merge.js";

const sources: SourceBook = {
  forWriter: () => "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02",
  manual: () => "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e03",
};

describe("Oura activity met column", () => {
  it("counts a JSON met cell as skipped activity data", () => {
    const csv = "day,met\n2026-01-01,\"{\\\"samples\\\":1}\"\n";
    const payload = importOura(new Map([["oura_activity.csv", csv]]), sources);
    expect(payload.skipped.get("oura_activity.csv: activity column met")).toBe(1);
  });
});
