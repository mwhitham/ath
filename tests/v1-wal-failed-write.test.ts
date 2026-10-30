import { spawn } from "node:child_process";
import { cp, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { ath } from "./helpers/cli-exec.js";

const DEMO_FIXTURE = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../examples/demo-athlete/athlete.ath.sqlite",
);

const directories: string[] = [];
const children: ReturnType<typeof spawn>[] = [];

afterEach(async () => {
  for (const child of children.splice(0)) {
    child.kill("SIGTERM");
  }
  for (const directory of directories.splice(0)) {
    await rm(directory, { recursive: true, force: true });
  }
});

describe.runIf(process.platform !== "win32")("WAL retention after failed writes", () => {
  it("keeps committed WAL data when a later write fails under a read snapshot", async () => {
    const directory = await mkdtemp(join(tmpdir(), "ath-wal-failed-write-"));
    directories.push(directory);
    await cp(dirname(DEMO_FIXTURE), directory, { recursive: true });
    const databasePath = join(directory, "athlete.ath.sqlite");

    const reader = spawn("sqlite3", [databasePath], {
      cwd: directory,
      stdio: ["pipe", "ignore", "pipe"],
    });
    children.push(reader);
    reader.stdin?.write("BEGIN DEFERRED;\nSELECT count(*) FROM records;\n");
    await new Promise<void>((resolveReady, reject) => {
      const timeout = setTimeout(() => reject(new Error("reader did not start")), 10_000);
      reader.stderr?.on("data", () => {
        clearTimeout(timeout);
        resolveReady();
      });
      reader.on("error", reject);
      setTimeout(() => {
        clearTimeout(timeout);
        resolveReady();
      }, 200);
    });

    const log = ath(["log", "note two", "-y"], directory);
    expect(log.code, log.stdout).toBe(0);

    const listed = JSON.parse(ath(["records", "list", "--json"], directory).stdout) as {
      records: Array<{ record_id: string; record_type: string; current_revision: number }>;
    };
    const report = listed.records
      .filter((record) => record.record_type === "athlete_report")
      .at(-1);
    expect(report).toBeDefined();
    const staleTarget = listed.records.find((record) => record.current_revision > 1);
    expect(staleTarget).toBeDefined();

    const staleDelete = ath(
      ["delete", staleTarget!.record_id, "--revision", "1", "-y"],
      directory,
    );
    expect(staleDelete.code).not.toBe(0);

    const showBeforeReaderEnds = ath(
      ["records", "show", report!.record_id, "--json"],
      directory,
    );
    expect(showBeforeReaderEnds.code, showBeforeReaderEnds.stdout).toBe(0);
    const shownBeforeReaderEnds = JSON.parse(showBeforeReaderEnds.stdout) as {
      record: { data: { note?: string } };
    };
    expect(shownBeforeReaderEnds.record.data.note).toBe("note two");

    reader.kill("SIGKILL");
    children.pop();
    await new Promise<void>((resolveExit) => {
      reader.on("exit", () => resolveExit());
      setTimeout(resolveExit, 2_000);
    });

    const showAfterReaderEnds = ath(["records", "show", report!.record_id, "--json"], directory);
    expect(showAfterReaderEnds.code, showAfterReaderEnds.stdout).toBe(0);
    const shown = JSON.parse(showAfterReaderEnds.stdout) as {
      record: { data: { note?: string } };
    };
    expect(shown.record.data.note).toBe("note two");

    const check = ath(["check"], directory);
    expect(check.code, check.stdout).toBe(0);
    expect(check.stdout).toContain("valid Athletic Standard 2.0.0 database");
  }, 60_000);
});
