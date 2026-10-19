import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { rmssdFromIntervals } from "../src/hrv.js";
import { PointMeasurementRecord } from "../src/v1/schema.js";

const casesRoot = join(import.meta.dirname, "..", "spec", "conformance", "cases");

function loadManifest(caseId: string): Record<string, unknown> {
  const path = join(casesRoot, caseId, "manifest.json");
  return JSON.parse(readFileSync(path, "utf8")) as Record<string, unknown>;
}

describe("spec/conformance vectors", () => {
  it("two-tier-wall-reject", () => {
    const manifest = loadManifest("two-tier-wall-reject");
    const record = manifest.record as Record<string, unknown>;
    const result = PointMeasurementRecord.safeParse(record);
    expect(result.success).toBe(false);
    const expectBlock = manifest.expect as { error_includes?: string };
    if (expectBlock.error_includes !== undefined) {
      const message = JSON.stringify(result.success ? {} : result.error.format());
      expect(message.toLowerCase()).toContain(expectBlock.error_includes.toLowerCase());
    }
  });

  it("rmssd-toy-intervals", () => {
    const manifest = loadManifest("rmssd-toy-intervals");
    const intervals = manifest.intervals_ms as number[];
    const expectBlock = manifest.expect as { rmssd_ms: number; n_beats: number };
    const computed = rmssdFromIntervals(intervals);
    expect(computed).not.toBeNull();
    expect(computed!.rmssd_ms).toBe(expectBlock.rmssd_ms);
    expect(computed!.n_beats).toBe(expectBlock.n_beats);
  });
});
