import { access, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  cleanupStaleExportStagingDirectories,
  exportStagingDirectoryName,
} from "../src/db/export-staging.js";

describe("export staging cleanup", () => {
  it("removes unmarked staging directories when the owning pid is dead", async () => {
    const parent = await mkdtemp(join(tmpdir(), "ath-export-staging-"));
    const deadPid = 99_999_999;
    const staleName = exportStagingDirectoryName(deadPid, Date.now(), "package");
    const stalePath = join(parent, staleName);
    await mkdir(stalePath, { recursive: true });
    await writeFile(join(stalePath, "athlete.ath.json"), "{}", { mode: 0o600 });

    await cleanupStaleExportStagingDirectories(parent);

    await expect(access(stalePath)).rejects.toMatchObject({ code: "ENOENT" });
    await rm(parent, { recursive: true, force: true });
  });
});
