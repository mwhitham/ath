import { describe, expect, it } from "vitest";
import { coverageOf, daysBetween, renderCoverage, RULES } from "../src/coverage.js";

describe("coverageOf", () => {
  it("counts observations, the window, and the days actually present", () => {
    const c = coverageOf(
      [
        { at: "2026-08-01T06:00:00Z" },
        { at: "2026-08-01T22:00:00Z" },
        { at: "2026-08-05T06:00:00Z" },
      ],
      "whoop-1",
      RULES.perSource,
    )!;
    expect(c).toMatchObject({
      n: 3,
      from: "2026-08-01",
      to: "2026-08-05",
      days_present: 2,
      days_expected: 5,
      source: "whoop-1",
    });
  });

  it("finds the ends of the window by instant, not by comparing day strings", () => {
    // 2026-08-30T20:00:00-07:00 is 2026-08-31T03:00Z, so it is the later reading
    // while naming the earlier local day. Sorting the strings gets this backwards.
    const c = coverageOf(
      [{ at: "2026-08-31T01:00:00Z" }, { at: "2026-08-30T20:00:00-07:00" }],
      "whoop-1",
      RULES.perSource,
    )!;
    expect(c.to).toBe("2026-08-30");
  });

  it("has nothing to say about nothing", () => {
    expect(coverageOf([], "whoop-1", RULES.perSource)).toBeNull();
  });

  it("counts both ends of a window as days", () => {
    expect(daysBetween("2026-08-01", "2026-08-01")).toBe(1);
    expect(daysBetween("2026-08-01", "2026-08-03")).toBe(3);
  });

  it("names the rule and hides the gap only when there is none", () => {
    const full = coverageOf(
      [{ at: "2026-08-01T06:00:00Z" }, { at: "2026-08-02T06:00:00Z" }],
      "whoop-1",
      RULES.sleepDuration,
    )!;
    expect(renderCoverage(full)).toBe(
      "n=2, 2026-08-01 → 2026-08-02, whoop-1 — actual sleep, excluding time awake — not time in bed",
    );

    const gappy = coverageOf(
      [{ at: "2026-08-01T06:00:00Z" }, { at: "2026-08-05T06:00:00Z" }],
      "whoop-1",
      RULES.perSource,
    )!;
    expect(renderCoverage(gappy)).toContain("2/5 days");
  });
});
