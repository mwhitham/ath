#!/usr/bin/env node
/**
 * A coach's steps against an Athletic Standard host, with the npm host client.
 *
 * It accepts an athlete's offer when given a code, lists the athletes who
 * shared a record with this app, reads one athlete's workout summaries, and
 * adds a note when given one. It prints each step in plain words.
 *
 * Environment:
 *   ATH_HOST_URL     the host's address, such as https://records.example.org
 *   ATH_HOST_TOKEN   the coach's access token, issued to this app
 *   ATH_GRANT_CODE   optional: an athlete's offer code to accept first
 *   ATH_ATHLETE_ID   optional: the athlete to work with; the first on the roster otherwise
 *   ATH_NOTE         optional: a note to add, citing the latest workout
 *   ATH_NOTE_KEY     optional: the request key for the note; reuse it to retry
 */

// This file runs from the repository against the package built in
// packages/npm/dist. In your own app, install athleticstandard and import
// from "athleticstandard" instead.
import { connectHost, HostError } from "../../packages/npm/dist/index.js";

function setting(name) {
  const value = process.env[name]?.trim();
  return value === undefined || value === "" ? undefined : value;
}

function required(name) {
  const value = setting(name);
  if (value === undefined) {
    console.error(`Set ${name}. The README explains each setting.`);
    process.exit(2);
  }
  return value;
}

const host = connectHost({ url: required("ATH_HOST_URL"), token: required("ATH_HOST_TOKEN") });
// The key makes a retry safe: the host returns the first result for the same
// key and body, and writes nothing new.
const noteKey = setting("ATH_NOTE_KEY") ?? crypto.randomUUID();

async function run() {
  const capabilities = await host.capabilities();
  console.log(`Connected to ${host.url} (API version ${capabilities.api_version}).`);
  console.log(`Who runs this host: ${capabilities.operator_disclosure}`);

  const code = setting("ATH_GRANT_CODE");
  if (code !== undefined) {
    const { grant } = await host.acceptGrantOffer(code);
    console.log(`Accepted the offer for athlete ${grant.resource_id}: ${grant.permissions.join(", ")}.`);
  }

  const { athletes } = await host.roster();
  if (athletes.length === 0) {
    console.log("No athlete shares a record with this app.");
    return;
  }
  console.log("Athletes who share a record with this app:");
  for (const athlete of athletes) {
    console.log(`  ${athlete.athlete_id} ${athlete.display_name ?? "(no name)"}: ${athlete.permissions.join(", ")}`);
  }

  const chosen = setting("ATH_ATHLETE_ID") ?? athletes[0].athlete_id;
  const athlete = athletes.find((entry) => entry.athlete_id === chosen);
  if (athlete === undefined) {
    console.log(`Athlete ${chosen} is not on this app's roster.`);
    process.exitCode = 1;
    return;
  }

  let latest;
  if (athlete.permissions.includes("read_workout_summaries")) {
    const { workouts } = await host.workouts(athlete.athlete_id, { limit: 5 });
    console.log(`Latest workouts for ${athlete.athlete_id}:`);
    for (const workout of workouts) {
      console.log(`  ${workout.start_at} to ${workout.end_at}: ${workout.activity_type ?? "activity not named"}`);
    }
    if (workouts.length === 0) console.log("  none yet");
    latest = workouts[0];
  } else {
    console.log(`This app may not read workouts for ${athlete.athlete_id}.`);
  }

  const note = setting("ATH_NOTE");
  if (note === undefined) {
    console.log("Set ATH_NOTE to add a note.");
    return;
  }
  if (!athlete.permissions.includes("add_coach_notes")) {
    console.log(`This app may not add notes for ${athlete.athlete_id}.`);
    return;
  }
  const evidence = latest === undefined ? [] : [{ record_id: latest.record_id, revision: latest.revision }];
  const added = await host.addNote(athlete.athlete_id, { note, evidence }, { idempotencyKey: noteKey });
  const cited = latest === undefined ? "" : `, citing the workout that started ${latest.start_at}`;
  console.log(`Added note ${added.records[0].record_id}${cited}.`);
  console.log(`The request key was ${noteKey}. Run again with ATH_NOTE_KEY=${noteKey} to retry without a second note.`);
}

try {
  await run();
} catch (error) {
  if (!(error instanceof HostError)) throw error;
  if (error.code === "unreachable") {
    console.error(`${error.message}.`);
  } else {
    console.error(`The host refused a step: ${error.message} (${error.code}, status ${error.status}).`);
  }
  if (error.idempotencyKey === noteKey) {
    console.error(`To retry the note without a second copy, set ATH_NOTE_KEY=${noteKey}.`);
  }
  process.exitCode = 1;
}
