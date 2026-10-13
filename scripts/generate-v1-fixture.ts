/**
 * Generate the deterministic v1 SQLite demo athlete.
 *
 * The fixture uses fixed IDs and times. Regenerate it; do not edit the database.
 */
import { createHash } from "node:crypto";
import { chmod, mkdir, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  correctRecord,
  createDatabase,
  deleteRecord,
  writeRecords,
} from "../src/db/index.js";
import { databaseClient } from "../src/db/client.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const fixtureDirectory = join(root, "examples", "demo-athlete");
const databasePath = join(fixtureDirectory, "athlete.ath.sqlite");
const attachmentDirectory = join(fixtureDirectory, "attachments");
const attachmentName = "meal-photo.png";

const createdAt = "2026-08-09T12:00:00.000Z";
const toolActor = {
  author_type: "tool" as const,
  tool: "ath",
  tool_version: "1.0.0",
};

function id(sequence: number): string {
  return `01890f3e-e7d3-7cc0-88c8-${sequence.toString(16).padStart(12, "0")}`;
}

const ids = {
  database: id(1),
  profile: id(2),
  wearableSource: id(3),
  manualSource: id(4),
  sourceDevice: id(5),
  benchmark: id(6),
  point: id(7),
  sample: id(8),
  sleep: id(9),
  workout: id(10),
  result: id(11),
  providerScore: id(12),
  athleteReport: id(13),
  attachment: id(14),
  nutrition: id(15),
  prediction: id(16),
  resultLink: id(17),
  grade: id(18),
  errorAnalysis: id(19),
  plan: id(20),
  plannedSession: id(21),
  coachingNote: id(22),
  review: id(23),
  goal: id(24),
  nutritionInterpretation: id(25),
} as const;
const attachmentPath = join(attachmentDirectory, `${ids.attachment}.png`);

const inputTimestamp = {
  timestamp_text: "2026-08-09T06:12:00-05:00",
  utc_offset_text: "-05:00",
  utc_offset_meaning: "provider_cycle_offset" as const,
  timestamp_basis: "source_offset_timestamp" as const,
};

const athleteAuthorship = {
  author: { author_type: "athlete" as const },
  authored_at: createdAt,
  evidence: [],
  content_version: 1,
  status: "active" as const,
};

const agentAuthorship = {
  author: {
    author_type: "agent" as const,
    agent: "ath-reference-agent",
    agent_version: "1.0.0",
    model: "fixture-model",
    model_version: "1",
  },
  authored_at: "2026-08-09T15:00:00.000Z",
  evidence: [
    { record_id: ids.point, revision: 1 },
    { record_id: ids.sleep, revision: 1 },
  ],
  content_version: 1,
  status: "active" as const,
};

const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Z9xkAAAAASUVORK5CYII=",
  "base64",
);

