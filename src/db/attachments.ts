import { extname } from "node:path";
import type { CurrentRecordT } from "../v1/schema.js";

type AttachmentRecord = Extract<CurrentRecordT, { record_type: "attachment" }>;

function startsWith(bytes: Uint8Array, prefix: readonly number[]): boolean {
  return prefix.every((value, index) => bytes[index] === value);
}

export function assertAttachmentFileType(
  record: AttachmentRecord,
  bytes: Uint8Array,
): void {
  const mediaType = record.data.media_type.toLowerCase();
  const matches =
    mediaType === "image/png"
      ? startsWith(bytes, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
      : mediaType === "image/jpeg"
        ? startsWith(bytes, [0xff, 0xd8, 0xff])
        : mediaType === "image/webp"
          ? (
              Buffer.from(bytes.subarray(0, 4)).toString("ascii") === "RIFF" &&
              Buffer.from(bytes.subarray(8, 12)).toString("ascii") === "WEBP"
            )
          : mediaType === "application/pdf"
            ? Buffer.from(bytes.subarray(0, 5)).toString("ascii") === "%PDF-"
            : !mediaType.startsWith("image/");
  if (!matches) {
    throw new Error(
      `attachment bytes do not match supported media type ${record.data.media_type}: ${record.record_id}`,
    );
  }
}

export function storedAttachmentName(
  record: AttachmentRecord,
): string {
  const knownExtension: Readonly<Record<string, string>> = {
    "image/jpeg": ".jpg",
    "image/png": ".png",
    "image/webp": ".webp",
    "application/pdf": ".pdf",
  };
  const inputExtension = extname(record.data.file_name);
  const extension =
    knownExtension[record.data.media_type] ??
    (/^\.[a-zA-Z0-9]{1,10}$/.test(inputExtension)
      ? inputExtension.toLowerCase()
      : "");
  return `${record.record_id}${extension}`;
}
