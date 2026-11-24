/**
 * A connection to an Athletic Standard host (`ath-host`), API version 1 (D266).
 *
 * The caller names the host's address and gives an OAuth access token for the
 * signed-in person. The host checks the token and that person's grants on
 * every request. This client never receives database credentials. It is
 * separate from the local client: database URLs given to `openDatabase` are
 * still refused, as REMOTE-TARGETS.md describes.
 *
 * Every write sends an `Idempotency-Key` header. When the caller gives no key,
 * the client makes a random one. A retry of the same write must send the same
 * key with the same body; the host then returns the first result and writes
 * nothing new. A thrown `HostError` from a write carries the key it used.
 */
import type { CurrentRecordT, RecordRevisionT, RecordTypeT } from "./schema.js";

/** The three permissions an athlete can grant a coach (D262). */
export type HostPermission = "read_workout_summaries" | "add_coach_notes" | "add_programming";

/** The stable error codes of host API version 1. */
export type HostErrorCode =
  | "sign_in_required"
  | "permission_denied"
  | "app_disconnected"
  | "not_found"
  | "stale"
  | "request_conflict"
  | "host_moving"
  | "limit_reached"
  | "invalid"
  | "refused"
  | "internal";

/**
 * An error from the host, or from reaching it.
 *
 * `status` is the HTTP status. `code` is one of the host's stable codes, or
 * `unreachable` (status 0) when no reply arrived, or `unexpected_response`
 * when the reply was not a host API error.
 */
export class HostError extends Error {
  readonly status: number;
  readonly code: HostErrorCode | "unreachable" | "unexpected_response";
  /** The key a write sent. Retry the same write with this key. */
  readonly idempotencyKey: string | undefined;

  constructor(
    status: number,
    code: HostErrorCode | "unreachable" | "unexpected_response",
    message: string,
    options: { idempotencyKey?: string; cause?: unknown } = {},
  ) {
    super(message, options.cause === undefined ? undefined : { cause: options.cause });
    this.name = "HostError";
    this.status = status;
    this.code = code;
    this.idempotencyKey = options.idempotencyKey;
  }
}

export interface HostConnectionOptions {
  /** The host's address, such as `https://records.example.org`. */
  url: string | URL;
  /**
   * The signed-in person's access token, or a function that returns a current
   * one. The function runs before every request, so it can refresh the token.
   */
  token: string | (() => string | Promise<string>);
  /** The fetch function to use. The default is the global `fetch`. */
  fetch?: typeof fetch;
}

/** Options for every write. */
export interface HostWriteOptions {
  /** 1 to 200 characters. Reuse it to retry the same write safely. */
  idempotencyKey?: string;
}

export interface HostCapabilities {
  api_version: number;
  athleticstandard_version: string;
  storage: "directory" | "libsql";
  permissions: HostPermission[];
  /** Who runs the host and its storage (D264). Show it before connecting. */
  operator_disclosure: string;
  open_enrollment: boolean;
}

export interface HostMe {
  principal_id: string;
  /** The app the token was issued to. Grants name this app. */
  client_id: string;
  /** The opaque ID the host writes into this person's coach notes (D255). */
  coach_id: string;
  /** The athletes this person owns. */
  athletes: string[];
}

export interface HostAthlete {
  athlete_id: string;
  display_name: string | null;
  state: string;
}

export interface HostAthleteSummary {
  athlete_id: string;
  role: "owner" | "coach";
  state: string;
  operator_disclosure: string;
  /** Present for the owner only. */
  database_revision?: number;
}

export interface HostCreatedAthlete {
  athlete_id: string;
  database_revision: number;
  operator_disclosure: string;
}

/** The result of a write: the new database revision and each record written. */
export interface HostWriteResult {
  database_revision: number;
  records: Array<{ record_id: string; revision: number }>;
}

export interface HostExport {
  /** The complete package as a zip. */
  bytes: Uint8Array;
  /** The SHA-256 of the package manifest, in hexadecimal (D263). */
  manifestDigest: string;
}

export interface HostChange {
  database_revision: number;
  record_id: string;
  record_revision: number | null;
  kind: string;
}

export interface HostChanges {
  from: number;
  to: number;
  /** False when the log no longer reaches back to `from`. Download a full export then. */
  complete: boolean;
  changes: HostChange[];
}

export interface HostNewRecord {
  record_type: RecordTypeT;
  data: Record<string, unknown>;
  record_id?: string;
  /** Upload IDs from `upload`. The host links them to this record. */
  attachments?: string[];
}

export interface HostUpload {
  upload_id: string;
  byte_count: number;
  sha256: string;
  media_type: string;
}

export interface HostAttachment {
  bytes: Uint8Array;
  mediaType: string | null;
}

