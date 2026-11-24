#!/usr/bin/env node
/**
 * An MCP server with two tools that work through an Athletic Standard host:
 * `list_workout_summaries` and `add_coaching_note`.
 *
 * Each tool calls the host API with the signed-in person's access token, so the
 * host applies that person's grants. The server never receives a database
 * credential (D266).
 *
 * Run over stdio:
 *   ATH_HOST_URL      the host's address, such as https://records.example.org
 *   ATH_HOST_TOKEN    the person's access token for the host
 *   ATH_HOST_TOKEN_FILE  instead of ATH_HOST_TOKEN: a file holding the token,
 *                     read before every request, so another program can renew it
 */
import { readFileSync, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
// This file runs from the repository against the package built in
// packages/npm/dist. In your own server, install athleticstandard and import
// from "athleticstandard" instead.
import { connectHost, HostError } from "../../packages/npm/dist/index.js";

/** The largest page the tool asks for. The host may cap it lower. */
const MAX_LIMIT = 200;

const workoutSummary = z.object({
  record_id: z.string(),
  revision: z.number(),
  start_at: z.string(),
  end_at: z.string(),
  activity_type: z.string().nullable(),
});

/** Plain words for a host refusal, for the model and the person reading along. */
function refusal(error, athleteId) {
  switch (error.code) {
    case "not_found":
      return `Athlete ${athleteId} has not shared a record with this app, or revoked access.`;
    case "permission_denied":
      return `Athlete ${athleteId} did not grant this app that permission.`;
    case "sign_in_required":
      return `The host did not accept the access token: ${error.message}`;
    case "unreachable":
      return `${error.message}. Try again later.`;
    default:
      return `The host refused the request: ${error.message} (${error.code})`;
  }
}

/** A HostError, also from another copy of the package than this file imports. */
function isHostError(error) {
  return error instanceof HostError || error?.name === "HostError";
}

/** Run a host call. A host refusal becomes a tool error the model can read. */
async function tool(athleteId, work) {
  try {
    return await work();
  } catch (error) {
    if (!isHostError(error)) throw error;
    return { isError: true, content: [{ type: "text", text: refusal(error, athleteId) }] };
  }
}

/**
 * The MCP server for one person's connection to a host.
 *
 * @param {{ host: import("../../packages/npm/dist/index.js").HostConnection }} options
 */
export function createServer({ host }) {
  const server = new McpServer({ name: "ath-host-mcp-example", version: "0.0.0" });

  server.registerTool(
    "list_workout_summaries",
    {
      title: "List workout summaries",
      description:
        "List an athlete's workouts, newest first: record ID, revision, start, end, and activity type. " +
        "Only athletes who granted this app read access to workout summaries are visible.",
      inputSchema: {
        athlete_id: z.string().min(1).describe("The athlete's ID from the host, such as ath_..."),
        limit: z.number().int().min(1).max(MAX_LIMIT).optional().describe("How many workouts to list."),
      },
      outputSchema: {
        workouts: z.array(workoutSummary),
        next: z.string().nullable().describe("The last record ID on this page when more workouts exist."),
      },
      annotations: { readOnlyHint: true, openWorldHint: true },
    },
    async ({ athlete_id, limit }) =>
      tool(athlete_id, async () => {
        const page = await host.workouts(athlete_id, limit === undefined ? {} : { limit });
        const lines = page.workouts.map(
          (workout) =>
            `${workout.start_at} to ${workout.end_at}: ${workout.activity_type ?? "activity not named"} ` +
            `(record ${workout.record_id}, revision ${workout.revision})`,
        );
        const text = lines.length === 0 ? `Athlete ${athlete_id} has no workouts yet.` : lines.join("\n");
        return { content: [{ type: "text", text }], structuredContent: page };
      }),
  );

  server.registerTool(
    "add_coaching_note",
    {
      title: "Add a coaching note",
      description:
        "Add a coaching note to an athlete's record. The host records the signed-in coach as its author. " +
        "It needs the athlete's grant to add coach notes.",
      inputSchema: {
        athlete_id: z.string().min(1).describe("The athlete's ID from the host, such as ath_..."),
        note: z.string().min(1).describe("The note's text."),
        title: z.string().min(1).optional().describe("A short title."),
      },
      outputSchema: {
        record_id: z.string(),
        revision: z.number(),
        database_revision: z.number(),
      },
      annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: true },
    },
    async ({ athlete_id, note, title }) =>
      tool(athlete_id, async () => {
        const request = title === undefined ? { note } : { note, title };
        // One request key per tool call. A lost reply is retried once with the
        // same key, so the host returns the first result instead of a second note.
        const idempotencyKey = crypto.randomUUID();
        let added;
        try {
          added = await host.addNote(athlete_id, request, { idempotencyKey });
        } catch (error) {
          if (!isHostError(error) || error.code !== "unreachable") throw error;
          added = await host.addNote(athlete_id, request, { idempotencyKey });
        }
        const record = added.records[0];
        return {
          content: [{ type: "text", text: `Added the note to athlete ${athlete_id} as record ${record.record_id}.` }],
          structuredContent: {
            record_id: record.record_id,
            revision: record.revision,
            database_revision: added.database_revision,
          },
        };
      }),
  );

  return server;
}

function setting(name) {
  const value = process.env[name]?.trim();
  return value === undefined || value === "" ? undefined : value;
}

/** Serve over stdio with the token from the environment. */
async function main() {
  const url = setting("ATH_HOST_URL");
  const token = setting("ATH_HOST_TOKEN");
  const tokenFile = setting("ATH_HOST_TOKEN_FILE");
  if (url === undefined || (token === undefined && tokenFile === undefined)) {
    // stdout carries MCP messages, so messages for people go to stderr.
    console.error("Set ATH_HOST_URL, and ATH_HOST_TOKEN or ATH_HOST_TOKEN_FILE. See README.md.");
    process.exit(2);
  }
  const host = connectHost({
    url,
    token: token ?? (() => readFileSync(tokenFile, "utf8").trim()),
  });
  await createServer({ host }).connect(new StdioServerTransport());
}

const started = process.argv[1] === undefined ? undefined : realpathSync(process.argv[1]);
if (started === realpathSync(fileURLToPath(import.meta.url))) {
  await main();
}
