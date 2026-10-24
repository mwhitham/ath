/**
 * The import entry point: detect the export, parse it, fold it in.
 *
 * Each importer returns a payload rather than writing anything, so the merge rules
 * (source upsert, deduplication that always includes the device, series replacement)
 * live in one place and apply identically to every vendor.
 */
import {
  ATHLETIC_STANDARD_VERSION,
  type AthleticStandardFileT,
  type SourceT,
} from "../schema.js";
import { silentProgress, type Progress } from "../progress.js";
import { detectExport, readCsvBundle, type DetectedExport } from "./detect.js";
import { importAppleHealth } from "./apple.js";
import { importWhoop } from "./whoop.js";
import { importOura } from "./oura.js";
import {
  manualSourceFor,
  mergePayload,
  pooledSources,
  sourceFor,
  type MergeSummary,
  type SourceBook,
} from "./merge.js";

export { detectExport, UnknownExportError } from "./detect.js";

const VENDOR_LABELS: Record<DetectedExport["format"], string> = {
  apple: "Apple Health",
  whoop: "WHOOP",
  oura: "Oura",
};

export interface ImportResult {
  summary: MergeSummary;
  label: string;
}

export interface ParsedProviderExport {
  format: DetectedExport["format"];
  route: "apple_health" | "whoop_csv" | "oura_csv";
  label: string;
  sources: SourceT[];
  payload: Awaited<ReturnType<typeof importAppleHealth>>;
}

/** Thrown when the file was written before readings were labelled by writer (D45). */
export class PooledFileError extends Error {
  constructor(sourceIds: string[]) {
    super(
      `this file labels every reading in an export with one source (${sourceIds.join(", ")}), ` +
        `so a watch and a scale share a name. Newer imports keep each device apart, and ` +
        `nothing in this file says which reading came from which device — only the original ` +
        `export does. Run \`ath init\` in a new folder and import your exports there.`,
    );
    this.name = "PooledFileError";
  }
}

async function parseProviderExportInto(
  file: AthleticStandardFileT,
  exportPath: string,
  progress: Progress,
): Promise<ParsedProviderExport> {
  const detected = await detectExport(exportPath);
  const label = `${VENDOR_LABELS[detected.format]} export`;
  const route =
    detected.format === "apple"
      ? "apple_health"
      : detected.format === "whoop"
        ? "whoop_csv"
        : "oura_csv";
  const container =
    detected.container === "zip"
      ? "zip export"
      : detected.container === "directory"
        ? "export folder"
        : detected.format === "apple"
          ? "export.xml"
          : "CSV export";

  const vendorLabel = VENDOR_LABELS[detected.format];
  const sources: SourceBook = {
    forWriter: (spec) => {
      const who = `${spec.writer}${spec.sensor ? ` ${spec.sensor}` : ""}`;
      const detail =
        spec.writer === vendorLabel
          ? `${vendorLabel} ${container}`
          : `${who} via ${vendorLabel} ${container}`;
      return sourceFor(file, { ...spec, via: route, detail });
    },
    manual: () => manualSourceFor(file),
  };

  const payload =
    detected.format === "apple"
      ? await importAppleHealth(detected, sources, progress)
      : detected.format === "whoop"
        ? importWhoop(await readCsvBundle(detected), sources)
        : importOura(await readCsvBundle(detected), sources);

  return {
    format: detected.format,
    route,
    label,
    sources: file.sources,
    payload,
  };
}

/**
 * Parse a provider export without applying the old JSON merge rules.
 *
 * The provider parsers remain shared while v1 maps their output into stable SQLite
 * records, revisions, and import keys.
 */
export async function parseProviderExport(
  exportPath: string,
  progress: Progress = silentProgress,
): Promise<ParsedProviderExport> {
  const file: AthleticStandardFileT = {
    athleticstandard_version: ATHLETIC_STANDARD_VERSION,
    athlete: {},
    sources: [],
    hard_signals: [],
    soft_signals: [],
    benchmarks: [],
    predictions: [],
  };
  return parseProviderExportInto(file, exportPath, progress);
}

