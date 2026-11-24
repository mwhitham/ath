/**
 * Test servers for the host client: a sign-in key server and a real `ath-host`.
 *
 * The tests run the executable named by ATH_HOST_EXECUTABLE. Without it they
 * use the debug build in target/, which `cargo build` makes at the repository
 * root. Each run makes a new Ed25519 key. It signs only these tests' tokens.
 */
import { spawn, type ChildProcess } from "node:child_process";
import { generateKeyPairSync, sign, type KeyObject } from "node:crypto";
import { existsSync, mkdtempSync, realpathSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { createServer as createNetServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { removeFolder, repositoryRoot } from "./support.js";

export const ISSUER = "https://sign-in.test";
export const AUDIENCE = "https://host.test";
export const ATHLETE_APP = "athlete-app";
export const COACH_APP = "coach-app";

const hostName = process.platform === "win32" ? "ath-host.exe" : "ath-host";
export const hostExecutable =
  process.env.ATH_HOST_EXECUTABLE !== undefined && process.env.ATH_HOST_EXECUTABLE !== ""
    ? process.env.ATH_HOST_EXECUTABLE
    : join(repositoryRoot, "target", "debug", hostName);
export const hostAvailable = existsSync(hostExecutable);
export const hostMissing =
  `ath-host is missing at ${hostExecutable}. ` +
  "Run cargo build at the repository root, or set ATH_HOST_EXECUTABLE.";

function base64url(value: string | object): string {
  return Buffer.from(typeof value === "string" ? value : JSON.stringify(value)).toString("base64url");
}

/** A key server that publishes a new Ed25519 key, and tokens signed with it. */
export class SignIn {
  readonly jwksUri: string;
  readonly #server: Server;
  readonly #key: KeyObject;

  private constructor(jwksUri: string, server: Server, key: KeyObject) {
    this.jwksUri = jwksUri;
    this.#server = server;
    this.#key = key;
  }

  static async start(): Promise<SignIn> {
    const { publicKey, privateKey } = generateKeyPairSync("ed25519");
    const jwk = { ...publicKey.export({ format: "jwk" }), kid: "test-key", alg: "EdDSA", use: "sig" };
    const server = createServer((_request, response) => {
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ keys: [jwk] }));
    });
    const port = await listen(server);
    return new SignIn(`http://127.0.0.1:${port}/jwks.json`, server, privateKey);
  }

  /** An access token for `subject`, issued to the app `client`. */
  token(subject: string, client: string, claims: Record<string, unknown> = {}): string {
    const header = base64url({ alg: "EdDSA", kid: "test-key", typ: "JWT" });
    const payload = base64url({
      iss: ISSUER,
      sub: subject,
      aud: AUDIENCE,
      azp: client,
      name: subject,
      exp: Math.floor(Date.now() / 1000) + 3600,
      ...claims,
    });
    const input = `${header}.${payload}`;
    return `${input}.${sign(null, Buffer.from(input), this.#key).toString("base64url")}`;
  }

  async stop(): Promise<void> {
    await new Promise<void>((resolve) => this.#server.close(() => resolve()));
  }
}

function listen(server: Server): Promise<number> {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      resolve(typeof address === "object" && address !== null ? address.port : 0);
    });
  });
}

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createNetServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address !== null ? address.port : 0;
      server.close(() => resolve(port));
    });
  });
}

/** A running `ath-host` in directory mode, with its own data folder. */
export class TestHost {
  readonly url: string;
  readonly #child: ChildProcess;
  readonly #folder: string;

  private constructor(url: string, child: ChildProcess, folder: string) {
    this.url = url;
    this.#child = child;
    this.#folder = folder;
  }

  static async start(signIn: SignIn): Promise<TestHost> {
    // The library refuses a path through a link, and the temporary folder is
    // often one on macOS, so use its real path.
    const folder = mkdtempSync(join(realpathSync(tmpdir()), "ath-npm-host-"));
    const port = await freePort();
    const config = join(folder, "host.json");
    writeFileSync(
      config,
      JSON.stringify({
        listen: `127.0.0.1:${port}`,
        public_url: AUDIENCE,
        data_directory: join(folder, "data"),
        storage: { kind: "directory" },
        sign_in: { issuer: ISSUER, audience: AUDIENCE, jwks_uri: signIn.jwksUri },
        operator_disclosure: "Test host run by the npm tests, storing records on this machine.",
        open_enrollment: true,
      }),
    );
    const child = spawn(hostExecutable, ["serve", "--config", config], { stdio: ["ignore", "ignore", "pipe"] });
    await new Promise<void>((resolve, reject) => {
      let log = "";
      const timer = setTimeout(() => reject(new Error(`ath-host did not start:\n${log}`)), 30_000);
      child.stderr?.on("data", (chunk: Buffer) => {
        log += chunk.toString();
        if (log.includes("listening on")) {
          clearTimeout(timer);
          resolve();
        }
      });
      child.once("exit", (code) => {
        clearTimeout(timer);
        reject(new Error(`ath-host stopped with ${String(code)}:\n${log}`));
      });
    });
    return new TestHost(`http://127.0.0.1:${port}`, child, folder);
  }

  async stop(): Promise<void> {
    if (this.#child.exitCode === null) {
      const exited = new Promise((resolve) => this.#child.once("exit", resolve));
      this.#child.kill();
      await exited;
    }
    removeFolder(this.#folder);
  }
}

/** The demo record's photo, a small PNG. */
export const demoPhotoPath = join(
  repositoryRoot,
  "examples",
  "demo-athlete",
  "attachments",
  "01890f3e-e7d3-7cc0-88c8-00000000000e.png",
);

/** A manual source, as an athlete's own app would write it. */
export const manualSource = { source_type: "manual", data_writer: "ath", data_route: "manual_entry" };

/** A rowing workout from `sourceId` on the given day. */
export function workout(sourceId: string, day: string, activity = "rowing"): Record<string, unknown> {
  const stamp = (time: string) => ({
    timestamp_text: `${day}T${time}Z`,
    utc_offset_text: "Z",
    utc_offset_meaning: "meaning_not_documented",
    timestamp_basis: "source_offset_timestamp",
  });
  return {
    source_id: sourceId,
    received_at: `${day}T08:00:00.000Z`,
    received_at_basis: "local_write_time",
    start_at: `${day}T07:00:00.000Z`,
    end_at: `${day}T07:45:00.000Z`,
    input_start_timestamp: stamp("07:00:00"),
    input_end_timestamp: stamp("07:45:00"),
    activity_type: activity,
    source_reported_summaries: [],
  };
}

/** An athlete report, as an owner would log it. */
export function report(note: string): Record<string, unknown> {
  return {
    report_type: "note",
    reported_at: "2026-11-21T08:00:00.000Z",
    input_reported_at: { timestamp_missing_reason: "not_applicable", utc_offset_missing_reason: "not_applicable" },
    reported_via: "text",
    note,
  };
}
