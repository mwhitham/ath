/**
 * Test servers: a sign-in key server and a real `ath-host` in directory mode.
 *
 * The tests run the executable named by ATH_HOST_EXECUTABLE, or the debug
 * build that `cargo build` makes at the repository root. Each run makes a new
 * Ed25519 key. It signs only these tests' tokens.
 */
import { spawn } from "node:child_process";
import { generateKeyPairSync, sign } from "node:crypto";
import { existsSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createServer as createNetServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
export const ISSUER = "https://sign-in.test";
export const AUDIENCE = "https://host.test";

const hostName = process.platform === "win32" ? "ath-host.exe" : "ath-host";
export const hostExecutable = process.env.ATH_HOST_EXECUTABLE || join(repositoryRoot, "target", "debug", hostName);
export const hostMissing = existsSync(hostExecutable)
  ? undefined
  : `ath-host is missing at ${hostExecutable}. Run cargo build at the repository root, or set ATH_HOST_EXECUTABLE.`;

const base64url = (value) => Buffer.from(typeof value === "string" ? value : JSON.stringify(value)).toString("base64url");

function listen(server) {
  return new Promise((done, fail) => {
    server.once("error", fail);
    server.listen(0, "127.0.0.1", () => done(server.address().port));
  });
}

/** A key server that publishes a new Ed25519 key, and tokens signed with it. */
export async function startSignIn() {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const jwk = { ...publicKey.export({ format: "jwk" }), kid: "test-key", alg: "EdDSA", use: "sig" };
  const server = createServer((_request, response) => {
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({ keys: [jwk] }));
  });
  const port = await listen(server);
  return {
    jwksUri: `http://127.0.0.1:${port}/jwks.json`,
    /** An access token for `subject`, issued to the app `client`. */
    token(subject, client) {
      const header = base64url({ alg: "EdDSA", kid: "test-key", typ: "JWT" });
      const exp = Math.floor(Date.now() / 1000) + 3600;
      const payload = base64url({ iss: ISSUER, sub: subject, aud: AUDIENCE, azp: client, name: subject, exp });
      const input = `${header}.${payload}`;
      return `${input}.${sign(null, Buffer.from(input), privateKey).toString("base64url")}`;
    },
    stop: () => new Promise((done) => server.close(() => done())),
  };
}

async function freePort() {
  const server = createNetServer();
  const port = await listen(server);
  await new Promise((done) => server.close(() => done()));
  return port;
}

/** A running `ath-host` with its own data folder. */
export async function startHost(signIn) {
  // The library refuses a path through a link, so use the temporary folder's real path.
  const folder = mkdtempSync(join(realpathSync(tmpdir()), "ath-mcp-host-"));
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
      operator_disclosure: "Test host run by the MCP example's tests, storing records on this machine.",
      open_enrollment: true,
    }),
  );
  const child = spawn(hostExecutable, ["serve", "--config", config], { stdio: ["ignore", "ignore", "pipe"] });
  await new Promise((done, fail) => {
    let log = "";
    const timer = setTimeout(() => fail(new Error(`ath-host did not start:\n${log}`)), 30_000);
    child.stderr.on("data", (chunk) => {
      log += chunk;
      if (log.includes("listening on")) {
        clearTimeout(timer);
        done();
      }
    });
    child.once("exit", (code) => {
      clearTimeout(timer);
      fail(new Error(`ath-host stopped with ${code}:\n${log}`));
    });
  });
  return {
    url: `http://127.0.0.1:${port}`,
    async stop() {
      if (child.exitCode === null) {
        const exited = new Promise((done) => child.once("exit", done));
        child.kill();
        await exited;
      }
      rmSync(folder, { recursive: true, force: true });
    },
  };
}

/** A rowing or running workout from `sourceId` on the given day. */
export function workout(sourceId, day, activity) {
  const stamp = (time) => ({
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
