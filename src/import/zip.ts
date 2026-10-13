/**
 * Reading entries out of a zip without inflating the whole archive.
 *
 * An Apple Health export.zip can be several gigabytes once expanded, and the file
 * we want is one entry inside it. `yauzl` gives a read stream per entry, so the
 * XML can be streamed straight into the parser.
 */
import { createWriteStream } from "node:fs";
import { Readable, Transform } from "node:stream";
import { pipeline } from "node:stream/promises";
import yauzl from "yauzl";

export const MAX_ZIP_ENTRIES = 100_000;
export const MAX_ZIP_ENTRY_BYTES = 16 * 1024 * 1024 * 1024;
export const MAX_ZIP_TOTAL_BYTES = 64 * 1024 * 1024 * 1024;
export const MAX_ZIP_MEMORY_ENTRY_BYTES = 64 * 1024 * 1024;
export const MAX_ZIP_COMPRESSION_RATIO = 1_000;

export interface ZipIndex {
  path: string;
  entries: string[];
}

function safeEntryName(name: string): void {
  const normalized = name.replaceAll("\\", "/");
  if (
    name.includes("\0") ||
    normalized.startsWith("/") ||
    /^[a-zA-Z]:/.test(normalized) ||
    normalized.split("/").some((part) => part === "..")
  ) {
    throw new Error(`archive entry has an unsafe path: ${name}`);
  }
}

function assertSafeEntry(entry: yauzl.Entry): void {
  safeEntryName(entry.fileName);
  const unixMode = (entry.externalFileAttributes >>> 16) & 0o170000;
  if (unixMode === 0o120000) {
    throw new Error(`archive entry is a symbolic link: ${entry.fileName}`);
  }
  if (entry.uncompressedSize > MAX_ZIP_ENTRY_BYTES) {
    throw new Error(
      `archive entry exceeds ${MAX_ZIP_ENTRY_BYTES} expanded bytes: ${entry.fileName}`,
    );
  }
  if (
    entry.uncompressedSize > 0 &&
    (entry.compressedSize === 0 ||
      entry.uncompressedSize / entry.compressedSize >
        MAX_ZIP_COMPRESSION_RATIO)
  ) {
    throw new Error(
      `archive entry exceeds the ${MAX_ZIP_COMPRESSION_RATIO}:1 expansion limit: ${entry.fileName}`,
    );
  }
}

/** Read the central directory only: entry names, no decompression. */
export function openZip(path: string): Promise<ZipIndex> {
  return new Promise((resolve, reject) => {
    yauzl.open(
      path,
      {
        lazyEntries: true,
        strictFileNames: true,
        validateEntrySizes: true,
      },
      (err, zip) => {
        if (err || !zip) {
          return reject(err ?? new Error(`could not open ${path} as a zip`));
        }
        const entries: string[] = [];
        let expandedBytes = 0;
        zip.on("entry", (entry) => {
          try {
            assertSafeEntry(entry);
            if (entries.length >= MAX_ZIP_ENTRIES) {
              throw new Error(`archive has more than ${MAX_ZIP_ENTRIES} entries`);
            }
            expandedBytes += entry.uncompressedSize;
            if (expandedBytes > MAX_ZIP_TOTAL_BYTES) {
              throw new Error(
                `archive exceeds ${MAX_ZIP_TOTAL_BYTES} total expanded bytes`,
              );
            }
            entries.push(entry.fileName);
            zip.readEntry();
          } catch (error) {
            zip.close();
            reject(error);
          }
        });
        zip.on("end", () => resolve({ path, entries }));
        zip.on("error", reject);
        zip.readEntry();
      },
    );
  });
}

export function zipEntryNames(zip: ZipIndex): string[] {
  return zip.entries;
}

