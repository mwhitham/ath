/**
 * The host client against a real `ath-host` in directory mode: an owner's
 * record with a photo, a coach who sees and adds only what was granted, and
 * the starter app in examples/host-starter. The client tests that need no
 * host run everywhere.
 */
import { execFile } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { promisify } from "node:util";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { connectHost, HostError, type HostConnection } from "../src/index.js";
import {
  ATHLETE_APP,
  COACH_APP,
  demoPhotoPath,
  hostAvailable,
  hostMissing,
  manualSource,
  report,
  SignIn,
  TestHost,
  workout,
} from "./host-support.js";
import { repositoryRoot } from "./support.js";

const run = promisify(execFile);

/** The HostError a promise rejects with. */
async function failure(promise: Promise<unknown>): Promise<HostError> {
  const error = await promise.then(
    () => undefined,
    (reason: unknown) => reason,
  );
  expect(error).toBeInstanceOf(HostError);
  return error as HostError;
}

describe("host client without a host", () => {
  it("refuses an address that is not http or https", () => {
    expect(() => connectHost({ url: "libsql://records.example.org", token: "t" })).toThrow(/https:\/\//);
  });

  it("sends a new key with each write unless the caller names one", async () => {
    const sent: Array<{ url: string; method: string; headers: Record<string, string> }> = [];
    const fake: typeof fetch = async (input, init) => {
      sent.push({
        url: String(input),
        method: init?.method ?? "GET",
        headers: init?.headers as Record<string, string>,
      });
      return Response.json({ database_revision: 1, records: [] }, { status: 201 });
    };
    const host = connectHost({ url: "https://records.example.org/", token: () => "token-1", fetch: fake });
    await host.addNote("ath_1", { note: "a" });
    await host.addNote("ath_1", { note: "a" });
    await host.addNote("ath_1", { note: "a" }, { idempotencyKey: "note-1" });
    await host.workouts("ath_1", { limit: 5 });
    expect(sent.map((request) => request.url)).toEqual([
      "https://records.example.org/v1/athletes/ath_1/notes",
      "https://records.example.org/v1/athletes/ath_1/notes",
      "https://records.example.org/v1/athletes/ath_1/notes",
      "https://records.example.org/v1/athletes/ath_1/workouts?limit=5",
    ]);
    const keys = sent.map((request) => request.headers["idempotency-key"]);
    expect(keys[0]).toMatch(/^[0-9a-f-]{36}$/);
    expect(keys[1]).not.toBe(keys[0]);
    expect(keys[2]).toBe("note-1");
    expect(keys[3]).toBeUndefined();
    expect(sent[0]?.headers.authorization).toBe("Bearer token-1");
  });

  it("reports no reply and an unexpected reply with the key that was sent", async () => {
    const down = connectHost({
      url: "https://records.example.org",
      token: "t",
      fetch: async () => {
        throw new TypeError("fetch failed");
      },
    });
    const unreachable = await failure(down.addNote("ath_1", { note: "a" }, { idempotencyKey: "note-1" }));
    expect([unreachable.status, unreachable.code, unreachable.idempotencyKey]).toEqual([0, "unreachable", "note-1"]);

    const proxy = connectHost({
      url: "https://records.example.org",
      token: "t",
      fetch: async () => new Response("Bad gateway", { status: 502 }),
    });
    const unexpected = await failure(proxy.roster());
    expect([unexpected.status, unexpected.code, unexpected.message]).toEqual([
      502,
      "unexpected_response",
      "The host answered 502: Bad gateway",
    ]);
  });
});

const hostTitle = hostAvailable ? "host client with ath-host" : `host client skipped: ${hostMissing}`;

describe.skipIf(!hostAvailable)(hostTitle, () => {
  let signIn: SignIn;
  let server: TestHost;
  const photo = readFileSync(demoPhotoPath);

  beforeAll(async () => {
    signIn = await SignIn.start();
    server = await TestHost.start(signIn);
  }, 60_000);

  afterAll(async () => {
    await server?.stop();
    await signIn?.stop();
  });

  function as(subject: string, app: string): HostConnection {
    return connectHost({ url: server.url, token: signIn.token(subject, app) });
  }

  it("lets an owner save a report with a photo, retry it, export it, and restore it", async () => {
    const owner = as("athlete-1", ATHLETE_APP);
    const capabilities = await owner.capabilities();
    expect(capabilities).toMatchObject({ api_version: 1, athleticstandard_version: "2.1.3", storage: "directory" });

    const { athlete_id: id } = await owner.createAthlete({ display_name: "Test athlete" });
    expect((await owner.me()).athletes).toEqual([id]);
    expect(await owner.athlete(id)).toMatchObject({ athlete_id: id, role: "owner" });

    const uploaded = await owner.upload(id, photo);
    expect(uploaded).toMatchObject({ byte_count: photo.length, media_type: "image/png" });

    const request = {
      record_type: "athlete_report" as const,
      data: report("Felt strong."),
      attachments: [uploaded.upload_id],
    };
    const created = await owner.createRecord(id, request, { idempotencyKey: "report-1" });
    expect(created.records).toHaveLength(2);
    const [attachment, saved] = created.records;

    // A retry after a lost reply returns the same result and writes nothing new.
    expect(await owner.createRecord(id, request, { idempotencyKey: "report-1" })).toEqual(created);
    expect((await owner.listRecords(id, { record_type: "athlete_report" })).records).toHaveLength(1);
    // The same key with a different body is a conflict.
    const conflict = await failure(
      owner.createRecord(id, { record_type: "athlete_report", data: report("Other.") }, { idempotencyKey: "report-1" }),
    );
    expect([conflict.status, conflict.code, conflict.idempotencyKey]).toEqual([409, "request_conflict", "report-1"]);

    const downloaded = await owner.downloadAttachment(id, attachment!.record_id);
    expect(Buffer.from(downloaded.bytes).equals(photo)).toBe(true);
    expect(downloaded.mediaType).toBe("image/png");

    // A correction names the revision it replaces; a stale one is refused.
    const current = await owner.getRecord(id, saved!.record_id);
    const changedData = { ...current.record.data, note: "Felt strong, slept well." };
    const corrected = await owner.correctRecord(id, saved!.record_id, { expected_revision: 1, data: changedData });
    expect(corrected.records).toEqual([{ record_id: saved!.record_id, revision: 2 }]);
    const stale = await failure(owner.correctRecord(id, saved!.record_id, { expected_revision: 1, data: changedData }));
    expect([stale.status, stale.code]).toEqual([409, "stale"]);
    const history = await owner.getRecord(id, saved!.record_id);
    expect(history.record.current_revision).toBe(2);
    expect(history.revisions).toHaveLength(2);

    const exported = await owner.exportPackage(id);
    expect(exported.manifestDigest).toMatch(/^[0-9a-f]{64}$/);
    expect(Buffer.from(exported.bytes.subarray(0, 2)).toString()).toBe("PK");

    const changes = await owner.changes(id, 0);
    expect(changes.complete).toBe(true);
    expect(changes.changes.map((change) => change.record_id)).toEqual([
      attachment!.record_id,
      saved!.record_id,
      saved!.record_id,
    ]);

    // The exported package restores into another empty record.
    const { athlete_id: copy } = await owner.createAthlete();
    await owner.putPackage(copy, exported.bytes);
    expect((await owner.getRecord(copy, saved!.record_id)).record.current_revision).toBe(2);
    const owned = (await owner.listAthletes()).athletes.map((athlete) => athlete.athlete_id);
    expect(owned.sort()).toEqual([id, copy].sort());

    // Deletion keeps history; erasure removes the record.
    await owner.deleteRecord(copy, saved!.record_id, { expected_revision: 2 });
    expect((await owner.listRecords(copy, { record_type: "athlete_report" })).records).toEqual([]);
    const note = await owner.createRecord(id, { record_type: "athlete_report", data: report("Typed by mistake.") });
    await owner.eraseRecord(id, note.records[0]!.record_id, { expected_revision: 1, reason_code: "athlete_request" });
    const erased = await failure(owner.getRecord(id, note.records[0]!.record_id));
    expect([erased.status, erased.code]).toEqual([404, "not_found"]);

    // Someone else cannot tell the record exists.
    const stranger = as("athlete-2", ATHLETE_APP);
    const attempts = [
      () => stranger.athlete(id),
      () => stranger.exportPackage(id),
      () => stranger.listRecords(id),
      () => stranger.changes(id, 0),
    ];
    for (const attempt of attempts) {
      expect((await failure(attempt())).code).toBe("not_found");
    }
    const unsigned = connectHost({ url: server.url, token: "not-a-token" });
    expect((await failure(unsigned.me())).code).toBe("sign_in_required");
  });

  it("lets an athlete disconnect one of two apps, and connect it again", async () => {
    const phone = as("athlete-5", ATHLETE_APP);
    const watch = as("athlete-5", "training-app");
    const { athlete_id: id } = await phone.createAthlete();
    await watch.createRecord(id, { record_type: "athlete_report", data: report("From the watch.") });
    const { apps } = await phone.listApps(id);
    expect(apps.map((app) => [app.client_id, app.state, app.this_app])).toEqual([
      [ATHLETE_APP, "connected", true],
      ["training-app", "connected", false],
    ]);

    const { app } = await phone.disconnectApp(id, "training-app");
    expect(app.state).toBe("disconnected");
    const refused = await failure(watch.listRecords(id));
    expect([refused.status, refused.code]).toEqual([403, "app_disconnected"]);
    expect((await watch.listAthletes()).athletes).toEqual([]);
    // The only connected app cannot be disconnected.
    expect((await failure(phone.disconnectApp(id, ATHLETE_APP))).code).toBe("refused");

    await phone.connectApp(id, "training-app");
    expect((await watch.listRecords(id, { record_type: "athlete_report" })).records).toHaveLength(1);
  });

  it("lets a coach see and add only what the athlete granted, until it is revoked", async () => {
    const owner = as("athlete-3", ATHLETE_APP);
    const { athlete_id: id } = await owner.createAthlete({ display_name: "Coached athlete" });
    const source = await owner.createRecord(id, { record_type: "source", data: manualSource });
    const sourceId = source.records[0]!.record_id;
    await owner.createRecord(id, { record_type: "workout_session", data: workout(sourceId, "2026-11-18") });
    await owner.createRecord(id, { record_type: "workout_session", data: workout(sourceId, "2026-11-20", "running") });

    const offer = await owner.createGrantOffer(id, {
      permissions: ["read_workout_summaries", "add_coach_notes", "add_programming"],
      client_id: COACH_APP,
      coach_label: "Coach Sam",
    });

    // The offer names the coach app; the same person in another app cannot accept it.
    const elsewhere = as("coach-1", ATHLETE_APP);
    expect((await failure(elsewhere.acceptGrantOffer(offer.code))).code).toBe("not_found");
    const coach = as("coach-1", COACH_APP);
    const { grant } = await coach.acceptGrantOffer(offer.code);
    expect(grant.permissions).toEqual(["read_workout_summaries", "add_coach_notes", "add_programming"]);
    expect((await failure(coach.acceptGrantOffer(offer.code))).code).toBe("not_found");

    const roster = await coach.roster();
    expect(roster.athletes.map((athlete) => athlete.athlete_id)).toEqual([id]);

    // Workout summaries, newest first, a page at a time, with nothing else from the workout.
    const first = await coach.workouts(id, { limit: 1 });
    expect(first.workouts.map((summary) => summary.activity_type)).toEqual(["running"]);
    expect(Object.keys(first.workouts[0]!).sort()).toEqual([
      "activity_type",
      "end_at",
      "record_id",
      "revision",
      "start_at",
    ]);
    const second = await coach.workouts(id, { limit: 1, after: first.next! });
    expect(second.workouts.map((summary) => summary.activity_type)).toEqual(["rowing"]);
    expect(second.next).toBeNull();

    const latest = first.workouts[0]!;
    const note = {
      note: "Keep the easy runs easy this week.",
      title: "Pacing",
      evidence: [{ record_id: latest.record_id, revision: latest.revision }],
    };
    const added = await coach.addNote(id, note, { idempotencyKey: "note-1" });
    expect(await coach.addNote(id, note, { idempotencyKey: "note-1" })).toEqual(added);

    // The note keeps its coach author, with the host's coach ID.
    const stored = await owner.getRecord(id, added.records[0]!.record_id);
    expect(stored.record.record_type).toBe("coaching_note");
    expect(stored.record.data).toMatchObject({
      note: note.note,
      authorship: { author: { author_type: "coach", coach: "Coach Sam", coach_id: (await coach.me()).coach_id } },
    });

    const program = await coach.addProgram(id, {
      plan: { title: "Base block", objective: "Build an aerobic base" },
      sessions: [
        { title: "Easy row", scheduled_date: "2026-12-01", blocks: [{ block_type: "main", title: "30 minutes easy" }] },
        { title: "Intervals", scheduled_date: "2026-12-03", blocks: [{ block_type: "main", title: "6 x 500 m" }] },
      ],
    });
    expect(program.records).toHaveLength(3);

    // A coach cannot read the owner's records or export them.
    expect((await failure(coach.listRecords(id))).code).toBe("permission_denied");
    expect((await failure(coach.exportPackage(id))).code).toBe("permission_denied");

    // Revoking stops the next request.
    const { grants } = await owner.listGrants(id);
    expect(grants.map((entry) => entry.grant_id)).toEqual([grant.grant_id]);
    await owner.revokeGrant(id, grant.grant_id);
    const revoked = await failure(coach.workouts(id));
    expect([revoked.status, revoked.code]).toEqual([404, "not_found"]);
    expect((await failure(coach.addNote(id, { note: "Late." }))).code).toBe("not_found");
    expect((await coach.roster()).athletes).toEqual([]);
  });

  it("runs the starter app's coach steps, and a retry adds no second note", async () => {
    // The starter imports the built package, as an app would.
    const packageRoot = join(repositoryRoot, "packages", "npm");
    const tsc = createRequire(join(packageRoot, "package.json")).resolve("typescript/bin/tsc");
    await run(process.execPath, [tsc, "-p", "tsconfig.build.json"], { cwd: packageRoot });

    const owner = as("athlete-4", ATHLETE_APP);
    const { athlete_id: id } = await owner.createAthlete({ display_name: "Starter athlete" });
    const source = await owner.createRecord(id, { record_type: "source", data: manualSource });
    const sourceId = source.records[0]!.record_id;
    await owner.createRecord(id, { record_type: "workout_session", data: workout(sourceId, "2026-11-20") });
    const offer = await owner.createGrantOffer(id, {
      permissions: ["read_workout_summaries", "add_coach_notes"],
      client_id: COACH_APP,
    });

    const starter = join(repositoryRoot, "examples", "host-starter", "coach.mjs");
    const env = {
      ...process.env,
      ATH_HOST_URL: server.url,
      ATH_HOST_TOKEN: signIn.token("coach-2", COACH_APP),
      ATH_NOTE: "Keep the easy rows easy this week.",
      ATH_NOTE_KEY: "starter-note-1",
    };
    const first = await run(process.execPath, [starter], { env: { ...env, ATH_GRANT_CODE: offer.code } });
    expect(first.stdout).toContain(`Accepted the offer for athlete ${id}: read_workout_summaries, add_coach_notes.`);
    expect(first.stdout).toContain("2026-11-20T07:00:00.000Z to 2026-11-20T07:45:00.000Z: rowing");
    const added = /Added note (\S+), citing the workout that started 2026-11-20T07:00:00\.000Z\./.exec(first.stdout);
    expect(added, first.stdout).not.toBeNull();

    const again = await run(process.execPath, [starter], { env });
    expect(again.stdout).toContain(`Added note ${added![1]!}`);
    const notes = await owner.listRecords(id, { record_type: "coaching_note" });
    expect(notes.records.map((record) => record.record_id)).toEqual([added![1]]);
  }, 300_000);
});
