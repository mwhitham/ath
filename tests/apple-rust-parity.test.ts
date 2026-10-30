import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { parseProviderExport } from "../src/import/index.js";
import type { ImportPayload } from "../src/import/merge.js";
import type { SourceT } from "../src/schema.js";

/**
 * Captured from the TypeScript Apple importer before that reader moved into
 * ath-core. Source ids are names, not generated ids. The Rust reader has to
 * match this output.
 */
const expected = JSON.parse(
  readFileSync(resolve(import.meta.dirname, "fixtures/apple-rust-parity.json"), "utf8"),
);

function label(sources: SourceT[], id: string): string {
  const source = sources.find((item) => item.id === id);
  if (!source) return `missing:${id}`;
  if (source.kind === "manual") return "manual";
  return [source.kind, source.writer, source.vendor ?? "", source.sensor ?? "", source.via ?? ""].join("|");
}

function signature(sources: SourceT[], payload: ImportPayload) {
  const hard = payload.hardSignals.map((signal) => {
    const { source, ...rest } = signal;
    const key = payload.deviceOf.get(signal);
    const seen = key ? payload.deviceIndex.get(key) : undefined;
    return {
      source: label(sources, source),
      signal: rest,
      receipt: payload.inputReceipts.get(signal) ?? null,
      device: seen ? { source: label(sources, seen.source), device: seen.device } : null,
    };
  });
  return {
    vendor: payload.vendor,
    hard,
    summaries: payload.providerSummaries.map((summary) => {
      const { source, ...rest } = summary;
      return { source: label(sources, source), summary: rest };
    }),
    series: payload.series.map((item) => ({
      quantity: item.quantity,
      day: item.day,
      source: label(sources, item.source),
      n: item.n,
      samples: item.samples,
    })),
    skipped: [...payload.skipped.entries()],
    skip_examples: [...payload.skipExamples.entries()],
    devices: [...payload.deviceIndex.values()].map((seen) => ({
      source: label(sources, seen.source),
      device: seen.device,
      from: seen.from,
      to: seen.to,
    })),
    sources: sources.map((source) => ({
      label: label(sources, source.id),
      kind: source.kind,
      writer: source.writer ?? null,
      vendor: source.vendor ?? null,
      sensor: source.sensor ?? null,
      via: source.via ?? null,
    })),
  };
}

describe("Apple Health in ath-core matches the TypeScript importer", () => {
  it("returns the same records, sleep sessions, timestamps, and offsets", async () => {
    const parsed = await parseProviderExport(resolve(import.meta.dirname, "fixtures/exports/apple"));
    expect(signature(parsed.sources, parsed.payload)).toEqual(expected);
    const rmssd = parsed.payload.hardSignals.filter(
      (signal) => "type" in signal && signal.type === "hrv_rmssd",
    );
    const sdnn = parsed.payload.series.find((item) => item.quantity === "hrv_sdnn");
    expect(rmssd.length).toBeGreaterThan(0);
    expect(sdnn?.samples.some((sample) => sample.value === 52.3)).toBe(true);
    expect(rmssd.some((signal) => signal.type === "hrv_rmssd" && signal.value === 52.3)).toBe(false);
    expect(parsed.payload.hardSignals.some((signal) => signal.source === parsed.sources.find((source) => source.kind === "manual")?.id && "type" in signal && signal.type === "body_temperature")).toBe(true);
  });
});
