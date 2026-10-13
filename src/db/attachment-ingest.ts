import { createHash } from "node:crypto";
import { lstat, mkdir, open, readFile, rename, rm } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import type { RecordActorT } from "../v1/schema.js";
import {
  refuseSymbolicLinkComponents,
  type AthleticDatabase,
} from "./client.js";
import { storedAttachmentName } from "./attachments.js";
import type { LogPlan } from "./log-entry.js";
import type { WriteRecordInput } from "./records.js";
import { createUuidV7 } from "./uuid.js";

export const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024;

export interface PreparedAttachment {
  input: WriteRecordInput;
  bytes: Buffer;
  final_path: string;
}

function mediaType(bytes: Uint8Array): string | null {
  if (
    bytes.length >= 8 &&
    bytes[0] === 0x89 &&
    bytes[1] === 0x50 &&
    bytes[2] === 0x4e &&
    bytes[3] === 0x47 &&
    bytes[4] === 0x0d &&
    bytes[5] === 0x0a &&
    bytes[6] === 0x1a &&
    bytes[7] === 0x0a
  ) {
    return "image/png";
  }
  if (
    bytes.length >= 3 &&
    bytes[0] === 0xff &&
    bytes[1] === 0xd8 &&
    bytes[2] === 0xff
  ) {
    return "image/jpeg";
  }
  if (
    Buffer.from(bytes.subarray(0, 4)).toString("ascii") === "RIFF" &&
    Buffer.from(bytes.subarray(8, 12)).toString("ascii") === "WEBP"
  ) {
    return "image/webp";
  }
  if (Buffer.from(bytes.subarray(0, 5)).toString("ascii") === "%PDF-") {
    return "application/pdf";
  }
  return null;
}

export async function prepareAttachments(
  database: AthleticDatabase,
  paths: readonly string[],
  actor: RecordActorT,
  createdAt: string,
): Promise<PreparedAttachment[]> {
  if (paths.length === 0) {
    return [];
  }
  if (database.localPath === undefined) {
    throw new Error("attachment ingestion requires a local SQLite database");
  }
  const attachmentDirectory = join(dirname(database.localPath), "attachments");
  await refuseSymbolicLinkComponents(attachmentDirectory, {
    allowMissingTail: true,
  });
  const prepared: PreparedAttachment[] = [];
  for (const suppliedPath of paths) {
    const path = resolve(suppliedPath);
    await refuseSymbolicLinkComponents(path);
    const stat = await lstat(path);
    if (!stat.isFile()) {
      throw new Error(`attachment is not a regular file: ${path}`);
    }
    if (stat.size > MAX_ATTACHMENT_BYTES) {
      throw new Error(
        `attachment exceeds ${MAX_ATTACHMENT_BYTES} bytes: ${path}`,
      );
    }
    const bytes = await readFile(path);
    const detectedMediaType = mediaType(bytes);
    if (detectedMediaType === null) {
      throw new Error(
        `unsupported attachment type: ${path}; use PNG, JPEG, WebP, or PDF`,
      );
    }
    const recordId = createUuidV7();
    const data = {
      file_name: basename(path),
      media_type: detectedMediaType,
      byte_count: bytes.byteLength,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    };
    const record = {
      record_id: recordId,
      record_type: "attachment" as const,
      created_at: createdAt,
      current_revision: 1,
      is_deleted: false,
      data,
    };
    prepared.push({
      input: {
        record_id: recordId,
        record_type: "attachment",
        created_at: createdAt,
        changed_by: actor,
        data,
      },
      bytes,
      final_path: join(attachmentDirectory, storedAttachmentName(record)),
    });
  }
  return prepared;
}

export function addAttachmentsToLogPlan(
  plan: LogPlan,
  attachments: readonly PreparedAttachment[],
): LogPlan {
  if (attachments.length === 0) {
    return plan;
  }
  const linkable = plan.inputs
    .map((input, index) => ({ input, index }))
    .filter(
      ({ input }) =>
        input.record_type === "athlete_report" ||
        input.record_type === "nutrition_entry",
    );
  if (linkable.length !== 1) {
    throw new Error(
      "attachments need exactly one athlete_report or nutrition_entry in the log batch",
    );
  }
  const linked = linkable[0]!;
  const data = linked.input.data as { attachment_ids?: string[] };
  const inputs = [...plan.inputs];
  inputs[linked.index] = {
    ...linked.input,
    data: {
      ...(linked.input.data as Record<string, unknown>),
      attachment_ids: [
        ...(data.attachment_ids ?? []),
        ...attachments.map((attachment) => attachment.input.record_id!),
      ],
    },
  };
  return {
    ...plan,
    interpretation:
      `${plan.interpretation} with ${attachments.length} attachment` +
      `${attachments.length === 1 ? "" : "s"}`,
    inputs: [
      ...attachments.map((attachment) => attachment.input),
      ...inputs,
    ],
  };
}

export async function withPreparedAttachmentFiles<T>(
  database: AthleticDatabase,
  attachments: readonly PreparedAttachment[],
  write: () => Promise<T>,
): Promise<T> {
  if (attachments.length === 0) {
    return write();
  }
  if (database.localPath === undefined) {
    throw new Error("attachment ingestion requires a local SQLite database");
  }
  const directory = join(dirname(database.localPath), "attachments");
  await mkdir(directory, { recursive: true, mode: 0o700 });
  await refuseSymbolicLinkComponents(directory);
  const temporaryFiles: string[] = [];
  const finalFiles: string[] = [];
  try {
    for (const attachment of attachments) {
      const temporary = `${attachment.final_path}.tmp-${process.pid}`;
      const file = await open(temporary, "wx", 0o600);
      try {
        await file.writeFile(attachment.bytes);
      } finally {
        await file.close();
      }
      temporaryFiles.push(temporary);
      await rename(temporary, attachment.final_path);
      temporaryFiles.pop();
      finalFiles.push(attachment.final_path);
    }
    return await write();
  } catch (error) {
    await Promise.all([
      ...temporaryFiles.map((path) => rm(path, { force: true })),
      ...finalFiles.map((path) => rm(path, { force: true })),
    ]);
    throw error;
  }
}