export interface HostGrantOfferRequest {
  permissions: HostPermission[];
  /** The app the coach must use to accept the offer. */
  client_id: string;
  coach_label?: string;
  /** How long the code works, from 60 seconds to 30 days. The default is 7 days. */
  valid_seconds?: number;
}

export interface HostGrantOffer {
  /** Give this code to the coach. It works once. */
  code: string;
  expires_at: string;
  permissions: HostPermission[];
  client_id: string;
}

export interface HostGrant {
  grant_id: string;
  resource_id: string;
  coach_principal_id: string;
  client_id: string;
  permissions: HostPermission[];
  coach_label: string | null;
  created_at: string;
  revoked_at: string | null;
}

/** An app the athlete has used with one record (D270). */
export interface HostApp {
  client_id: string;
  state: "connected" | "disconnected";
  connected_at: string;
  last_used_at: string;
  disconnected_at: string | null;
  /** True for the app the request came from. */
  this_app: boolean;
}

export interface HostRosterEntry {
  athlete_id: string;
  display_name: string | null;
  permissions: HostPermission[];
  grant_id: string;
}

/** A workout as the read grant shows it. Nothing else from the workout is sent. */
export interface HostWorkoutSummary {
  record_id: string;
  revision: number;
  start_at: string;
  end_at: string;
  activity_type: string | null;
}

export interface HostWorkoutPage {
  workouts: HostWorkoutSummary[];
  /** Pass this as `after` to read the next page. Null on the last page. */
  next: string | null;
}

export interface HostNoteRequest {
  note: string;
  title?: string;
  applies_from?: string;
  applies_to?: string;
  /** Current workout revisions the coach can read. */
  evidence?: Array<{ record_id: string; revision: number }>;
}

export interface HostProgramRequest {
  /** A training plan without `authorship`. The host writes the coach as author. */
  plan: Record<string, unknown>;
  /** Planned sessions. The host links each to the plan and marks it planned. */
  sessions?: Array<Record<string, unknown>>;
}

/** One method for each operation of host API version 1. */
export interface HostConnection {
  /** The host's address, without a trailing slash. */
  readonly url: string;

  capabilities(): Promise<HostCapabilities>;
  me(): Promise<HostMe>;

  // The owner's record.
  createAthlete(
    request?: { display_name?: string; invitation?: string },
    options?: HostWriteOptions,
  ): Promise<HostCreatedAthlete>;
  listAthletes(): Promise<{ athletes: HostAthlete[] }>;
  athlete(athleteId: string): Promise<HostAthleteSummary>;
  /** Restore a complete package, as a zip, into the athlete's empty record. */
  putPackage(athleteId: string, zip: Uint8Array, options?: HostWriteOptions): Promise<{ database_revision: number }>;
  exportPackage(athleteId: string): Promise<HostExport>;
  changes(athleteId: string, since: number): Promise<HostChanges>;
  listRecords(
    athleteId: string,
    options?: { record_type?: RecordTypeT; limit?: number },
  ): Promise<{ records: CurrentRecordT[] }>;
  getRecord(athleteId: string, recordId: string): Promise<{ record: CurrentRecordT; revisions: RecordRevisionT[] }>;
  createRecord(athleteId: string, record: HostNewRecord, options?: HostWriteOptions): Promise<HostWriteResult>;
  correctRecord(
    athleteId: string,
    recordId: string,
    correction: { expected_revision: number; data: Record<string, unknown> },
    options?: HostWriteOptions,
  ): Promise<HostWriteResult>;
  deleteRecord(
    athleteId: string,
    recordId: string,
    deletion: { expected_revision: number },
    options?: HostWriteOptions,
  ): Promise<HostWriteResult>;
  /** Remove a record and every record that depends on it, permanently. */
  eraseRecord(
    athleteId: string,
    recordId: string,
    erasure: { expected_revision: number; reason_code: string },
    options?: HostWriteOptions,
  ): Promise<{ database_revision: number }>;
  /** Stage a PNG, JPEG, WebP, or PDF file for `createRecord`'s `attachments`. */
  upload(athleteId: string, bytes: Uint8Array, options?: HostWriteOptions): Promise<HostUpload>;
  downloadAttachment(athleteId: string, attachmentRecordId: string): Promise<HostAttachment>;

  // The owner's grants (D262).
  createGrantOffer(
    athleteId: string,
    offer: HostGrantOfferRequest,
    options?: HostWriteOptions,
  ): Promise<HostGrantOffer>;
  listGrants(athleteId: string): Promise<{ grants: HostGrant[] }>;
  revokeGrant(
    athleteId: string,
    grantId: string,
    options?: HostWriteOptions,
  ): Promise<{ revoked: string; access_epoch: number }>;

