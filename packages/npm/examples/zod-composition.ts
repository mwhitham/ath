/**
 * The published schemas are genuine Zod objects. A caller composes them into
 * its own schemas, infers TypeScript types from them, and parses in process
 * with parse and safeParse. Nothing here starts a native process.
 *
 * Run: node --import tsx examples/zod-composition.ts
 */
import { z } from "zod";
import {
  AgentActor,
  CurrentRecord,
  EvidenceReference,
  PointMeasurementTypeSchema,
  RecordId,
  UtcTimestamp,
  type CurrentRecordT,
  type PointMeasurementType,
} from "athleticstandard";

// Composition: wrap, extend, pick, and combine the published schemas.
const AgentNote = z.object({
  written_by: AgentActor,
  about: z.array(EvidenceReference).min(1),
  written_at: UtcTimestamp,
  text: z.string().min(1),
});
const ReviewedReference = EvidenceReference.extend({ reviewed: z.boolean() });
const RecordIdOnly = EvidenceReference.pick({ record_id: true });
const Batch = z.object({ records: z.array(CurrentRecord).max(100) });
const Quantity = PointMeasurementTypeSchema.or(z.literal("custom"));

// Inference: types follow the composed schemas.
type AgentNoteT = z.infer<typeof AgentNote>;
type ReviewedReferenceT = z.infer<typeof ReviewedReference>;
const quantity: PointMeasurementType = "hrv_rmssd";

function describeRecord(record: CurrentRecordT): string {
  // The discriminated union narrows on record_type.
  if (record.record_type === "point_measurement") {
    return `${record.data.quantity} ${record.data.value.standard_value}${record.data.value.standard_unit}`;
  }
  return record.record_type;
}

const note: AgentNoteT = AgentNote.parse({
  written_by: { author_type: "agent", agent: "example-agent", agent_version: "1.0.0" },
  about: [{ record_id: "01890f3e-e7d3-7cc0-88c8-000000000064", revision: 1 }],
  written_at: "2026-01-02T03:04:05.678Z",
  text: "RMSSD is steady this week.",
});
const reviewed: ReviewedReferenceT = ReviewedReference.parse({ ...note.about[0], reviewed: true });

const measurement = CurrentRecord.parse({
  record_id: "01890f3e-e7d3-7cc0-88c8-000000000064",
  record_type: "point_measurement",
  created_at: "2026-01-02T03:04:05.678Z",
  current_revision: 1,
  is_deleted: false,
  data: {
    source_id: "01890f3e-e7d3-7cc0-88c8-000000000003",
    received_at: "2026-01-02T03:04:05.678Z",
    received_at_basis: "import_time",
    quantity,
    input_timestamp: {
      timestamp_missing_reason: "not_provided_by_source",
      utc_offset_missing_reason: "not_provided_by_source",
    },
    value: {
      standard_value: 42,
      standard_unit: "ms",
      input_value_missing_reason: "not_provided_by_source",
      input_unit_missing_reason: "not_provided_by_source",
    },
  },
});

// The two-tier wall: a measured record without a source is refused.
const sourceless = Batch.safeParse({
  records: [{ ...measurement, data: { ...measurement.data, source_id: undefined } }],
});
const badId = RecordId.safeParse("01890F3E-E7D3-7CC0-88C8-000000000064");
let thrown = "";
try {
  UtcTimestamp.parse("2026-01-02");
} catch (error) {
  thrown = error instanceof z.ZodError ? error.issues[0]!.code : "not a ZodError";
}

console.log(
  JSON.stringify({
    note: note.text,
    reviewed: reviewed.reviewed,
    // Picked schemas stay strict, so only the picked key is accepted.
    record_id_only: RecordIdOnly.parse({ record_id: reviewed.record_id }).record_id === reviewed.record_id,
    picked_is_strict: !RecordIdOnly.safeParse(reviewed).success,
    describe: describeRecord(measurement),
    quantity_custom: Quantity.parse("custom"),
    sourceless_refused: !sourceless.success,
    sourceless_path: sourceless.success ? null : sourceless.error.issues[0]!.path.join("."),
    uppercase_id_refused: !badId.success,
    thrown,
  }),
);
