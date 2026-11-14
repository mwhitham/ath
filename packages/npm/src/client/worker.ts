/**
 * The transport to the native library: one `ath __client` worker per call.
 *
 * A request is one JSON line on the worker's standard input. The worker
 * answers with JSON lines: items, progress, callback requests, and one final
 * result or error. Lines are parsed as they arrive, so a large result streams
 * through a bounded pipe instead of one buffered document. Callbacks run here
 * and their answers go back on standard input. Aborting the signal cancels the
 * operation; the worker stops before committing anything further.
 *
 * Synchronous exports use a synchronous child process with the same messages.
 */
import { spawn, spawnSync } from "node:child_process";
import { StringDecoder } from "node:string_decoder";
import {
  OPERATIONS,
  PROTOCOL_VERSION,
  WORKER_ARGUMENT,
  type AsyncOperationName,
  type OperationTypes,
  type SyncOperationName,
} from "../generated/protocol.js";
import { resolveNativeExecutable } from "../native-binary.js";
import { errorFromWorker, type WorkerError } from "./errors.js";

/** The database a request names. Each request opens and checks it again. */
export interface WorkerTarget {
  path: string;
  allow_outdated_schema: boolean;
  database_id: string | null;
}

export interface CallOptions<Name extends AsyncOperationName> {
  target?: WorkerTarget;
  callbacks?: OperationTypes[Name]["callbacks"];
  onProgress?: (progress: unknown) => void;
  signal?: AbortSignal;
}

export interface CallResult<Name extends AsyncOperationName> {
  output: OperationTypes[Name]["output"];
  items: Array<OperationTypes[Name]["item"]>;
  /** The database ID after the operation, when it used a database. */
  database_id?: string;
}

interface WorkerEvent {
  protocol?: unknown;
  id?: unknown;
  type?: unknown;
  output?: unknown;
  item?: unknown;
  progress?: unknown;
  error?: WorkerError;
  callback_id?: unknown;
  name?: unknown;
  input?: unknown;
  database_id?: unknown;
}

/** Diagnostics kept from a worker's standard error. */
const STDERR_LIMIT = 64 * 1024;
/** How long a cancelled worker may take to stop before it is killed. */
const CANCEL_GRACE_MS = 5_000;

let nextRequestId = 0;

function requestLine(
  id: string,
  op: string,
  input: unknown,
  target: WorkerTarget | undefined,
): string {
  return `${JSON.stringify({
    protocol: PROTOCOL_VERSION,
    id,
    op,
    input,
    ...(target === undefined ? {} : { target }),
  })}\n`;
}

function protocolMismatch(detail: string): Error {
  return new Error(`ath worker protocol mismatch: ${detail}`);
}

function checkEvent(event: WorkerEvent, id: string): void {
  if (event.protocol !== PROTOCOL_VERSION) {
    throw protocolMismatch(
      `this client speaks protocol ${PROTOCOL_VERSION}, the worker answered ${String(event.protocol)}`,
    );
  }
  if (event.id !== id && event.id !== "") {
    throw protocolMismatch(`answer for request ${String(event.id)}, expected ${id}`);
  }
}

function exitedWithout(status: number | null, signal: NodeJS.Signals | null, stderr: string): Error {
  const how = signal === null ? `status ${String(status)}` : `signal ${signal}`;
  const detail = stderr.trim();
  return new Error(
    `ath worker exited with ${how} before it answered${detail === "" ? "" : `: ${detail}`}`,
  );
}

