/**
 * The client protocol: version refusal before any database opens, one
 * worker per call, streamed results, cancellation during a write, and a
 * worker that exits without answering.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, renameSync, rmSync } from "node:fs";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { athlete, nativeExecutable, noteData, removeFolder, temporaryFolder } from "./support.js";
import * as database from "../src/database.js";
import { callAsync, callSync } from "../src/client/worker.js";
import { PROTOCOL_VERSION, WORKER_ARGUMENT, OPERATIONS } from "../src/generated/protocol.js";

function runWorker(request: unknown): { status: number | null; events: Array<Record<string, unknown>> } {
  const result = spawnSync(nativeExecutable, [WORKER_ARGUMENT], {
    input: `${JSON.stringify(request)}\n`,
    encoding: "utf8",
  });
  const events = result.stdout
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line) => JSON.parse(line) as Record<string, unknown>);
  return { status: result.status, events };
}

let folder: string;

beforeAll(() => {
  folder = temporaryFolder();
});

afterAll(() => {
  removeFolder(folder);
});

describe("protocol versions", () => {
  it("refuses another version before opening the database", () => {
    const path = join(folder, "never-created.ath.sqlite");
    const { status, events } = runWorker({
      protocol: PROTOCOL_VERSION + 1,
      id: "1",
      op: "database.create",
      input: { path },
    });
    expect(status).toBe(2);
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({ type: "error", error: { code: "protocol_version" } });
    expect(existsSync(path)).toBe(false);
  });

  it("refuses a request without a version", () => {
    const { status, events } = runWorker({ id: "1", op: "ids.uuid_v7", input: {} });
    expect(status).toBe(2);
    expect(events[0]).toMatchObject({ error: { code: "protocol_version" } });
  });

  it("refuses an operation that is not defined, such as raw SQL", () => {
    const { events } = runWorker({ protocol: PROTOCOL_VERSION, id: "4", op: "sql.execute", input: {} });
    expect(events[0]).toMatchObject({ id: "4", type: "error", error: { code: "protocol" } });
  });

  it("answers with the request ID and version", () => {
    const { status, events } = runWorker({ protocol: PROTOCOL_VERSION, id: "abc", op: "ids.uuid_v7", input: {} });
    expect(status).toBe(0);
    expect(events).toEqual([
      expect.objectContaining({ protocol: PROTOCOL_VERSION, id: "abc", type: "result" }),
    ]);
  });

  it("generates one dispatch entry per operation and no SQL operation", () => {
    const names = Object.keys(OPERATIONS);
    expect(names.length).toBeGreaterThan(60);
    expect(names.some((name) => /^sql|execute|action/i.test(name))).toBe(false);
    for (const [name, definition] of Object.entries(OPERATIONS)) {
      if (definition.kind === "sync") {
        expect(definition.target, name).toBe("none");
        expect(definition.callbacks, name).toEqual([]);
      }
    }
  });
});

describe("workers", () => {
  it("runs a synchronous export in a synchronous child process", () => {
    expect(callSync("ids.uuid_v7", { now: 0 })).toMatch(/^00000000-0000-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
  });

  it("streams a large result one record per line", async () => {
    const handle = await database.createDatabase(join(folder, "stream.ath.sqlite"));
    const inputs = Array.from({ length: 3_000 }, (_, index) => ({
      record_type: "athlete_report" as const,
      data: noteData(new Date(Date.UTC(2026, 0, 1, 0, 0, index)).toISOString()),
      changed_by: athlete,
    }));
    await database.writeRecords(handle, inputs as never);
    const records = await database.listRecords(handle, { limit: 10_000 });
    expect(records).toHaveLength(3_000);
    expect(JSON.stringify(records).length).toBeGreaterThan(64 * 1024);
    handle.close();
  });

  it("cancels a write without leaving a partial change", async () => {
    const handle = await database.createDatabase(join(folder, "cancel.ath.sqlite"));
    const before = await database.readDatabaseMetadata(handle);
    const inputs = Array.from({ length: 20_000 }, (_, index) => ({
      record_type: "athlete_report",
      data: noteData(new Date(Date.UTC(2026, 0, 1, 0, 0, index)).toISOString()),
      changed_by: athlete,
    }));
    const controller = new AbortController();
    const target = {
      path: handle.localPath!,
      allow_outdated_schema: false,
      database_id: before.database_id,
    };
    let writing = false;
    const running = callAsync(
      "records.write",
      { inputs },
      {
        target,
        signal: controller.signal,
        // The worker reports the write phase after every input passed its
        // checks, just before its one transaction.
        onProgress: (progress) => {
          if ((progress as { phase?: string }).phase === "write") {
            writing = true;
            controller.abort(new Error("stop"));
          }
        },
      },
    );
    await expect(running).rejects.toThrow("stop");
    expect(writing).toBe(true);
    // Let the worker finish exiting, then read with a new worker.
    await new Promise((resolve) => setTimeout(resolve, 500));
    const stats = await database.databaseStats(handle);
    expect(stats.current_records.total).toBe(0);
    expect(stats.database_revision).toBe(0);
    expect((await database.checkDatabase(handle)).valid).toBe(true);
    handle.close();
  });

  it("refuses a handle whose path now holds another database", async () => {
    const path = join(folder, "replaced.ath.sqlite");
    const handle = await database.createDatabase(path);
    const opened = await database.readDatabaseMetadata(handle);
    const otherPath = join(folder, "other.ath.sqlite");
    const other = await database.createDatabase(otherPath);
    const otherId = (await database.readDatabaseMetadata(other)).database_id;
    other.close();
    expect(otherId).not.toBe(opened.database_id);

    // Put the other database's file at the handle's path.
    for (const suffix of ["-wal", "-shm"]) {
      rmSync(`${path}${suffix}`, { force: true });
      rmSync(`${otherPath}${suffix}`, { force: true });
    }
    renameSync(otherPath, path);
    const before = readFileSync(path);
    const refusal = `is not the one this handle opened: database ID ${otherId} differs from ${opened.database_id}`;
    await expect(database.databaseStats(handle)).rejects.toThrow(refusal);
    const writeError = await database
      .writeRecord(handle, { record_type: "athlete_report", data: noteData(), changed_by: athlete } as never)
      .then(
        () => undefined,
        (caught: unknown) => caught as Error & { code?: string },
      );
    expect(writeError?.message).toContain(refusal);
    expect(writeError?.code).toBe("ATH_REFUSED");
    expect(readFileSync(path).equals(before)).toBe(true);
    // A new handle opens the file that is there now.
    const reopened = await database.openDatabase(path);
    expect((await database.readDatabaseMetadata(reopened)).database_id).toBe(otherId);
    reopened.close();
    handle.close();
  });

  it("reports a worker that exits before it answers", async () => {
    const previous = process.env.ATH_NATIVE_EXECUTABLE;
    process.env.ATH_NATIVE_EXECUTABLE = process.platform === "win32" ? "cmd.exe" : "/usr/bin/true";
    try {
      await expect(callAsync("database.inspect", { path: join(folder, "x.ath.sqlite") })).rejects.toThrow(
        /ath worker exited with status 0 before it answered|protocol mismatch/,
      );
    } finally {
      process.env.ATH_NATIVE_EXECUTABLE = previous;
    }
  });
});
