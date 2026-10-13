import { extname } from "node:path";
import type { CurrentRecordT } from "../v1/schema.js";

export function storedAttachmentName(
  record: Extract<CurrentRecordT, { record_type: "attachment" }>,
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
