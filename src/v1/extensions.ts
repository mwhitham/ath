/**
 * Optional namespaced extension envelope on v1 records (D118).
 *
 * Extension values inherit the parent record class. They cannot claim a source on
 * athlete-reported records or remove source requirements from measured data.
 */
import { z } from "zod";

/** Maximum UTF-8 bytes for the whole extensions object when JSON-encoded. */
export const EXTENSIONS_MAX_ENCODED_BYTES = 65_536;

/** Maximum UTF-8 bytes for one extension value when JSON-encoded. */
export const EXTENSION_VALUE_MAX_ENCODED_BYTES = 16_384;

/** Maximum extension keys on one record. */
export const EXTENSIONS_MAX_KEYS = 32;

/** Maximum nesting depth for extension JSON values (objects and arrays). */
export const EXTENSION_VALUE_MAX_DEPTH = 8;

/** Maximum length of one extension string value. */
export const EXTENSION_STRING_MAX_LENGTH = 4_096;

/** Maximum array length at one extension array node. */
export const EXTENSION_ARRAY_MAX_LENGTH = 256;

/** Maximum object keys at one extension object node. */
export const EXTENSION_OBJECT_MAX_KEYS = 64;

/**
 * Namespaced extension ID: reverse-DNS owner segment, slash, extension name.
 * Example: `ath.fit/demo/widget`.
 */
export const ExtensionId = z
  .string()
  .regex(
    /^[a-z][a-z0-9]*(?:\.[a-z][a-z0-9]*)+\/[a-z][a-z0-9][a-z0-9._-]*$/,
    "namespaced extension id",
  )
  .max(200);

export type ExtensionIdT = z.infer<typeof ExtensionId>;

const ATHLETE_REPORTED_RECORD_TYPES = new Set([
  "athlete_report",
  "nutrition_entry",
  "attachment",
  "athlete_profile",
]);

const FORBIDDEN_EXTENSION_KEYS = new Set(["source_id"]);

function encodedUtf8Length(value: string): number {
  return new TextEncoder().encode(value).length;
}

function extensionValueDepth(value: unknown, depth = 1): number {
  if (value === null || typeof value !== "object") {
    return depth;
  }
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return depth;
    }
    return Math.max(...value.map((child) => extensionValueDepth(child, depth + 1)));
  }
  const entries = Object.entries(value as Record<string, unknown>);
  if (entries.length === 0) {
    return depth;
  }
  return Math.max(...entries.map(([, child]) => extensionValueDepth(child, depth + 1)));
}

function collectForbiddenExtensionKeys(
  value: unknown,
  path: string,
  issues: Array<{ path: string; message: string }>,
): void {
  if (value === null || typeof value !== "object") {
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((child, index) => {
      collectForbiddenExtensionKeys(child, `${path}[${index}]`, issues);
    });
    return;
  }
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    if (FORBIDDEN_EXTENSION_KEYS.has(key)) {
      issues.push({
        path: path === "" ? key : `${path}.${key}`,
        message: "extensions cannot contain source_id",
      });
    }
    collectForbiddenExtensionKeys(child, path === "" ? key : `${path}.${key}`, issues);
  }
}

function validateExtensionValueShape(
  value: unknown,
  path: string,
  depth: number,
  issues: Array<{ path: string; message: string }>,
): void {
  if (depth > EXTENSION_VALUE_MAX_DEPTH) {
    issues.push({ path, message: `extension value exceeds depth ${EXTENSION_VALUE_MAX_DEPTH}` });
    return;
  }
  if (value === null) {
    return;
  }
  if (typeof value === "string") {
    if (value.length > EXTENSION_STRING_MAX_LENGTH) {
      issues.push({
        path,
        message: `extension string exceeds ${EXTENSION_STRING_MAX_LENGTH} characters`,
      });
    }
    return;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return;
  }
  if (Array.isArray(value)) {
    if (value.length > EXTENSION_ARRAY_MAX_LENGTH) {
      issues.push({
        path,
        message: `extension array exceeds ${EXTENSION_ARRAY_MAX_LENGTH} elements`,
      });
    }
    value.forEach((child, index) => {
      validateExtensionValueShape(child, `${path}[${index}]`, depth + 1, issues);
    });
    return;
  }
  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>);
    if (entries.length > EXTENSION_OBJECT_MAX_KEYS) {
      issues.push({
        path,
        message: `extension object exceeds ${EXTENSION_OBJECT_MAX_KEYS} keys`,
      });
    }
    for (const [key, child] of entries) {
      validateExtensionValueShape(child, path === "" ? key : `${path}.${key}`, depth + 1, issues);
    }
  }
}

const ExtensionJsonValue: z.ZodType<unknown> = z.lazy(() =>
  z.union([
    z.string().max(EXTENSION_STRING_MAX_LENGTH),
    z.number().finite(),
    z.boolean(),
    z.null(),
    z.array(ExtensionJsonValue).max(EXTENSION_ARRAY_MAX_LENGTH),
    z.record(z.string(), ExtensionJsonValue),
  ]),
);

export const RecordExtensions = z
  .record(z.string(), ExtensionJsonValue)
  .superRefine((extensions, context) => {
    const keys = Object.keys(extensions);
    for (const key of keys) {
      const parsedId = ExtensionId.safeParse(key);
      if (!parsedId.success) {
        context.addIssue({
          code: "custom",
          path: [key],
          message: "extension key must be a namespaced extension id",
        });
      }
    }
    if (keys.length > EXTENSIONS_MAX_KEYS) {
      context.addIssue({
        code: "custom",
        message: `extensions cannot contain more than ${EXTENSIONS_MAX_KEYS} keys`,
      });
      return;
    }
    const encoded = JSON.stringify(extensions);
    if (encodedUtf8Length(encoded) > EXTENSIONS_MAX_ENCODED_BYTES) {
      context.addIssue({
        code: "custom",
        message: `extensions exceed ${EXTENSIONS_MAX_ENCODED_BYTES} encoded bytes`,
      });
    }
    for (const [extensionId, value] of Object.entries(extensions)) {
      const valueEncoded = JSON.stringify(value);
      if (encodedUtf8Length(valueEncoded) > EXTENSION_VALUE_MAX_ENCODED_BYTES) {
        context.addIssue({
          code: "custom",
          path: [extensionId],
          message: `extension value exceeds ${EXTENSION_VALUE_MAX_ENCODED_BYTES} encoded bytes`,
        });
      }
      const depthIssues: Array<{ path: string; message: string }> = [];
      validateExtensionValueShape(value, extensionId, 1, depthIssues);
      for (const issue of depthIssues) {
        context.addIssue({
          code: "custom",
          path: issue.path.split(/\.|\[|\]/).filter(Boolean),
          message: issue.message,
        });
      }
    }
  });

export type RecordExtensionsT = z.infer<typeof RecordExtensions>;

export function validateExtensionsForRecordType(
  recordType: string,
  extensions: RecordExtensionsT | undefined,
): void {
  if (extensions === undefined) {
    return;
  }
  if (!ATHLETE_REPORTED_RECORD_TYPES.has(recordType)) {
    return;
  }
  const issues: Array<{ path: string; message: string }> = [];
  collectForbiddenExtensionKeys(extensions, "", issues);
  if (issues.length > 0) {
    throw new Error(issues[0]?.message ?? "invalid extensions for athlete-reported record");
  }
}
