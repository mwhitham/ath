import {
  EvidenceReference,
  RecordErasureMarker,
  type EvidenceReferenceT,
  type RecordErasureMarkerT,
  type RecordRevisionT,
} from "../v1/schema.js";
import { databaseClient, type AthleticDatabase } from "./client.js";
import { readRecordRevisions } from "./records.js";

export type EvidenceReferenceResolution =
  | {
      status: "available";
      reference: EvidenceReferenceT;
      revision: RecordRevisionT;
    }
  | {
      status: "erased";
      reference: EvidenceReferenceT;
      erasure_marker: RecordErasureMarkerT;
    }
  | {
      status: "missing";
      reference: EvidenceReferenceT;
    };

export async function resolveEvidenceReference(
  database: AthleticDatabase,
  input: EvidenceReferenceT,
): Promise<EvidenceReferenceResolution> {
  const reference = EvidenceReference.parse(input);
  const erased = await databaseClient(database).execute({
    sql: `
      SELECT record_id, erased_at, reason_code
      FROM record_erasure_markers
      WHERE record_id = ?
    `,
    args: [reference.record_id],
  });
  if (erased.rows[0] !== undefined) {
    return {
      status: "erased",
      reference,
      erasure_marker: RecordErasureMarker.parse({
        record_id: erased.rows[0].record_id,
        erased_at: erased.rows[0].erased_at,
        reason_code: erased.rows[0].reason_code,
      }),
    };
  }
  const revision = (await readRecordRevisions(
    database,
    reference.record_id,
  )).find((candidate) => candidate.revision === reference.revision);
  if (revision === undefined) {
    return { status: "missing", reference };
  }
  return { status: "available", reference, revision };
}