  // The owner's apps (D270). An app connects on its first use with a record.
  listApps(athleteId: string): Promise<{ apps: HostApp[] }>;
  /**
   * Disconnect an app from the record. Its next request is refused with
   * `app_disconnected`. Another app must stay connected.
   */
  disconnectApp(
    athleteId: string,
    clientId: string,
    options?: HostWriteOptions,
  ): Promise<{ app: HostApp; access_epoch: number }>;
  /** Connect a disconnected app again. Call this from an app that is still connected. */
  connectApp(athleteId: string, clientId: string, options?: HostWriteOptions): Promise<{ app: HostApp }>;

  // A coach.
  /** Accept an athlete's offer. The token must be issued to the app the offer names. */
  acceptGrantOffer(code: string, options?: HostWriteOptions): Promise<{ grant: HostGrant }>;
  roster(): Promise<{ athletes: HostRosterEntry[] }>;
  workouts(athleteId: string, options?: { limit?: number; after?: string }): Promise<HostWorkoutPage>;
  addNote(athleteId: string, note: HostNoteRequest, options?: HostWriteOptions): Promise<HostWriteResult>;
  addProgram(athleteId: string, program: HostProgramRequest, options?: HostWriteOptions): Promise<HostWriteResult>;
}

type CallBody = { json: unknown } | { bytes: Uint8Array; contentType: string } | undefined;

interface HostCall {
  method: "GET" | "POST" | "PUT" | "DELETE";
  path: string;
  query?: Record<string, string | number | undefined>;
  body?: CallBody;
  /** Present for a write: the key to send, or undefined to make one. */
  write?: HostWriteOptions;
}

/**
 * Connect to a host. Nothing is sent until the first call.
 *
 * ```ts
 * const host = connectHost({ url: "https://records.example.org", token });
 * const { athletes } = await host.roster();
 * ```
 */
