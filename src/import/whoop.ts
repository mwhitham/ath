/**
 * WHOOP's standard dashboard export: four flat CSVs.
 *
 * The reader is ath-core. This file only asks for a source id and puts the
 * signals back into the maps the rest of the import uses.
 */
import { runAthCore } from "../ath-core.js";
import type { HardSignalT, SoftSignalT } from "../schema.js";
import { emptyPayload, type ImportPayload, type SourceBook } from "./merge.js";

type SignalReceipt = ImportPayload["inputReceipts"] extends Map<unknown, infer Value>
  ? Value
  : never;

export function importWhoop(files: Map<string, string>, sources: SourceBook): ImportPayload {
  const sourceId = sources.forWriter({ writer: "WHOOP", vendor: "whoop" });
  const raw = runAthCore(
    ["import-whoop"],
    process.env,
    JSON.stringify({ files: Object.fromEntries(files), source_id: sourceId }),
  ) as {
    vendor: string;
    detail: string;
    hard: Array<{ signal: HardSignalT; receipt: SignalReceipt }>;
    soft: Array<{ signal: SoftSignalT; receipt: SignalReceipt }>;
    skipped: Record<string, number>;
    skip_examples: Record<string, string>;
  };
  const payload = emptyPayload(raw.vendor, raw.detail);
  for (const item of raw.hard) {
    payload.inputReceipts.set(item.signal, item.receipt);
    payload.hardSignals.push(item.signal);
  }
  for (const item of raw.soft) {
    payload.inputReceipts.set(item.signal, item.receipt);
    payload.softSignals.push(item.signal);
  }
  for (const [reason, count] of Object.entries(raw.skipped)) {
    payload.skipped.set(reason, count);
  }
  for (const [reason, example] of Object.entries(raw.skip_examples)) {
    payload.skipExamples.set(reason, example);
  }
  return payload;
}