/** Run one asynchronous operation in its own worker. */
export function callAsync<Name extends AsyncOperationName>(
  name: Name,
  input: OperationTypes[Name]["input"],
  options: CallOptions<Name> = {},
): Promise<CallResult<Name>> {
  const definition = OPERATIONS[name];
  if (definition.kind !== "async") {
    return Promise.reject(new Error(`${name} is not an asynchronous operation`));
  }
  if (definition.target !== "none" && options.target === undefined) {
    return Promise.reject(new Error(`${name} needs a database target`));
  }
  if (options.signal?.aborted === true) {
    return Promise.reject(options.signal.reason ?? new Error("The operation was aborted"));
  }
  const executable = resolveNativeExecutable();
  nextRequestId += 1;
  const id = String(nextRequestId);
  const line = requestLine(id, name, input, options.target);

  return new Promise<CallResult<Name>>((resolve, reject) => {
    const child = spawn(executable, [WORKER_ARGUMENT], {
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    });
    const decoder = new StringDecoder("utf8");
    const items: Array<OperationTypes[Name]["item"]> = [];
    let pending = "";
    let stderr = "";
    let settled = false;
    let cancelTimer: NodeJS.Timeout | undefined;

    const finish = (error: unknown, value?: CallResult<Name>): void => {
      if (settled) {
        return;
      }
      settled = true;
      options.signal?.removeEventListener("abort", onAbort);
      if (!child.stdin.destroyed) {
        child.stdin.end();
      }
      if (error === undefined) {
        resolve(value!);
      } else {
        reject(error);
      }
    };

    const send = (message: unknown): void => {
      if (!child.stdin.destroyed && child.stdin.writable) {
        child.stdin.write(`${JSON.stringify({ protocol: PROTOCOL_VERSION, id, ...(message as object) })}\n`);
      }
    };

    const onAbort = (): void => {
      if (settled) {
        return;
      }
      send({ type: "cancel" });
      child.stdin.end();
      cancelTimer = setTimeout(() => child.kill("SIGKILL"), CANCEL_GRACE_MS);
      cancelTimer.unref();
      finish(options.signal?.reason ?? new Error("The operation was aborted"));
    };
    options.signal?.addEventListener("abort", onAbort, { once: true });

    const runCallback = async (event: WorkerEvent): Promise<void> => {
      const callbackId = String(event.callback_id);
      const callbacks = (options.callbacks ?? {}) as Record<string, (input: unknown) => Promise<unknown>>;
      const callback = typeof event.name === "string" ? callbacks[event.name] : undefined;
      try {
        if (callback === undefined) {
          throw new Error(`no callback named ${String(event.name)} was supplied`);
        }
        const output = await callback(event.input);
        send({ type: "callback_result", callback_id: callbackId, output: output ?? null });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        send({ type: "callback_error", callback_id: callbackId, error: { message } });
      }
    };

    const handle = (text: string): void => {
      if (settled || text.trim() === "") {
        return;
      }
      let event: WorkerEvent;
      try {
        event = JSON.parse(text) as WorkerEvent;
        checkEvent(event, id);
      } catch (error) {
        child.kill();
        finish(error instanceof SyntaxError ? protocolMismatch("an answer is not JSON") : error);
        return;
      }
      switch (event.type) {
        case "item":
          items.push(event.item as OperationTypes[Name]["item"]);
          return;
        case "progress":
          options.onProgress?.(event.progress);
          return;
        case "callback":
          void runCallback(event);
          return;
        case "result":
          finish(undefined, {
            output: event.output as OperationTypes[Name]["output"],
            items,
            ...(typeof event.database_id === "string" ? { database_id: event.database_id } : {}),
          });
          return;
        case "error":
          finish(errorFromWorker(event.error ?? { code: "error", message: "unknown worker error" }));
          return;
        default:
          child.kill();
          finish(protocolMismatch(`unknown event ${String(event.type)}`));
      }
    };

    child.stdout.on("data", (chunk: Buffer) => {
      pending += decoder.write(chunk);
      let newline = pending.indexOf("\n");
      while (newline !== -1) {
        const text = pending.slice(0, newline);
        pending = pending.slice(newline + 1);
        handle(text);
        newline = pending.indexOf("\n");
      }
    });
    child.stderr.on("data", (chunk: Buffer) => {
      if (stderr.length < STDERR_LIMIT) {
        stderr += chunk.toString("utf8").slice(0, STDERR_LIMIT - stderr.length);
      }
    });
    // A worker that exits closes its input; writing after that is not an error here.
    child.stdin.on("error", () => undefined);
    child.on("error", (error) => finish(error));
    child.on("close", (status, signal) => {
      if (cancelTimer !== undefined) {
        clearTimeout(cancelTimer);
      }
      pending += decoder.end();
      if (pending !== "") {
        handle(pending);
        pending = "";
      }
      finish(exitedWithout(status, signal, stderr));
    });
    child.stdin.write(line);
  });
}

/** Run one synchronous operation in a synchronous child process. */
export function callSync<Name extends SyncOperationName>(
  name: Name,
  input: OperationTypes[Name]["input"],
): OperationTypes[Name]["output"] {
  const definition = OPERATIONS[name];
  if (definition.kind !== "sync") {
    throw new Error(`${name} is not a synchronous operation`);
  }
  const executable = resolveNativeExecutable();
  nextRequestId += 1;
  const id = String(nextRequestId);
  const result = spawnSync(executable, [WORKER_ARGUMENT], {
    input: requestLine(id, name, input, undefined),
    maxBuffer: Number.MAX_SAFE_INTEGER,
    windowsHide: true,
  });
  if (result.error !== undefined) {
    throw result.error;
  }
  const lines = result.stdout.toString("utf8").split("\n").filter((text) => text.trim() !== "");
  for (const text of lines) {
    let event: WorkerEvent;
    try {
      event = JSON.parse(text) as WorkerEvent;
    } catch {
      throw protocolMismatch("an answer is not JSON");
    }
    checkEvent(event, id);
    if (event.type === "result") {
      return event.output as OperationTypes[Name]["output"];
    }
    if (event.type === "error") {
      throw errorFromWorker(event.error ?? { code: "error", message: "unknown worker error" });
    }
  }
  throw exitedWithout(result.status, result.signal, result.stderr.toString("utf8").slice(0, STDERR_LIMIT));
}
