import { ZodError } from "zod";

function formatZodIssuesJsonString(text: string): string | undefined {
  const trimmed = text.trim();
  if (!trimmed.startsWith("[")) {
    return undefined;
  }
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (!Array.isArray(parsed) || parsed.length === 0) {
      return undefined;
    }
    const first = parsed[0] as { path?: unknown[]; message?: string };
    if (typeof first.message !== "string") {
      return undefined;
    }
    const path =
      Array.isArray(first.path) && first.path.length > 0
        ? first.path.map(String).join(".")
        : "record";
    return `${path}: ${first.message}`;
  } catch {
    return undefined;
  }
}

export function formatSchemaValidationError(error: unknown): string {
  if (error instanceof ZodError) {
    const first = error.issues[0];
    if (first === undefined) {
      return "record does not match its domain schema";
    }
    const path = first.path.length === 0 ? "record" : first.path.join(".");
    return `${path}: ${first.message}`;
  }
  if (error instanceof Error) {
    const fromJson = formatZodIssuesJsonString(error.message);
    return fromJson ?? error.message;
  }
  if (typeof error === "string") {
    const fromJson = formatZodIssuesJsonString(error);
    return fromJson ?? error;
  }
  return String(error);
}