/** A single entry as text. For entries small enough to hold in memory (CSVs). */
export function readZipEntry(zipPath: string, entryName: string): Promise<string> {
  return new Promise((resolve, reject) => {
    safeEntryName(entryName);
    yauzl.open(zipPath, {
      lazyEntries: true,
      strictFileNames: true,
      validateEntrySizes: true,
    }, (err, zip) => {
      if (err || !zip) return reject(err ?? new Error(`could not open ${zipPath} as a zip`));
      let found = false;
      zip.on("entry", (entry) => {
        if (entry.fileName !== entryName) return zip.readEntry();
        found = true;
        try {
          assertSafeEntry(entry);
          if (entry.uncompressedSize > MAX_ZIP_MEMORY_ENTRY_BYTES) {
            throw new Error(
              `archive text entry exceeds ${MAX_ZIP_MEMORY_ENTRY_BYTES} bytes: ${entryName}`,
            );
          }
        } catch (error) {
          zip.close();
          reject(error);
          return;
        }
        zip.openReadStream(entry, (streamErr, stream) => {
          if (streamErr || !stream) {
            return reject(streamErr ?? new Error(`could not read ${entryName}`));
          }
          const chunks: Buffer[] = [];
          let bytes = 0;
          stream.on("data", (c: Buffer) => {
            bytes += c.byteLength;
            if (bytes > MAX_ZIP_MEMORY_ENTRY_BYTES) {
              stream.destroy(
                new Error(
                  `archive text entry exceeds ${MAX_ZIP_MEMORY_ENTRY_BYTES} bytes: ${entryName}`,
                ),
              );
              return;
            }
            chunks.push(c);
          });
          stream.on("end", () => {
            zip.close();
            resolve(Buffer.concat(chunks).toString("utf8"));
          });
          stream.on("error", reject);
        });
      });
      zip.on("end", () => {
        if (!found) reject(new Error(`${entryName} not found in ${zipPath}`));
      });
      zip.on("error", reject);
      zip.readEntry();
    });
  });
}

export interface ZipEntryStream {
  stream: Readable;
  /** Bytes once inflated, from the central directory. What a progress bar counts against. */
  size: number;
}

/**
 * A read stream for one entry, so a large XML never lands in memory whole.
 */
export function openZipEntryStream(zipPath: string, entryName: string): Promise<ZipEntryStream> {
  return new Promise((resolve, reject) => {
    safeEntryName(entryName);
    yauzl.open(zipPath, {
      lazyEntries: true,
      strictFileNames: true,
      validateEntrySizes: true,
    }, (err, zip) => {
      if (err || !zip) return reject(err ?? new Error(`could not open ${zipPath} as a zip`));
      let found = false;
      zip.on("entry", (entry) => {
        if (entry.fileName !== entryName) return zip.readEntry();
        found = true;
        try {
          assertSafeEntry(entry);
        } catch (error) {
          zip.close();
          reject(error);
          return;
        }
        zip.openReadStream(entry, (streamErr, stream) => {
          if (streamErr || !stream) {
            return reject(streamErr ?? new Error(`could not read ${entryName}`));
          }
          let bytes = 0;
          const limiter = new Transform({
            transform(chunk: Buffer, _encoding, callback) {
              bytes += chunk.byteLength;
              if (bytes > MAX_ZIP_ENTRY_BYTES) {
                callback(
                  new Error(
                    `archive entry exceeds ${MAX_ZIP_ENTRY_BYTES} expanded bytes: ${entryName}`,
                  ),
                );
                return;
              }
              callback(null, chunk);
            },
          });
          resolve({
            stream: (stream as unknown as Readable).pipe(limiter),
            size: entry.uncompressedSize,
          });
        });
      });
      zip.on("end", () => {
        if (!found) reject(new Error(`${entryName} not found in ${zipPath}`));
      });
      zip.on("error", reject);
      zip.readEntry();
    });
  });
}

/** Spill an entry to disk, for cases that need a seekable file. */
export async function extractZipEntry(
  zipPath: string,
  entryName: string,
  destination: string,
): Promise<void> {
  const { stream } = await openZipEntryStream(zipPath, entryName);
  await pipeline(stream, createWriteStream(destination));
}