async function generate(): Promise<void> {
  await rm(attachmentDirectory, { recursive: true, force: true });
  await mkdir(attachmentDirectory, { recursive: true, mode: 0o700 });
  await Promise.all(
    [databasePath, `${databasePath}-wal`, `${databasePath}-shm`].map((path) =>
      rm(path, { force: true }),
    ),
  );
  await writeFile(attachmentPath, png, { mode: 0o600 });
  await chmod(attachmentDirectory, 0o700);

  const attachmentSha256 = createHash("sha256").update(png).digest("hex");
  const database = await createDatabase(databasePath, {
    databaseId: ids.database,
    createdAt,
  });

  try {
    await writeRecords(database, [
      {
        record_type: "athlete_profile",
        record_id: ids.profile,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          name: "Demo Athlete",
          birth_year: 1990,
          sex: "female",
          display_units: "metric",
          default_time_zone: "America/Chicago",
        },
      },
      {
        record_type: "source",
        record_id: ids.wearableSource,
        created_at: createdAt,
        became_known_at_basis: "provider_import",
        changed_by: toolActor,
        data: {
          source_type: "wearable",
          data_writer: "WHOOP",
          data_route: "whoop_csv",
          vendor: "whoop",
          sensor_type: "optical",
        },
      },
      {
        record_type: "source",
        record_id: ids.manualSource,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          source_type: "manual",
          data_writer: "ath",
          data_route: "manual_entry",
        },
      },
      {
        record_type: "source_device",
        record_id: ids.sourceDevice,
        created_at: createdAt,
        became_known_at_basis: "provider_import",
        changed_by: toolActor,
        data: {
          source_id: ids.wearableSource,
          name: "WHOOP",
          model: "4.0",
          coverage_start_date: "2026-08-08",
          coverage_end_date: "2026-08-10",
          accepted_record_count: 5,
        },
      },
      {
        record_type: "benchmark_definition",
        record_id: ids.benchmark,
        created_at: createdAt,
        changed_by: toolActor,
        data: {
          authorship: athleteAuthorship,
          benchmark_key: "fran",
          benchmark_type: "named_wod",
          score_type: "time",
          definition: "21-15-9 thrusters and pull-ups for time",
          tags: ["short", "high-power"],
        },
      },
      {
        record_type: "point_measurement",
        record_id: ids.point,
        created_at: createdAt,
        became_known_at: createdAt,
        became_known_at_basis: "provider_import",
        changed_by: toolActor,
        data: {
          source_id: ids.wearableSource,
          provider_record_id: "whoop-recovery-2026-08-09",
          received_at: createdAt,
          received_at_basis: "import_time",
          quantity: "hrv_rmssd",
          recorded_at: "2026-08-09T11:12:00.000Z",
          input_timestamp: inputTimestamp,
          value: {
            standard_value: 62,
            standard_unit: "ms",
            input_value: 62,
            input_unit: "ms",
          },
        },
      },
      {
        record_type: "measurement_sample",
        record_id: ids.sample,
        created_at: createdAt,
        became_known_at_basis: "provider_import",
        changed_by: toolActor,
        data: {
          source_id: ids.wearableSource,
          provider_record_id: "whoop-hr-2026-08-09-001",
          received_at: createdAt,
          received_at_basis: "import_time",
          quantity: "heart_rate",
          recorded_at: "2026-08-09T11:12:00.000Z",
          input_timestamp: inputTimestamp,
          value: {
            standard_value: 142,
            standard_unit: "bpm",
            input_value: 142,
            input_unit: "count/min",
          },
        },
      },
      {
        record_type: "sleep_session",
        record_id: ids.sleep,
        created_at: createdAt,
        became_known_at_basis: "provider_import",
        changed_by: toolActor,
        data: {
          source_id: ids.wearableSource,
          received_at: createdAt,
          received_at_basis: "import_time",
          start_at: "2026-08-09T03:30:00.000Z",
          end_at: "2026-08-09T11:00:00.000Z",
          input_start_timestamp: {
            ...inputTimestamp,
            timestamp_text: "2026-08-08T22:30:00-05:00",
          },
          input_end_timestamp: {
            ...inputTimestamp,
            timestamp_text: "2026-08-09T06:00:00-05:00",
          },
          source_reported_summaries: [
            {
              summary_type: "sleep_duration",
              standard_value: 26_100,
              standard_unit: "s",
              input_value: 435,
              input_unit: "min",
            },
          ],
          provider_reporting_periods: [
            {
              period_type: "whoop_physiological_cycle",
              provider_period_id: "cycle-2026-08-09",
              start_at: "2026-08-08T11:00:00.000Z",
              end_at: "2026-08-09T11:00:00.000Z",
              timezone_offset_text: "-05:00",
              timezone_offset_meaning: "provider_cycle_offset",
            },
          ],
        },
      },
      {
        record_type: "workout_session",
        record_id: ids.workout,
        created_at: "2026-08-10T18:00:00.000Z",
        became_known_at_basis: "provider_import",
        changed_by: toolActor,
        data: {
          source_id: ids.wearableSource,
          received_at: "2026-08-10T18:00:00.000Z",
          received_at_basis: "import_time",
          start_at: "2026-08-10T17:25:00.000Z",
          end_at: "2026-08-10T17:48:00.000Z",
          input_start_timestamp: {
            ...inputTimestamp,
            timestamp_text: "2026-08-10T12:25:00-05:00",
          },
          input_end_timestamp: {
            ...inputTimestamp,
            timestamp_text: "2026-08-10T12:48:00-05:00",
          },
          activity_type: "crossfit",
          source_reported_summaries: [
            {
              summary_type: "average_heart_rate",
              standard_value: 168,
              standard_unit: "bpm",
              input_value: 168,
              input_unit: "bpm",
            },
          ],
          segments: [
            {
              label: "Fran",
              source_reported_summaries: [
                {
                  summary_type: "duration",
                  standard_value: 300,
                  standard_unit: "s",
                  input_value: 300,
                  input_unit: "s",
                },
              ],
            },
          ],
        },
      },
      {
        record_type: "benchmark_result",
        record_id: ids.result,
        created_at: "2026-08-10T18:00:00.000Z",
        changed_by: toolActor,
        data: {
          source_id: ids.manualSource,
          received_at: "2026-08-10T18:00:00.000Z",
          received_at_basis: "local_write_time",
          benchmark_definition_id: ids.benchmark,
          recorded_at: "2026-08-10T17:48:00.000Z",
          input_timestamp: {
            timestamp_text: "2026-08-10T17:48:00.000Z",
            utc_offset_text: "Z",
            utc_offset_meaning: "documented_recording_offset",
            timestamp_basis: "source_instant",
          },
          score: { duration_s: 300 },
          scaling: "rx",
          workout_session_id: ids.workout,
        },
      },
      {
        record_type: "provider_score",
        record_id: ids.providerScore,
        created_at: createdAt,
        became_known_at_basis: "provider_import",
        changed_by: toolActor,
        data: {
          source_id: ids.wearableSource,
          provider_record_id: "whoop-recovery-score-2026-08-09",
          received_at: createdAt,
          received_at_basis: "import_time",
          metric: "recovery",
          value: {
            standard_value: 78,
            standard_unit: "score",
            input_value: 78,
            input_unit: "score",
          },
          scale: "0-100",
          input_timestamp: {
            timestamp_missing_reason: "not_provided_by_source",
            utc_offset_missing_reason: "not_provided_by_source",
          },
          provider_reporting_periods: [
            {
              period_type: "whoop_physiological_cycle",
              provider_period_id: "cycle-2026-08-09",
            },
          ],
        },
      },
      {
        record_type: "attachment",
        record_id: ids.attachment,
        created_at: "2026-08-09T18:00:00.000Z",
        changed_by: toolActor,
        data: {
          file_name: attachmentName,
          media_type: "image/png",
          byte_count: png.byteLength,
          sha256: attachmentSha256,
          description: "Meal photo",
        },
      },
      {
        record_type: "athlete_report",
        record_id: ids.athleteReport,
        created_at: "2026-08-09T13:00:00.000Z",
        changed_by: toolActor,
        data: {
          report_type: "sleep_quality",
          reported_at: "2026-08-09T13:00:00.000Z",
          input_reported_at: {
            timestamp_text: "2026-08-09T13:00:00.000Z",
            utc_offset_text: "Z",
            utc_offset_meaning: "documented_recording_offset",
            timestamp_basis: "source_instant",
          },
          reported_via: "text",
          rating: 2,
          rating_scale: "1-5",
          note: "Poor sleep before training",
        },
      },
      {
        record_type: "nutrition_entry",
        record_id: ids.nutrition,
        created_at: "2026-08-09T18:00:00.000Z",
        changed_by: toolActor,
        data: {
          reported_at: "2026-08-09T18:00:00.000Z",
          input_reported_at: {
            timestamp_text: "2026-08-09T18:00:00.000Z",
            utc_offset_text: "Z",
            utc_offset_meaning: "documented_recording_offset",
            timestamp_basis: "source_instant",
          },
          reported_via: "photo",
          athlete_description: "Chicken, rice, and vegetables",
          reported_items: [
            {
              food_description: "chicken and rice bowl",
              amount: 1,
              amount_unit: "bowl",
            },
          ],
          attachment_ids: [ids.attachment],
        },
      },
      {
        record_type: "prediction",
        record_id: ids.prediction,
        created_at: "2026-08-09T15:00:00.000Z",
        changed_by: toolActor,
        data: {
          authorship: agentAuthorship,
          benchmark_definition_id: ids.benchmark,
          predicted_score: { duration_s: 280 },
          predicted_range: {
            low: { duration_s: 270 },
            high: { duration_s: 290 },
          },
          confidence: "moderate",
          reasoning: "HRV and sleep evidence supported a moderate prediction.",
        },
      },
      {
        record_type: "prediction_result_link",
        record_id: ids.resultLink,
        created_at: "2026-08-10T18:00:00.000Z",
        changed_by: toolActor,
        data: {
          prediction: { record_id: ids.prediction, revision: 1 },
          benchmark_result: { record_id: ids.result, revision: 1 },
          created_by: toolActor,
          linked_at: "2026-08-10T18:00:00.000Z",
          content_version: 1,
          status: "active",
        },
      },
      {
        record_type: "prediction_grade",
        record_id: ids.grade,
        created_at: "2026-08-10T18:00:00.000Z",
        changed_by: toolActor,
        data: {
          authorship: {
            author: {
              ...toolActor,
              calculation_version: "prediction-grade-v1",
            },
            authored_at: "2026-08-10T18:00:00.000Z",
            evidence: [{ record_id: ids.resultLink, revision: 1 }],
            content_version: 1,
            status: "active",
          },
          prediction_result_link: { record_id: ids.resultLink, revision: 1 },
          signed_error: -20,
          absolute_error_percentage: 6.7,
          result_in_predicted_range: false,
          predicted_score: { duration_s: 280 },
          actual_score: { duration_s: 300 },
        },
      },
      {
        record_type: "prediction_error_analysis",
        record_id: ids.errorAnalysis,
        created_at: "2026-08-10T19:00:00.000Z",
        changed_by: toolActor,
        data: {
          authorship: {
            ...agentAuthorship,
            authored_at: "2026-08-10T19:00:00.000Z",
            evidence: [
              { record_id: ids.grade, revision: 1 },
              { record_id: ids.athleteReport, revision: 1 },
            ],
          },
          prediction_grade: { record_id: ids.grade, revision: 1 },
          error_direction: "slower",
          error_severity: "significant",
          causes: [
            {
              evidence: { record_id: ids.athleteReport, revision: 1 },
              explanation: "The athlete reported poor sleep before the attempt.",
            },
          ],
          unexplained: false,
          lesson: "Include reported sleep quality in short high-power predictions.",
        },
      },
      {
        record_type: "training_plan",
        record_id: ids.plan,
        created_at: "2026-08-09T16:00:00.000Z",
        changed_by: toolActor,
        data: {
          authorship: {
            ...agentAuthorship,
            authored_at: "2026-08-09T16:00:00.000Z",
          },
          title: "Demo training week",
          objective: "Practice benchmark pacing",
          start_date: "2026-08-10",
          end_date: "2026-08-16",
        },
      },
      {
        record_type: "planned_session",
        record_id: ids.plannedSession,
        created_at: "2026-08-09T16:00:00.000Z",
        changed_by: toolActor,
        data: {
          authorship: {
            ...agentAuthorship,
            authored_at: "2026-08-09T16:00:00.000Z",
          },
          training_plan_id: ids.plan,
          title: "Fran pacing session",
          scheduled_date: "2026-08-10",
          activity_type: "crossfit",
          blocks: [
            {
              block_type: "benchmark",
              title: "Fran",
              instructions: "Complete at sustainable opening pace.",
            },
          ],
          completion_status: "completed",
          completed_workout_session_id: ids.workout,
        },
      },
      {
        record_type: "coaching_note",
        record_id: ids.coachingNote,
        created_at: "2026-08-10T19:00:00.000Z",
        changed_by: toolActor,
        data: {
          authorship: {
            ...agentAuthorship,
            authored_at: "2026-08-10T19:00:00.000Z",
          },
          title: "Pacing",
          note: "Use a steadier opening set next time.",
          applies_from: "2026-08-10",
        },
      },
      {
        record_type: "saved_training_review",
        record_id: ids.review,
        created_at: "2026-08-10T19:00:00.000Z",
        changed_by: toolActor,
        data: {
          authorship: {
            ...agentAuthorship,
            authored_at: "2026-08-10T19:00:00.000Z",
          },
          period_start: "2026-08-04",
          period_end: "2026-08-10",
          summary: "Training was consistent; the benchmark missed its predicted range.",
          recommendations: ["Use steadier benchmark pacing."],
        },
      },
      {
        record_type: "goal",
        record_id: ids.goal,
        created_at: "2026-08-10T19:00:00.000Z",
        changed_by: toolActor,
        data: {
          authorship: {
            ...agentAuthorship,
            authored_at: "2026-08-10T19:00:00.000Z",
          },
          title: "Fran under 4:30",
          target_date: "2026-12-31",
          target_benchmark_definition_id: ids.benchmark,
          target_score: { duration_s: 270 },
        },
      },
      {
        record_type: "nutrition_model_interpretation",
        record_id: ids.nutritionInterpretation,
        created_at: "2026-08-09T18:05:00.000Z",
        changed_by: toolActor,
        data: {
          authorship: {
            ...agentAuthorship,
            authored_at: "2026-08-09T18:05:00.000Z",
            evidence: [
              { record_id: ids.nutrition, revision: 1 },
              { record_id: ids.attachment, revision: 1 },
            ],
          },
          nutrition_entry_id: ids.nutrition,
          confidence: 0.78,
          interpreted_items: [
            {
              food_description: "chicken and rice bowl",
              amount: 1,
              amount_unit: "bowl",
            },
          ],
          explanation: "Interpretation of the athlete's description and photo.",
        },
      },
    ]);

    await correctRecord(database, {
      record_id: ids.point,
      expected_revision: 1,
      changed_by: toolActor,
      became_known_at: "2026-08-10T12:00:00.000Z",
      became_known_at_basis: "provider_import",
      data: {
        source_id: ids.wearableSource,
        provider_record_id: "whoop-recovery-2026-08-09",
        received_at: "2026-08-10T12:00:00.000Z",
        received_at_basis: "import_time",
        quantity: "hrv_rmssd",
        recorded_at: "2026-08-09T11:12:00.000Z",
        input_timestamp: inputTimestamp,
        value: {
          standard_value: 64,
          standard_unit: "ms",
          input_value: 64,
          input_unit: "ms",
        },
      },
    });

    await deleteRecord(database, {
      record_id: ids.athleteReport,
      expected_revision: 1,
      deleted_by: toolActor,
      deleted_at: "2026-08-11T12:00:00.000Z",
    });

    const foreignKeys = await databaseClient(database).execute("PRAGMA foreign_key_check");
    if (foreignKeys.rows.length > 0) {
      throw new Error("Generated fixture has foreign-key failures");
    }
    const integrity = await databaseClient(database).execute("PRAGMA integrity_check");
    if (integrity.rows[0]?.integrity_check !== "ok") {
      throw new Error("Generated fixture failed SQLite integrity_check");
    }
    await databaseClient(database).execute("PRAGMA wal_checkpoint(TRUNCATE)");
  } finally {
    database.close();
  }

  await Promise.all([
    rm(`${databasePath}-wal`, { force: true }),
    rm(`${databasePath}-shm`, { force: true }),
  ]);
  await chmod(databasePath, 0o600);
  console.log("wrote examples/demo-athlete/athlete.ath.sqlite");
}

await generate();
