import { open, rename, unlink } from "node:fs/promises";
import { dirname } from "node:path";

/** Writes bytes to a new path with fsync before the file is visible at the final name. */
export async function writeOwnerOnlyDurable(path: string, bytes: Uint8Array): Promise<void> {
  const directory = dirname(path);
  const temporary = `${path}.tmp-${process.pid}-${Date.now()}`;
  const file = await open(temporary, "wx", 0o600);
  try {
    await file.writeFile(bytes);
    await file.sync();
  } finally {
    await file.close();
  }
  await rename(temporary, path);
  await syncParentDirectory(directory);
}

/**
 * Directory fsync keeps renames durable on Unix. Node throws EINVAL on Windows,
 * where the file fsync above is enough for a single-file write.
 */
export async function syncParentDirectory(directory: string): Promise<void> {
  if (process.platform === "win32") {
    return;
  }
  const directoryHandle = await open(directory, "r");
  try {
    await directoryHandle.sync();
  } finally {
    await directoryHandle.close();
  }
}

export async function removeFileIfPresent(path: string): Promise<void> {
  try {
    await unlink(path);
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
      throw error;
    }
  }
}