export function connectHost(options: HostConnectionOptions): HostConnection {
  const url = String(options.url).replace(/\/+$/, "");
  if (!/^https?:\/\//i.test(url)) {
    throw new TypeError(`A host address starts with https:// or http://, got ${url}`);
  }
  const send = options.fetch ?? globalThis.fetch;
  const token = options.token;

  async function call(request: HostCall): Promise<Response> {
    const target = new URL(url + request.path);
    for (const [name, value] of Object.entries(request.query ?? {})) {
      if (value !== undefined) target.searchParams.set(name, String(value));
    }
    const key = request.write === undefined ? undefined : (request.write.idempotencyKey ?? newKey());
    const headers: Record<string, string> = {
      authorization: `Bearer ${typeof token === "function" ? await token() : token}`,
      accept: "application/json",
    };
    if (key !== undefined) headers["idempotency-key"] = key;
    let body: string | Uint8Array | undefined;
    if (request.body !== undefined && "json" in request.body) {
      headers["content-type"] = "application/json";
      body = JSON.stringify(request.body.json);
    } else if (request.body !== undefined) {
      headers["content-type"] = request.body.contentType;
      body = request.body.bytes;
    }
    const keyOption = key === undefined ? {} : { idempotencyKey: key };
    let response: Response;
    try {
      response = await send(target, { method: request.method, headers, ...(body === undefined ? {} : { body }) });
    } catch (cause) {
      throw new HostError(0, "unreachable", `No reply from ${url}: ${String(cause)}`, { ...keyOption, cause });
    }
    if (!response.ok) {
      throw await errorFrom(response, keyOption);
    }
    return response;
  }

  async function json<T>(request: HostCall): Promise<T> {
    return (await (await call(request)).json()) as T;
  }

  const athletePath = (athleteId: string, rest = "") => `/v1/athletes/${encodeURIComponent(athleteId)}${rest}`;
  const recordPath = (athleteId: string, recordId: string, rest = "") =>
    athletePath(athleteId, `/records/${encodeURIComponent(recordId)}${rest}`);

  return {
    url,
    capabilities: () => json({ method: "GET", path: "/v1/capabilities" }),
    me: () => json({ method: "GET", path: "/v1/me" }),

    createAthlete: (request = {}, options = {}) =>
      json({ method: "POST", path: "/v1/athletes", body: { json: request }, write: options }),
    listAthletes: () => json({ method: "GET", path: "/v1/athletes" }),
    athlete: (athleteId) => json({ method: "GET", path: athletePath(athleteId) }),
    putPackage: (athleteId, zip, options = {}) =>
      json({
        method: "PUT",
        path: athletePath(athleteId, "/package"),
        body: { bytes: zip, contentType: "application/zip" },
        write: options,
      }),
    exportPackage: async (athleteId) => {
      const response = await call({ method: "GET", path: athletePath(athleteId, "/export") });
      const manifestDigest = response.headers.get("x-manifest-digest");
      if (manifestDigest === null) {
        throw new HostError(response.status, "unexpected_response", "The export has no x-manifest-digest header");
      }
      return { bytes: new Uint8Array(await response.arrayBuffer()), manifestDigest };
    },
    changes: (athleteId, since) =>
      json({ method: "GET", path: athletePath(athleteId, "/changes"), query: { since } }),
    listRecords: (athleteId, options = {}) =>
      json({
        method: "GET",
        path: athletePath(athleteId, "/records"),
        query: { record_type: options.record_type, limit: options.limit },
      }),
    getRecord: (athleteId, recordId) => json({ method: "GET", path: recordPath(athleteId, recordId) }),
    createRecord: (athleteId, record, options = {}) =>
      json({ method: "POST", path: athletePath(athleteId, "/records"), body: { json: record }, write: options }),
    correctRecord: (athleteId, recordId, correction, options = {}) =>
      json({
        method: "POST",
        path: recordPath(athleteId, recordId, "/corrections"),
        body: { json: correction },
        write: options,
      }),
    deleteRecord: (athleteId, recordId, deletion, options = {}) =>
      json({
        method: "POST",
        path: recordPath(athleteId, recordId, "/deletion"),
        body: { json: deletion },
        write: options,
      }),
    eraseRecord: (athleteId, recordId, erasure, options = {}) =>
      json({
        method: "POST",
        path: recordPath(athleteId, recordId, "/erasure"),
        body: { json: erasure },
        write: options,
      }),
    upload: (athleteId, bytes, options = {}) =>
      json({
        method: "POST",
        path: athletePath(athleteId, "/uploads"),
        body: { bytes, contentType: "application/octet-stream" },
        write: options,
      }),
    downloadAttachment: async (athleteId, attachmentRecordId) => {
      const response = await call({
        method: "GET",
        path: athletePath(athleteId, `/attachments/${encodeURIComponent(attachmentRecordId)}`),
      });
      return { bytes: new Uint8Array(await response.arrayBuffer()), mediaType: response.headers.get("content-type") };
    },

    createGrantOffer: (athleteId, offer, options = {}) =>
      json({ method: "POST", path: athletePath(athleteId, "/grant-offers"), body: { json: offer }, write: options }),
    listGrants: (athleteId) => json({ method: "GET", path: athletePath(athleteId, "/grants") }),
    revokeGrant: (athleteId, grantId, options = {}) =>
      json({
        method: "DELETE",
        path: athletePath(athleteId, `/grants/${encodeURIComponent(grantId)}`),
        write: options,
      }),

    listApps: (athleteId) => json({ method: "GET", path: athletePath(athleteId, "/apps") }),
    disconnectApp: (athleteId, clientId, options = {}) =>
      json({ method: "DELETE", path: athletePath(athleteId, `/apps/${encodeURIComponent(clientId)}`), write: options }),
    connectApp: (athleteId, clientId, options = {}) =>
      json({ method: "PUT", path: athletePath(athleteId, `/apps/${encodeURIComponent(clientId)}`), write: options }),

    acceptGrantOffer: (code, options = {}) =>
      json({ method: "POST", path: "/v1/grant-offers/accept", body: { json: { code } }, write: options }),
    roster: () => json({ method: "GET", path: "/v1/roster" }),
    workouts: (athleteId, options = {}) =>
      json({
        method: "GET",
        path: athletePath(athleteId, "/workouts"),
        query: { limit: options.limit, after: options.after },
      }),
    addNote: (athleteId, note, options = {}) =>
      json({ method: "POST", path: athletePath(athleteId, "/notes"), body: { json: note }, write: options }),
    addProgram: (athleteId, program, options = {}) =>
      json({ method: "POST", path: athletePath(athleteId, "/programs"), body: { json: program }, write: options }),
  };
}

function newKey(): string {
  return globalThis.crypto.randomUUID();
}

const CODES: ReadonlySet<string> = new Set<HostErrorCode>([
  "sign_in_required",
  "permission_denied",
  "app_disconnected",
  "not_found",
  "stale",
  "request_conflict",
  "host_moving",
  "limit_reached",
  "invalid",
  "refused",
  "internal",
]);

/** The HostError for a reply that is not a success. */
async function errorFrom(response: Response, options: { idempotencyKey?: string }): Promise<HostError> {
  const text = await response.text().catch(() => "");
  try {
    const parsed = JSON.parse(text) as { error?: { code?: unknown; message?: unknown } };
    const code = parsed.error?.code;
    const message = parsed.error?.message;
    if (typeof code === "string" && CODES.has(code) && typeof message === "string") {
      return new HostError(response.status, code as HostErrorCode, message, options);
    }
  } catch {
    // Not JSON: reported below.
  }
  const detail = text.trim().slice(0, 200);
  return new HostError(
    response.status,
    "unexpected_response",
    `The host answered ${response.status}${detail === "" ? "" : `: ${detail}`}`,
    options,
  );
}
