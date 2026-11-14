/**
 * Error classes callers catch, and the mapping from worker errors to them.
 *
 * The classes keep their 2.1.0 constructors, names, messages, and fields.
 * Validation failures become genuine ZodError objects, as the 2.1.0 schema
 * parses threw them.
 */
import { ZodError } from "zod";
import type { MigrationPreview } from "../types.js";

export class MigrationRequiredError extends Error {
  readonly preview: MigrationPreview;

  constructor(preview: MigrationPreview) {
    super(
      `Database SQL schema ${preview.source_sql_schema_version} must be upgraded to ${preview.target_sql_schema_version} before other commands can run. Run ath migrate.`,
    );
    this.name = "MigrationRequiredError";
    this.preview = preview;
  }
}

/** The most entries a PackageRepresentationError keeps in each list. */
const MAX_REPRESENTATION_ISSUES = 1_000;

export class PackageRepresentationError extends Error {
  readonly source_database_id: string | undefined;
  readonly source_database_revision: number | undefined;
  readonly unrepresentable_records: string[];
  readonly unknown_fields: string[];

  constructor(options: {
    source_database_id?: string;
    source_database_revision?: number;
    unrepresentable_records?: string[];
    unknown_fields?: string[];
  }) {
    const detail = options.unrepresentable_records?.[0] ?? options.unknown_fields?.[0];
    super(
      "package has unknown fields or records that v1 cannot represent" +
        (detail === undefined ? "" : `: ${detail}`),
    );
    this.name = "PackageRepresentationError";
    this.source_database_id = options.source_database_id;
    this.source_database_revision = options.source_database_revision;
    this.unrepresentable_records =
      options.unrepresentable_records?.slice(0, MAX_REPRESENTATION_ISSUES) ?? [];
    this.unknown_fields = options.unknown_fields?.slice(0, MAX_REPRESENTATION_ISSUES) ?? [];
  }
}

/** The error object a worker sends. */
export interface WorkerError {
  code: string;
  message?: string;
  issues?: Array<{ path: Array<string | number>; code: string; message: string }>;
  preview?: MigrationPreview;
  source_database_id?: string;
  source_database_revision?: number;
  unrepresentable_records?: string[];
  unknown_fields?: string[];
}

/** The JavaScript error for a worker error. */
export function errorFromWorker(error: WorkerError): Error {
  switch (error.code) {
    case "invalid":
      if (error.issues !== undefined && error.issues.length > 0) {
        return new ZodError(error.issues as ConstructorParameters<typeof ZodError>[0]);
      }
      return new Error(error.message ?? "invalid input");
    case "migration_required":
      if (error.preview !== undefined) {
        return new MigrationRequiredError(error.preview);
      }
      return new Error(error.message ?? "migration required");
    case "package_representation":
      return new PackageRepresentationError({
        ...(error.source_database_id === undefined ? {} : { source_database_id: error.source_database_id }),
        ...(error.source_database_revision === undefined
          ? {}
          : { source_database_revision: error.source_database_revision }),
        ...(error.unrepresentable_records === undefined
          ? {}
          : { unrepresentable_records: error.unrepresentable_records }),
        ...(error.unknown_fields === undefined ? {} : { unknown_fields: error.unknown_fields }),
      });
    default: {
      const result = new Error(error.message ?? "the native operation failed");
      Object.defineProperty(result, "code", { value: `ATH_${error.code.toUpperCase()}`, enumerable: false });
      return result;
    }
  }
}
