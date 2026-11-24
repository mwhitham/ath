/**
 * The MCP server against a real `ath-host`: an in-memory MCP client calls both
 * tools as a coach, and the host applies that coach's grant. One test uses a
 * stand-in host to lose a reply.
 */
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { after, before, describe, it } from "node:test";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { getDefaultEnvironment, StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { hostMissing, repositoryRoot, startHost, startSignIn, workout } from "./support.mjs";

const clientModule = join(repositoryRoot, "packages", "npm", "dist", "index.js");
const unbuilt = existsSync(clientModule)
  ? undefined
  : `The npm package is not built at ${clientModule}. Run (cd packages/npm && npm ci && npm run build).`;

/** An MCP client connected in memory to a server for `host`. */
async function connect(host) {
  const { createServer } = await import("../server.mjs");
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  await createServer({ host }).connect(serverTransport);
  const client = new Client({ name: "ath-host-mcp-test", version: "0.0.0" });
  await client.connect(clientTransport);
  return client;
}

describe("MCP server retries", { skip: unbuilt }, () => {
  it("sends a lost note again with the same request key", async () => {
    const { HostError } = await import(clientModule);
    const keys = [];
    const host = {
      async addNote(_athleteId, _note, { idempotencyKey }) {
        keys.push(idempotencyKey);
        if (keys.length === 1) throw new HostError(0, "unreachable", "No reply", { idempotencyKey });
        return { database_revision: 3, records: [{ record_id: "note-1", revision: 1 }] };
      },
    };
    const client = await connect(host);
    const result = await client.callTool({ name: "add_coaching_note", arguments: { athlete_id: "ath_1", note: "x" } });
    assert.deepEqual(result.structuredContent, { record_id: "note-1", revision: 1, database_revision: 3 });
    assert.equal(keys.length, 2);
    assert.equal(keys[0], keys[1]);
    await client.close();
  });
});

describe("MCP server through ath-host", { skip: hostMissing ?? unbuilt }, () => {
  let signIn;
  let host;
  let owner;
  let athleteId;
  let otherAthleteId;
  let grantId;
  let coachToken;
  let mcp;

  before(async () => {
    const { connectHost } = await import(clientModule);
    signIn = await startSignIn();
    host = await startHost(signIn);

    // An athlete with two workouts grants the coach's app two permissions.
    owner = connectHost({ url: host.url, token: signIn.token("athlete-1", "athlete-app") });
    ({ athlete_id: athleteId } = await owner.createAthlete({ display_name: "Coached athlete" }));
    const source = await owner.createRecord(athleteId, {
      record_type: "source",
      data: { source_type: "manual", data_writer: "ath", data_route: "manual_entry" },
    });
    const sourceId = source.records[0].record_id;
    for (const [day, activity] of [["2026-11-18", "rowing"], ["2026-11-20", "running"]]) {
      await owner.createRecord(athleteId, { record_type: "workout_session", data: workout(sourceId, day, activity) });
    }
    const offer = await owner.createGrantOffer(athleteId, {
      permissions: ["read_workout_summaries", "add_coach_notes"],
      client_id: "coach-mcp",
      coach_label: "Coach Sam",
    });

    // Another athlete on the same host shares nothing.
    const other = connectHost({ url: host.url, token: signIn.token("athlete-2", "athlete-app") });
    ({ athlete_id: otherAthleteId } = await other.createAthlete());

    // The coach signs in with the app the athlete named. The server acts with that token.
    coachToken = signIn.token("coach-1", "coach-mcp");
    const coach = connectHost({ url: host.url, token: coachToken });
    ({
      grant: { grant_id: grantId },
    } = await coach.acceptGrantOffer(offer.code));
    mcp = await connect(coach);
  });

  after(async () => {
    await mcp?.close();
    await host?.stop();
    await signIn?.stop();
  });

  it("offers the two tools", async () => {
    const { tools } = await mcp.listTools();
    assert.deepEqual(tools.map((tool) => tool.name).sort(), ["add_coaching_note", "list_workout_summaries"]);
  });

  it("lists workout summaries, newest first, a page at a time", async () => {
    const first = await mcp.callTool({ name: "list_workout_summaries", arguments: { athlete_id: athleteId, limit: 1 } });
    assert.equal(first.isError, undefined);
    assert.deepEqual(
      first.structuredContent.workouts.map((summary) => summary.activity_type),
      ["running"],
    );
    assert.deepEqual(Object.keys(first.structuredContent.workouts[0]).sort(), [
      "activity_type",
      "end_at",
      "record_id",
      "revision",
      "start_at",
    ]);
    assert.match(first.content[0].text, /^2026-11-20T07:00:00\.000Z to 2026-11-20T07:45:00\.000Z: running/);
    assert.equal(typeof first.structuredContent.next, "string");

    const all = await mcp.callTool({ name: "list_workout_summaries", arguments: { athlete_id: athleteId } });
    assert.equal(all.structuredContent.workouts.length, 2);
    assert.equal(all.structuredContent.next, null);
  });

  it("adds a coaching note with the coach as author", async () => {
    const added = await mcp.callTool({
      name: "add_coaching_note",
      arguments: { athlete_id: athleteId, note: "Keep the easy runs easy this week.", title: "Pacing" },
    });
    assert.equal(added.isError, undefined, added.content[0]?.text);
    const { record } = await owner.getRecord(athleteId, added.structuredContent.record_id);
    assert.equal(record.record_type, "coaching_note");
    assert.equal(record.data.note, "Keep the easy runs easy this week.");
    assert.equal(record.data.title, "Pacing");
    assert.equal(record.data.authorship.author.author_type, "coach");
    assert.equal(record.data.authorship.author.coach, "Coach Sam");
  });

  it("reports an athlete who shared nothing as a tool error", async () => {
    const result = await mcp.callTool({ name: "list_workout_summaries", arguments: { athlete_id: otherAthleteId } });
    assert.equal(result.isError, true);
    assert.match(result.content[0].text, /has not shared a record with this app/);
  });

  it("serves over stdio with the token from the environment", async () => {
    const stdio = new Client({ name: "ath-host-mcp-stdio-test", version: "0.0.0" });
    await stdio.connect(
      new StdioClientTransport({
        command: process.execPath,
        args: [fileURLToPath(new URL("../server.mjs", import.meta.url))],
        env: { ...getDefaultEnvironment(), ATH_HOST_URL: host.url, ATH_HOST_TOKEN: coachToken },
        stderr: "pipe",
      }),
    );
    try {
      const result = await stdio.callTool({ name: "list_workout_summaries", arguments: { athlete_id: athleteId } });
      assert.equal(result.isError, undefined, result.content[0]?.text);
      assert.equal(result.structuredContent.workouts.length, 2);
    } finally {
      await stdio.close();
    }
  });

  it("stops at the next call after the athlete revokes the grant", async () => {
    await owner.revokeGrant(athleteId, grantId);
    for (const [name, args] of [
      ["list_workout_summaries", { athlete_id: athleteId }],
      ["add_coaching_note", { athlete_id: athleteId, note: "Late." }],
    ]) {
      const result = await mcp.callTool({ name, arguments: args });
      assert.equal(result.isError, true, name);
      assert.match(result.content[0].text, /revoked access/);
    }
  });
});
