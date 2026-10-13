import {
  createWriteStream,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { once } from "node:events";
import { describe, expect, it } from "vitest";
import { ZipFile } from "yazl";
import { assertAttachmentFileType } from "../src/db/attachments.js";
import { detectExport, readCsvBundle } from "../src/import/detect.js";
import { openZip } from "../src/import/zip.js";
import type { CurrentRecordT } from "../src/v1/schema.js";

async function writeZip(
  path: string,
  entries: Array<{ name: string; bytes: Buffer; mode?: number }>,
): Promise<void> {
  const zip = new ZipFile();
  for (const entry of entries) {
    zip.addBuffer(entry.bytes, entry.name, {
      ...(entry.mode === undefined ? {} : { mode: entry.mode }),
    });
  }
  const output = createWriteStream(path, { mode: 0o600 });
  zip.outputStream.pipe(output);
  zip.end();
  await once(output, "close");
}

function replaceEvery(
  input: Buffer,
  before: string,
  after: string,
): Buffer {
  expect(Buffer.byteLength(before)).toBe(Buffer.byteLength(after));
  const output = Buffer.from(input);
  let offset = 0;
  while ((offset = output.indexOf(before, offset, "utf8")) !== -1) {
    output.write(after, offset, "utf8");
    offset += Buffer.byteLength(after);
  }
  return output;
}

describe("untrusted provider and attachment input", () => {
  it("rejects archive traversal before reading an entry", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-zip-safety-"));
    const safe = join(directory, "safe.zip");
    const unsafe = join(directory, "unsafe.zip");
    await writeZip(safe, [
      {
        name: "aa/evil.csv",
        bytes: Buffer.from("day,readiness_score\n2026-01-01,90\n"),
      },
    ]);
    writeFileSync(
      unsafe,
      replaceEvery(readFileSync(safe), "aa/evil.csv", "../evil.csv"),
      { mode: 0o600 },
    );
    await expect(openZip(unsafe)).rejects.toThrow(/invalid relative path|unsafe path/i);
  });

  it("rejects archive links and extreme expansion ratios", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-zip-safety-"));
    const linked = join(directory, "linked.zip");
    await writeZip(linked, [
      {
        name: "export.xml",
        bytes: Buffer.from("target"),
        mode: 0o120777,
      },
    ]);
    await expect(openZip(linked)).rejects.toThrow("symbolic link");

    const expanded = join(directory, "expanded.zip");
    await writeZip(expanded, [
      {
        name: "export.xml",
        bytes: Buffer.alloc(2 * 1024 * 1024),
      },
    ]);
    await expect(openZip(expanded)).rejects.toThrow("expansion limit");
  });

  it("rejects symbolic links inside provider directories", async () => {
    const directory = mkdtempSync(join(tmpdir(), "ath-provider-safety-"));
    const outside = join(directory, "outside.csv");
    writeFileSync(
      outside,
      "Cycle start time,Cycle end time,Cycle timezone\n" +
        "2026-01-01 00:00:00,2026-01-01 08:00:00,UTC+00:00\n",
    );
    const provider = join(directory, "provider");
    mkdirSync(provider);
    symlinkSync(outside, join(provider, "physiological_cycles.csv"));
    const detected = await detectExport(provider);
    await expect(readCsvBundle(detected)).rejects.toThrow("symbolic link");
  });

  it("rejects image bytes that do not match the declared type", () => {
    const record = {
      record_id: "01890f3e-e7d3-7cc0-88c8-000000000001",
      record_type: "attachment",
      created_at: "2026-10-13T12:00:00.000Z",
      current_revision: 1,
      is_deleted: false,
      data: {
        file_name: "photo.png",
        media_type: "image/png",
        byte_count: 4,
        sha256: "0".repeat(64),
      },
    } satisfies CurrentRecordT;
    expect(() =>
      assertAttachmentFileType(record, Buffer.from("nope")),
    ).toThrow("do not match supported media type image/png");
  });
});
