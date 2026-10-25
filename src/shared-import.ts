import { lstatSync, readdirSync, readFileSync } from "node:fs";
import { basename } from "node:path";
import { openZip, zipEntryNames } from "./import/zip.js";

export type SharedImport = "garmin" | "strava";

export async function detectSharedImport(path: string): Promise<SharedImport | null> {
  let stat;
  try {
    stat = lstatSync(path);
  } catch {
    return null;
  }
  if (stat.isSymbolicLink()) return null;
  if (stat.isDirectory()) {
    return detectNames(readdirSync(path));
  }
  if (!stat.isFile()) return null;
  const lower = basename(path).toLowerCase();
  if (lower.endsWith(".gpx") || lower.endsWith(".fit")) return "garmin";
  if (lower.endsWith(".json")) {
    const head = readFileSync(path, "utf8").slice(0, 8000);
    if (
      head.includes("activityName") ||
      head.includes("summarizedActivities") ||
      head.includes("startTimeGMT")
    ) {
      return "garmin";
    }
  }
  if (lower.endsWith(".zip")) {
    const zip = await openZip(path);
    return detectNames(zipEntryNames(zip));
  }
  return null;
}

function detectNames(names: string[]): SharedImport | null {
  const lower = names.map((name) => name.toLowerCase());
  if (lower.some((name) => name.endsWith("activities.csv"))) return "strava";
  if (
    lower.some(
      (name) =>
        name.endsWith(".gpx") ||
        name.endsWith(".fit") ||
        name.endsWith("summarizedactivities.json"),
    )
  ) {
    return "garmin";
  }
  return null;
}

export function renderSharedImport(kind: SharedImport, preview: unknown): string {
  const body = preview as {
    understood?: string[];
    preserved?: string[];
    left_out?: string[];
    record_ids?: string[];
  };
  const lines = [
    `${kind} import`,
    `understood: ${(body.understood ?? []).join(", ") || "none"}`,
    `kept under original names: ${(body.preserved ?? []).join(", ") || "none"}`,
  ];
  for (const item of body.left_out ?? []) lines.push(`left out: ${item}`);
  if ((body.record_ids ?? []).length > 0) {
    lines.push(`records: ${body.record_ids?.join(", ")}`);
  }
  return lines.join("\n");
}
