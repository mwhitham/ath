import { ZodError } from "zod";

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
    return error.message;
  }
  return String(error);
}
