/**
 * Athletic Standard v1 domain and portable-package schemas.
 *
 * These schemas describe athlete-owned records. SQLite and the JSON package are
 * encodings of the same records. Database-only settings and service data do not
 * belong here.
 */
import { z } from "zod";
import { RecordExtensions, validateExtensionsForRecordType } from "./extensions.js";

export const ATHLETIC_STANDARD_VERSION = "1.0.0";

export const UtcTimestamp = z.iso
  .datetime({ offset: false, local: false, precision: 3 })
  .describe("Normalized ISO 8601 timestamp in UTC with millisecond precision");

export const CalendarDate = z.iso.date().describe("ISO 8601 calendar date");

export const Semver = z
  .string()
  .regex(/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/, "semantic version");

export const RecordId = z
  .string()
  .regex(
    /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    "lowercase UUIDv7",
  )
  .describe("Stable UUIDv7 record ID");

export const DatabaseId = RecordId.describe("Stable UUIDv7 database ID");

export const Sha256 = z
  .string()
  .regex(/^[a-f0-9]{64}$/, "lowercase SHA-256 hex digest");

export const RecordRevisionNumber = z.number().int().positive();

export const RecordType = z.enum([
  "athlete_profile",
  "source",
  "source_device",
  "point_measurement",
  "measurement_sample",
  "sleep_session",
  "workout_session",
  "benchmark_result",
  "provider_summary",
  "provider_score",
  "athlete_report",
  "nutrition_entry",
  "attachment",
  "benchmark_definition",
  "prediction",
  "prediction_result_link",
  "prediction_grade",
  "prediction_error_analysis",
  "training_plan",
  "planned_session",
  "coaching_note",
  "saved_training_review",
  "goal",
  "nutrition_model_interpretation",
]);

export type RecordTypeT = z.infer<typeof RecordType>;

export const EvidenceReference = z.strictObject({
  record_id: RecordId,
  revision: RecordRevisionNumber,
});

export const AthleteActor = z.strictObject({
  author_type: z.literal("athlete"),
});

export const AgentActor = z.strictObject({
  author_type: z.literal("agent"),
  agent: z.string().min(1),
  agent_version: Semver.optional(),
});

export const ToolActor = z.strictObject({
  author_type: z.literal("tool"),
  tool: z.string().min(1),
  tool_version: Semver,
});

export const RecordActor = z.discriminatedUnion("author_type", [
  AthleteActor,
  AgentActor,
  ToolActor,
]);

export const AgentAuthor = AgentActor.extend({
  model: z.string().min(1),
  model_version: z.string().min(1).optional(),
});

export const ToolAuthor = ToolActor.extend({
  calculation_version: z.string().min(1),
});

export const AuthoredRecordAuthor = z.discriminatedUnion("author_type", [
  AthleteActor,
  AgentAuthor,
  ToolAuthor,
]);

export const AuthoredRecordStatus = z.enum([
  "draft",
  "active",
  "completed",
  "superseded",
  "cancelled",
]);

export const AuthoredRecordMetadata = z.strictObject({
  author: AuthoredRecordAuthor,
  authored_at: UtcTimestamp,
  evidence: z.array(EvidenceReference),
  content_version: z.number().int().positive(),
  status: AuthoredRecordStatus,
});

export const MissingReason = z.enum([
  "not_provided_by_source",
  "not_exposed_by_route",
  "not_applicable",
]);

export const InputTimestampBasis = z.enum([
  "source_instant",
  "source_offset_timestamp",
  "source_local_timestamp_with_documented_offset",
  "reconstructed_from_series_start",
]);

export const UtcOffsetMeaning = z.enum([
  "documented_recording_offset",
  "provider_cycle_offset",
  "route_rendering_offset",
  "meaning_not_documented",
]);

export const SourceTimeZoneMeaning = z.enum([
  "apple_health_object_creation_time_zone",
]);

export const InputTimestampReceipt = z
  .strictObject({
    timestamp_text: z.string().min(1).optional(),
    utc_offset_text: z.string().min(1).optional(),
    utc_offset_meaning: UtcOffsetMeaning.optional(),
    source_time_zone: z.string().min(1).optional(),
    source_time_zone_meaning: SourceTimeZoneMeaning.optional(),
    timestamp_basis: InputTimestampBasis.optional(),
    timestamp_missing_reason: MissingReason.optional(),
    utc_offset_missing_reason: MissingReason.optional(),
  })
  .superRefine((value, context) => {
    if (value.timestamp_text === undefined && value.timestamp_missing_reason === undefined) {
      context.addIssue({
        code: "custom",
        path: ["timestamp_missing_reason"],
        message: "A missing input timestamp needs a reason",
      });
    }
    if (value.timestamp_text !== undefined && value.timestamp_missing_reason !== undefined) {
      context.addIssue({
        code: "custom",
        path: ["timestamp_missing_reason"],
        message: "An available input timestamp cannot also be marked missing",
      });
    }
    if (value.timestamp_text !== undefined && value.timestamp_basis === undefined) {
      context.addIssue({
        code: "custom",
        path: ["timestamp_basis"],
        message: "An input timestamp needs its documented basis",
      });
    }
    if (value.utc_offset_text === undefined && value.utc_offset_missing_reason === undefined) {
      context.addIssue({
        code: "custom",
        path: ["utc_offset_missing_reason"],
        message: "A missing input UTC offset needs a reason",
      });
    }
    if (value.utc_offset_text !== undefined && value.utc_offset_missing_reason !== undefined) {
      context.addIssue({
        code: "custom",
        path: ["utc_offset_missing_reason"],
        message: "An available input UTC offset cannot also be marked missing",
      });
    }
    if (value.utc_offset_text !== undefined && value.utc_offset_meaning === undefined) {
      context.addIssue({
        code: "custom",
        path: ["utc_offset_meaning"],
        message: "An input UTC offset needs its documented meaning",
      });
    }
    if (value.utc_offset_text === undefined && value.utc_offset_meaning !== undefined) {
      context.addIssue({
        code: "custom",
        path: ["utc_offset_meaning"],
        message: "UTC offset meaning requires UTC offset text",
      });
    }
    if (
      (value.source_time_zone === undefined) !==
      (value.source_time_zone_meaning === undefined)
    ) {
      context.addIssue({
        code: "custom",
        path: ["source_time_zone_meaning"],
        message:
          "A source timezone and its documented meaning must be provided together",
      });
    }
  });

export const ReceivedAtBasis = z.enum([
  "provider_supplied",
  "connector_received_time",
  "file_modified_time",
  "import_time",
  "local_write_time",
]);

export const ProviderReportingPeriodType = z.enum([
  "whoop_physiological_cycle",
  "oura_activity_day",
  "oura_sleep_day",
  "oura_calendar_day",
  "oura_day_unspecified",
  "apple_activity_summary_day",
]);

export const ProviderReportingPeriod = z
  .strictObject({
    period_type: ProviderReportingPeriodType,
    provider_period_id: z.string().min(1).optional(),
    provider_date: CalendarDate.optional(),
    start_at: UtcTimestamp.optional(),
    end_at: UtcTimestamp.optional(),
    input_start_timestamp: InputTimestampReceipt.optional(),
    input_end_timestamp: InputTimestampReceipt.optional(),
    timezone_offset_text: z.string().min(1).optional(),
    timezone_offset_meaning: UtcOffsetMeaning.optional(),
    timezone_offset_missing_reason: MissingReason.optional(),
  })
  .superRefine((value, context) => {
    if (
      value.provider_period_id === undefined &&
      value.provider_date === undefined &&
      value.start_at === undefined
    ) {
      context.addIssue({
        code: "custom",
        message: "A provider reporting period needs an ID, date, or start instant",
      });
    }
    if (
      value.start_at !== undefined &&
      value.end_at !== undefined &&
      Date.parse(value.end_at) < Date.parse(value.start_at)
    ) {
      context.addIssue({
        code: "custom",
        path: ["end_at"],
        message: "Provider reporting period end cannot precede its start",
      });
    }
    if (
      value.timezone_offset_text !== undefined &&
      value.timezone_offset_meaning === undefined
    ) {
      context.addIssue({
        code: "custom",
        path: ["timezone_offset_meaning"],
        message: "A provider UTC offset needs its documented meaning",
      });
    }
    if (
      value.timezone_offset_text !== undefined &&
      value.timezone_offset_missing_reason !== undefined
    ) {
      context.addIssue({
        code: "custom",
        path: ["timezone_offset_missing_reason"],
        message: "An available provider UTC offset cannot also be marked missing",
      });
    }
  });

const sourceValueFields = {
  standard_value: z.number(),
  standard_unit: z.string().min(1),
  input_value: z.number().optional(),
  input_unit: z.string().min(1).optional(),
  input_value_missing_reason: MissingReason.optional(),
  input_unit_missing_reason: MissingReason.optional(),
} as const;

function validateSourceValue(
  value: {
    input_value?: number | undefined;
    input_unit?: string | undefined;
    input_value_missing_reason?: z.infer<typeof MissingReason> | undefined;
    input_unit_missing_reason?: z.infer<typeof MissingReason> | undefined;
  },
  context: z.RefinementCtx,
): void {
  if ((value.input_value === undefined) === (value.input_value_missing_reason === undefined)) {
    context.addIssue({
      code: "custom",
      path: ["input_value_missing_reason"],
      message: "Provide the input value or one missing reason",
    });
  }
  if ((value.input_unit === undefined) === (value.input_unit_missing_reason === undefined)) {
    context.addIssue({
      code: "custom",
      path: ["input_unit_missing_reason"],
      message: "Provide the input unit or one missing reason",
    });
  }
}

export const SourceValue = z
  .strictObject(sourceValueFields)
  .superRefine((value, context) => {
    validateSourceValue(value, context);
  });

export const SourceReportedSummary = z
  .strictObject({
    ...sourceValueFields,
    summary_type: z.string().min(1),
  })
  .superRefine((value, context) => {
    validateSourceValue(value, context);
  });

const measuredRecordFields = {
  source_id: RecordId,
  provider_record_id: z.string().min(1).optional(),
  provider_reporting_periods: z.array(ProviderReportingPeriod).optional(),
  received_at: UtcTimestamp,
  received_at_basis: ReceivedAtBasis,
} as const;

function isIanaTimeZone(value: string): boolean {
  try {
    new Intl.DateTimeFormat("en-US", { timeZone: value }).format();
    return true;
  } catch {
    return false;
  }
}

export const AthleteProfileData = z.strictObject({
  name: z.string().min(1).optional(),
  birth_year: z.number().int().min(1900).max(2100).optional(),
  sex: z.enum(["male", "female"]).optional(),
  display_units: z.enum(["metric", "imperial"]).optional(),
  default_time_zone: z
    .string()
    .min(1)
    .refine(isIanaTimeZone, "IANA timezone")
    .optional()
    .describe("IANA timezone for calendar calculations"),
});

export const SourceType = z.enum(["wearable", "export_file", "connector", "manual"]);

export const SourceData = z.strictObject({
  source_type: SourceType,
  data_writer: z.string().min(1),
  data_route: z.string().min(1),
  vendor: z.string().min(1).optional(),
  sensor_type: z.string().min(1).optional(),
  detail: z.string().min(1).optional(),
});

export const SourceDeviceData = z.strictObject({
  source_id: RecordId,
  name: z.string().min(1).optional(),
  manufacturer: z.string().min(1).optional(),
  model: z.string().min(1).optional(),
  hardware: z.string().min(1).optional(),
  serial_number: z.string().min(1).optional(),
  coverage_start_date: CalendarDate.optional(),
  coverage_end_date: CalendarDate.optional(),
  accepted_record_count: z.number().int().nonnegative().optional(),
});

export const POINT_MEASUREMENT_UNITS = {
  hrv_rmssd: "ms",
  hrv_sdnn: "ms",
  resting_heart_rate: "bpm",
  body_weight: "kg",
  respiratory_rate: "brpm",
  vo2_max: "ml/kg/min",
  hr_recovery: "bpm",
  walking_heart_rate: "bpm",
  oxygen_saturation: "%",
  body_temperature: "°C",
  skin_temperature: "°C",
  wrist_temperature_sleeping: "°C",
  temperature_deviation: "°C",
  height: "cm",
  lean_body_mass: "kg",
  body_fat_percentage: "%",
  blood_pressure_systolic: "mmHg",
  blood_pressure_diastolic: "mmHg",
} as const;

export type PointMeasurementType = keyof typeof POINT_MEASUREMENT_UNITS;

export const PointMeasurementTypeSchema = z.enum(
  Object.keys(POINT_MEASUREMENT_UNITS) as [PointMeasurementType, ...PointMeasurementType[]],
);

export const SERIES_QUANTITY_UNITS = {
  heart_rate: "bpm",
  hrv_beats: "ms",
  ecg_beats: "ms",
  hrv_sdnn: "ms",
  respiratory_rate: "brpm",
  oxygen_saturation: "%",
  steps: "count",
  active_energy: "kcal",
  distance_walking_running: "m",
  distance_cycling: "m",
  distance_swimming: "m",
  running_speed: "m/s",
  running_power: "W",
  running_stride_length: "m",
  running_vertical_oscillation: "cm",
  running_ground_contact_time: "ms",
  physical_effort: "MET",
  basal_energy: "kcal",
  exercise_time: "min",
  flights_climbed: "count",
  walking_speed: "m/s",
  walking_step_length: "m",
  walking_asymmetry_percentage: "%",
  time_in_daylight: "min",
} as const;

export type SeriesQuantity = keyof typeof SERIES_QUANTITY_UNITS;

export const SeriesQuantitySchema = z.enum(
  Object.keys(SERIES_QUANTITY_UNITS) as [SeriesQuantity, ...SeriesQuantity[]],
);

export const PointMeasurementCalculation = z.strictObject({
  from_quantity: z.string().min(1),
  method: z.string().min(1),
  calculation_version: z.string().min(1),
  window_s: z.number().positive().optional(),
  usable_sample_count: z.number().int().positive().optional(),
  dropped_sample_count: z.number().int().nonnegative().optional(),
  signal_quality: z.number().nonnegative().optional(),
  input_records: z.array(EvidenceReference),
});

export const PointMeasurementData = z
  .strictObject({
    ...measuredRecordFields,
    quantity: PointMeasurementTypeSchema,
    recorded_at: UtcTimestamp.optional(),
    input_timestamp: InputTimestampReceipt,
    value: SourceValue,
    calculations: z.array(PointMeasurementCalculation).optional(),
  })
  .superRefine((value, context) => {
    const expectedUnit = POINT_MEASUREMENT_UNITS[value.quantity];
    if (value.value.standard_unit !== expectedUnit) {
      context.addIssue({
        code: "custom",
        path: ["value", "standard_unit"],
        message: `${value.quantity} uses ${expectedUnit}`,
      });
    }
    if (value.quantity !== "temperature_deviation" && value.value.standard_value <= 0) {
      context.addIssue({
        code: "custom",
        path: ["value", "standard_value"],
        message: `${value.quantity} must be positive`,
      });
    }
  });

export const MeasurementSampleData = z
  .strictObject({
    ...measuredRecordFields,
    quantity: SeriesQuantitySchema,
    recorded_at: UtcTimestamp,
    input_timestamp: InputTimestampReceipt,
    value: SourceValue,
    duration_ms: z.number().int().nonnegative().optional(),
    timestamp_base_text: z.string().min(1).optional(),
    timestamp_reconstruction_method: z.string().min(1).optional(),
  })
  .superRefine((value, context) => {
    const expectedUnit = SERIES_QUANTITY_UNITS[value.quantity];
    if (value.value.standard_unit !== expectedUnit) {
      context.addIssue({
        code: "custom",
        path: ["value", "standard_unit"],
        message: `${value.quantity} uses ${expectedUnit}`,
      });
    }
    const reconstructed =
      value.input_timestamp.timestamp_basis === "reconstructed_from_series_start";
    if (
      reconstructed !==
      (value.timestamp_base_text !== undefined &&
        value.timestamp_reconstruction_method !== undefined)
    ) {
      context.addIssue({
        code: "custom",
        path: ["timestamp_reconstruction_method"],
        message:
          "A reconstructed timestamp needs its base and method, and other timestamps cannot claim them",
      });
    }
  });

export const SleepSessionData = z
  .strictObject({
    ...measuredRecordFields,
    start_at: UtcTimestamp,
    end_at: UtcTimestamp,
    input_start_timestamp: InputTimestampReceipt,
    input_end_timestamp: InputTimestampReceipt,
    source_reported_summaries: z.array(SourceReportedSummary),
  })
  .refine((value) => Date.parse(value.end_at) >= Date.parse(value.start_at), {
    path: ["end_at"],
    message: "Sleep session end cannot precede its start",
  });

export const WorkoutSessionSegment = z.strictObject({
  label: z.string().min(1),
  start_offset_ms: z.number().int().nonnegative().optional(),
  end_offset_ms: z.number().int().nonnegative().optional(),
  source_reported_summaries: z.array(SourceReportedSummary),
});

export const WorkoutSessionData = z
  .strictObject({
    ...measuredRecordFields,
    start_at: UtcTimestamp,
    end_at: UtcTimestamp,
    input_start_timestamp: InputTimestampReceipt,
    input_end_timestamp: InputTimestampReceipt,
    activity_type: z.string().min(1).optional(),
    source_reported_summaries: z.array(SourceReportedSummary),
    segments: z.array(WorkoutSessionSegment).optional(),
  })
  .refine((value) => Date.parse(value.end_at) >= Date.parse(value.start_at), {
    path: ["end_at"],
    message: "Workout session end cannot precede its start",
  });

export const Score = z
  .strictObject({
    duration_s: z.number().positive().optional(),
    reps: z.number().int().positive().optional(),
    weight_kg: z.number().positive().optional(),
  })
  .refine(
    (value) =>
      value.duration_s !== undefined || value.reps !== undefined || value.weight_kg !== undefined,
    { message: "A score needs duration_s, reps, or weight_kg" },
  );

export const BenchmarkResultSegment = z.strictObject({
  label: z.string().min(1),
  score: Score.optional(),
});

export const BenchmarkResultData = z.strictObject({
  ...measuredRecordFields,
  benchmark_definition_id: RecordId,
  recorded_at: UtcTimestamp,
  input_timestamp: InputTimestampReceipt,
  score: Score,
  input_score_text: z.string().min(1).optional(),
  scaling: z.enum(["rx", "scaled"]).optional(),
  workout_session_id: RecordId.optional(),
  segments: z.array(BenchmarkResultSegment).optional(),
});

export const ProviderSummaryData = z
  .strictObject({
    ...measuredRecordFields,
    summary_type: z.string().min(1),
    value: SourceValue,
    recorded_at: UtcTimestamp.optional(),
    input_timestamp: InputTimestampReceipt,
  })
  .refine(
    (value) =>
      value.recorded_at !== undefined ||
      (value.provider_reporting_periods !== undefined &&
        value.provider_reporting_periods.length > 0),
    {
      path: ["provider_reporting_periods"],
      message: "A provider summary needs an instant or a provider reporting period",
    },
  );

export const ProviderScoreData = z
  .strictObject({
    ...measuredRecordFields,
    metric: z.string().min(1),
    value: SourceValue,
    scale: z.string().min(1),
    recorded_at: UtcTimestamp.optional(),
    input_timestamp: InputTimestampReceipt,
  })
  .refine(
    (value) =>
      value.recorded_at !== undefined ||
      (value.provider_reporting_periods !== undefined &&
        value.provider_reporting_periods.length > 0),
    {
      path: ["provider_reporting_periods"],
      message: "A provider score needs an instant or a provider reporting period",
    },
  );

export const AthleteReportType = z.enum([
  "sleep_quality",
  "soreness",
  "stress",
  "mood",
  "energy",
  "note",
]);

export const AthleteReportData = z
  .strictObject({
    report_type: AthleteReportType,
    reported_at: UtcTimestamp,
    input_reported_at: InputTimestampReceipt,
    reported_via: z.enum(["text", "voice", "photo"]),
    rating: z.number().optional(),
    rating_scale: z.string().min(1).optional(),
    body_region: z.string().min(1).optional(),
    note: z.string().min(1).optional(),
    attachment_ids: z.array(RecordId).optional(),
  })
  .refine((value) => value.rating === undefined || value.rating_scale !== undefined, {
    path: ["rating_scale"],
    message: "A rating needs its scale",
  });

export const ReportedNutrient = z.strictObject({
  nutrient_name: z.string().min(1),
  amount: z.number(),
  unit: z.string().min(1),
});

export const ReportedNutritionItem = z.strictObject({
  food_description: z.string().min(1),
  amount: z.number().optional(),
  amount_unit: z.string().min(1).optional(),
  nutrients: z.array(ReportedNutrient).optional(),
});

export const NutritionEntryData = z.strictObject({
  reported_at: UtcTimestamp,
  input_reported_at: InputTimestampReceipt,
  reported_via: z.enum(["text", "voice", "photo"]),
  athlete_description: z.string().min(1),
  reported_items: z.array(ReportedNutritionItem),
  attachment_ids: z.array(RecordId).optional(),
});

export const AttachmentData = z.strictObject({
  file_name: z.string().min(1),
  media_type: z.string().min(1),
  byte_count: z.number().int().nonnegative(),
  sha256: Sha256,
  description: z.string().min(1).optional(),
});

const authoredRecordFields = {
  authorship: AuthoredRecordMetadata,
} as const;

export const BenchmarkDefinitionData = z.strictObject({
  ...authoredRecordFields,
  benchmark_key: z
    .string()
    .regex(/^[a-z0-9][a-z0-9_-]*$/, "lowercase key with hyphens or underscores"),
  benchmark_type: z.enum(["named_wod", "run", "hyrox", "lift", "custom"]),
  score_type: z.enum(["time", "reps", "load"]),
  definition: z.string().min(1),
  tags: z.array(z.string().min(1)).optional(),
});

export const Confidence = z.enum(["low", "moderate", "high"]);

export const PredictionData = z.strictObject({
  ...authoredRecordFields,
  benchmark_definition_id: RecordId,
  predicted_score: Score,
  predicted_range: z.strictObject({ low: Score, high: Score }).optional(),
  confidence: Confidence,
  reasoning: z.string().min(1),
});

export const PredictionResultLinkData = z.strictObject({
  prediction: EvidenceReference,
  benchmark_result: EvidenceReference,
  created_by: RecordActor,
  linked_at: UtcTimestamp,
  content_version: z.number().int().positive(),
  status: AuthoredRecordStatus,
});

export const PredictionGradeData = z.strictObject({
  ...authoredRecordFields,
  prediction_result_link: EvidenceReference,
  signed_error: z.number(),
  absolute_error_percentage: z.number().nonnegative(),
  result_in_predicted_range: z.boolean(),
  predicted_score: Score,
  actual_score: Score,
});

export const PredictionErrorCause = z.strictObject({
  evidence: EvidenceReference,
  explanation: z.string().min(1),
});

export const PredictionErrorAnalysisData = z
  .strictObject({
    ...authoredRecordFields,
    prediction_grade: EvidenceReference,
    error_direction: z.enum(["faster", "slower", "higher", "lower"]),
    error_severity: z.enum(["minor", "significant", "severe"]),
    causes: z.array(PredictionErrorCause),
    unexplained: z.boolean(),
    lesson: z.string().min(1).optional(),
  })
  .refine((value) => value.unexplained || value.causes.length > 0, {
    path: ["unexplained"],
    message: "An analysis without causes must mark the error unexplained",
  });

export const TrainingPlanData = z.strictObject({
  ...authoredRecordFields,
  title: z.string().min(1),
  objective: z.string().min(1).optional(),
  start_date: CalendarDate.optional(),
  end_date: CalendarDate.optional(),
});

export const PlannedSessionBlock = z.strictObject({
  block_type: z.string().min(1),
  title: z.string().min(1),
  instructions: z.string().min(1).optional(),
  planned_duration_s: z.number().positive().optional(),
  planned_distance_m: z.number().positive().optional(),
  planned_reps: z.number().int().positive().optional(),
  planned_weight_kg: z.number().positive().optional(),
});

export const PlannedSessionData = z
  .strictObject({
    ...authoredRecordFields,
    training_plan_id: RecordId.optional(),
    title: z.string().min(1),
    scheduled_date: CalendarDate.optional(),
    scheduled_start_at: UtcTimestamp.optional(),
    activity_type: z.string().min(1).optional(),
    blocks: z.array(PlannedSessionBlock),
    completion_status: z.enum(["planned", "completed", "skipped", "cancelled"]),
    completed_workout_session_id: RecordId.optional(),
  })
  .refine(
    (value) => value.scheduled_date !== undefined || value.scheduled_start_at !== undefined,
    {
      path: ["scheduled_date"],
      message: "A planned session needs a date or start instant",
    },
  );

export const CoachingNoteData = z.strictObject({
  ...authoredRecordFields,
  title: z.string().min(1).optional(),
  note: z.string().min(1),
  applies_from: CalendarDate.optional(),
  applies_to: CalendarDate.optional(),
});

export const SavedTrainingReviewData = z.strictObject({
  ...authoredRecordFields,
  period_start: CalendarDate,
  period_end: CalendarDate,
  summary: z.string().min(1),
  recommendations: z.array(z.string().min(1)).optional(),
});

export const GoalData = z.strictObject({
  ...authoredRecordFields,
  title: z.string().min(1),
  description: z.string().min(1).optional(),
  target_date: CalendarDate.optional(),
  target_benchmark_definition_id: RecordId.optional(),
  target_score: Score.optional(),
});

export const NutritionModelInterpretationData = z.strictObject({
  ...authoredRecordFields,
  nutrition_entry_id: RecordId,
  confidence: z.number().min(0).max(1),
  interpreted_items: z.array(ReportedNutritionItem),
  explanation: z.string().min(1).optional(),
});

const currentRecordFields = {
  record_id: RecordId,
  created_at: UtcTimestamp,
  current_revision: RecordRevisionNumber,
  is_deleted: z.boolean(),
  extensions: RecordExtensions.optional(),
} as const;

function currentRecordVariant<const T extends RecordTypeT, S extends z.ZodType>(
  recordType: T,
  data: S,
) {
  return z
    .strictObject({
      ...currentRecordFields,
      record_type: z.literal(recordType),
      data,
    })
    .superRefine((value, context) => {
      try {
        validateExtensionsForRecordType(recordType, value.extensions);
      } catch (error) {
        context.addIssue({
          code: "custom",
          path: ["extensions"],
          message: (error as Error).message,
        });
      }
    });
}

export const AthleteProfileRecord = currentRecordVariant("athlete_profile", AthleteProfileData);
export const SourceRecord = currentRecordVariant("source", SourceData);
export const SourceDeviceRecord = currentRecordVariant("source_device", SourceDeviceData);
export const PointMeasurementRecord = currentRecordVariant(
  "point_measurement",
  PointMeasurementData,
);
export const MeasurementSampleRecord = currentRecordVariant(
  "measurement_sample",
  MeasurementSampleData,
);
export const SleepSessionRecord = currentRecordVariant("sleep_session", SleepSessionData);
export const WorkoutSessionRecord = currentRecordVariant("workout_session", WorkoutSessionData);
export const BenchmarkResultRecord = currentRecordVariant(
  "benchmark_result",
  BenchmarkResultData,
);
export const ProviderSummaryRecord = currentRecordVariant(
  "provider_summary",
  ProviderSummaryData,
);
export const ProviderScoreRecord = currentRecordVariant("provider_score", ProviderScoreData);
export const AthleteReportRecord = currentRecordVariant("athlete_report", AthleteReportData);
export const NutritionEntryRecord = currentRecordVariant("nutrition_entry", NutritionEntryData);
export const AttachmentRecord = currentRecordVariant("attachment", AttachmentData);
export const BenchmarkDefinitionRecord = currentRecordVariant(
  "benchmark_definition",
  BenchmarkDefinitionData,
);
export const PredictionRecord = currentRecordVariant("prediction", PredictionData);
export const PredictionResultLinkRecord = currentRecordVariant(
  "prediction_result_link",
  PredictionResultLinkData,
);
export const PredictionGradeRecord = currentRecordVariant(
  "prediction_grade",
  PredictionGradeData,
);
export const PredictionErrorAnalysisRecord = currentRecordVariant(
  "prediction_error_analysis",
  PredictionErrorAnalysisData,
);
export const TrainingPlanRecord = currentRecordVariant("training_plan", TrainingPlanData);
export const PlannedSessionRecord = currentRecordVariant("planned_session", PlannedSessionData);
export const CoachingNoteRecord = currentRecordVariant("coaching_note", CoachingNoteData);
export const SavedTrainingReviewRecord = currentRecordVariant(
  "saved_training_review",
  SavedTrainingReviewData,
);
export const GoalRecord = currentRecordVariant("goal", GoalData);
export const NutritionModelInterpretationRecord = currentRecordVariant(
  "nutrition_model_interpretation",
  NutritionModelInterpretationData,
);

const nonSampleRecordSchemas = [
  AthleteProfileRecord,
  SourceRecord,
  SourceDeviceRecord,
  PointMeasurementRecord,
  SleepSessionRecord,
  WorkoutSessionRecord,
  BenchmarkResultRecord,
  ProviderSummaryRecord,
  ProviderScoreRecord,
  AthleteReportRecord,
  NutritionEntryRecord,
  AttachmentRecord,
  BenchmarkDefinitionRecord,
  PredictionRecord,
  PredictionResultLinkRecord,
  PredictionGradeRecord,
  PredictionErrorAnalysisRecord,
  TrainingPlanRecord,
  PlannedSessionRecord,
  CoachingNoteRecord,
  SavedTrainingReviewRecord,
  GoalRecord,
  NutritionModelInterpretationRecord,
] as const;

export const NonSampleRecord = z.discriminatedUnion("record_type", nonSampleRecordSchemas);

export const CurrentRecord = z.discriminatedUnion("record_type", [
  ...nonSampleRecordSchemas,
  MeasurementSampleRecord,
]);

export const BecameKnownAtBasis = z.enum([
  "local_write",
  "provider_import",
  "package_restore",
  "package_merge",
]);

const recordRevisionFields = {
  record_id: RecordId,
  revision: RecordRevisionNumber,
  previous_revision: RecordRevisionNumber.nullable(),
  became_known_at: UtcTimestamp,
  became_known_at_basis: BecameKnownAtBasis,
  changed_by: RecordActor,
} as const;

function recordRevisionVariant<const T extends RecordTypeT, S extends z.ZodType>(
  recordType: T,
  data: S,
) {
  return z
    .strictObject({
      ...recordRevisionFields,
      record_type: z.literal(recordType),
      data,
      extensions: RecordExtensions.optional(),
    })
    .refine(
      (value) =>
        value.revision === 1
          ? value.previous_revision === null
          : value.previous_revision === value.revision - 1,
      {
        path: ["previous_revision"],
        message: "previous_revision must be null for revision 1 or the prior revision number",
      },
    )
    .superRefine((value, context) => {
      try {
        validateExtensionsForRecordType(recordType, value.extensions);
      } catch (error) {
        context.addIssue({
          code: "custom",
          path: ["extensions"],
          message: (error as Error).message,
        });
      }
    });
}

export const AthleteProfileRevision = recordRevisionVariant(
  "athlete_profile",
  AthleteProfileData,
);
export const SourceRevision = recordRevisionVariant("source", SourceData);
export const SourceDeviceRevision = recordRevisionVariant("source_device", SourceDeviceData);
export const PointMeasurementRevision = recordRevisionVariant(
  "point_measurement",
  PointMeasurementData,
);
export const MeasurementSampleRevision = recordRevisionVariant(
  "measurement_sample",
  MeasurementSampleData,
);
export const SleepSessionRevision = recordRevisionVariant("sleep_session", SleepSessionData);
export const WorkoutSessionRevision = recordRevisionVariant(
  "workout_session",
  WorkoutSessionData,
);
export const BenchmarkResultRevision = recordRevisionVariant(
  "benchmark_result",
  BenchmarkResultData,
);
export const ProviderSummaryRevision = recordRevisionVariant(
  "provider_summary",
  ProviderSummaryData,
);
export const ProviderScoreRevision = recordRevisionVariant("provider_score", ProviderScoreData);
export const AthleteReportRevision = recordRevisionVariant("athlete_report", AthleteReportData);
export const NutritionEntryRevision = recordRevisionVariant(
  "nutrition_entry",
  NutritionEntryData,
);
export const AttachmentRevision = recordRevisionVariant("attachment", AttachmentData);
export const BenchmarkDefinitionRevision = recordRevisionVariant(
  "benchmark_definition",
  BenchmarkDefinitionData,
);
export const PredictionRevision = recordRevisionVariant("prediction", PredictionData);
export const PredictionResultLinkRevision = recordRevisionVariant(
  "prediction_result_link",
  PredictionResultLinkData,
);
export const PredictionGradeRevision = recordRevisionVariant(
  "prediction_grade",
  PredictionGradeData,
);
export const PredictionErrorAnalysisRevision = recordRevisionVariant(
  "prediction_error_analysis",
  PredictionErrorAnalysisData,
);
export const TrainingPlanRevision = recordRevisionVariant("training_plan", TrainingPlanData);
export const PlannedSessionRevision = recordRevisionVariant(
  "planned_session",
  PlannedSessionData,
);
export const CoachingNoteRevision = recordRevisionVariant("coaching_note", CoachingNoteData);
export const SavedTrainingReviewRevision = recordRevisionVariant(
  "saved_training_review",
  SavedTrainingReviewData,
);
export const GoalRevision = recordRevisionVariant("goal", GoalData);
export const NutritionModelInterpretationRevision = recordRevisionVariant(
  "nutrition_model_interpretation",
  NutritionModelInterpretationData,
);

const nonSampleRevisionSchemas = [
  AthleteProfileRevision,
  SourceRevision,
  SourceDeviceRevision,
  PointMeasurementRevision,
  SleepSessionRevision,
  WorkoutSessionRevision,
  BenchmarkResultRevision,
  ProviderSummaryRevision,
  ProviderScoreRevision,
  AthleteReportRevision,
  NutritionEntryRevision,
  AttachmentRevision,
  BenchmarkDefinitionRevision,
  PredictionRevision,
  PredictionResultLinkRevision,
  PredictionGradeRevision,
  PredictionErrorAnalysisRevision,
  TrainingPlanRevision,
  PlannedSessionRevision,
  CoachingNoteRevision,
  SavedTrainingReviewRevision,
  GoalRevision,
  NutritionModelInterpretationRevision,
] as const;

export const NonSampleRecordRevision = z.union(nonSampleRevisionSchemas);

export const RecordRevision = z.union([
  ...nonSampleRevisionSchemas,
  MeasurementSampleRevision,
]);

export const RecordDeletion = z.strictObject({
  record_id: RecordId,
  record_revision: RecordRevisionNumber,
  deleted_at: UtcTimestamp,
  deleted_by: RecordActor,
});

export const ErasureReasonCode = z.enum([
  "athlete_request",
  "legal_requirement",
  "recorded_in_error",
  "other",
]);

export const RecordErasureMarker = z.strictObject({
  record_id: RecordId,
  erased_at: UtcTimestamp,
  reason_code: ErasureReasonCode,
});

export const RecordImportKey = z.strictObject({
  record_id: RecordId,
  match_key_method: z.string().min(1),
  match_key: z.string().min(1),
  imported_at: UtcTimestamp,
});

export const PackageRelativePath = z
  .string()
  .min(1)
  .regex(
    /^(?!\/)(?!.*\\)(?!.*(?:^|\/)\.\.(?:\/|$)).+$/,
    "Package path must be relative and cannot traverse parent directories",
  );

export const MeasurementSeriesFileReference = z.strictObject({
  file: PackageRelativePath,
  sha256: Sha256,
  source_id: RecordId,
  quantity: SeriesQuantitySchema,
  start_at: UtcTimestamp,
  end_at: UtcTimestamp,
  sample_count: z.number().int().nonnegative(),
  revision_count: z.number().int().nonnegative(),
  deletion_count: z.number().int().nonnegative(),
  import_key_count: z.number().int().nonnegative(),
});

type CurrentHistoryRecord = {
  record_id: string;
  record_type: string;
  current_revision: number;
  is_deleted: boolean;
  data: unknown;
};

type HistoryRevision = {
  record_id: string;
  record_type: string;
  revision: number;
  previous_revision: number | null;
  data: unknown;
};

type HistoryDeletion = {
  record_id: string;
  record_revision: number;
};

type HistoryImportKey = {
  record_id: string;
  match_key_method: string;
  match_key: string;
};

function validateCompleteRecordHistory(
  records: readonly CurrentHistoryRecord[],
  revisions: readonly HistoryRevision[],
  deletions: readonly HistoryDeletion[],
  importKeys: readonly HistoryImportKey[],
  context: z.RefinementCtx,
): void {
  const recordsById = new Map<string, { record: CurrentHistoryRecord; index: number }>();
  records.forEach((record, index) => {
    if (recordsById.has(record.record_id)) {
      context.addIssue({
        code: "custom",
        path: ["records", index, "record_id"],
        message: "A package cannot contain the same current record twice",
      });
      return;
    }
    recordsById.set(record.record_id, { record, index });
  });

  const revisionsByRecord = new Map<string, HistoryRevision[]>();
  const revisionKeys = new Set<string>();
  revisions.forEach((revision, index) => {
    const key = `${revision.record_id}:${revision.revision}`;
    if (revisionKeys.has(key)) {
      context.addIssue({
        code: "custom",
        path: ["record_revisions", index],
        message: "A package cannot contain the same record revision twice",
      });
    }
    revisionKeys.add(key);
    const owner = recordsById.get(revision.record_id);
    if (owner === undefined) {
      context.addIssue({
        code: "custom",
        path: ["record_revisions", index, "record_id"],
        message: "A retained revision needs its current record",
      });
    } else if (owner.record.record_type !== revision.record_type) {
      context.addIssue({
        code: "custom",
        path: ["record_revisions", index, "record_type"],
        message: "A revision must keep its record type",
      });
    }
    const group = revisionsByRecord.get(revision.record_id) ?? [];
    group.push(revision);
    revisionsByRecord.set(revision.record_id, group);
  });

  const deletionByRecord = new Map<string, HistoryDeletion>();
  deletions.forEach((deletion, index) => {
    const owner = recordsById.get(deletion.record_id);
    if (owner === undefined) {
      context.addIssue({
        code: "custom",
        path: ["record_deletions", index, "record_id"],
        message: "A deletion record needs its retained current record",
      });
    }
    if (deletionByRecord.has(deletion.record_id)) {
      context.addIssue({
        code: "custom",
        path: ["record_deletions", index, "record_id"],
        message: "A record can have only one current deletion record",
      });
    }
    deletionByRecord.set(deletion.record_id, deletion);
  });

  recordsById.forEach(({ record, index }) => {
    const recordRevisions = (revisionsByRecord.get(record.record_id) ?? []).sort(
      (left, right) => left.revision - right.revision,
    );
    if (recordRevisions.length !== record.current_revision) {
      context.addIssue({
        code: "custom",
        path: ["records", index, "current_revision"],
        message: "A complete package needs every revision through current_revision",
      });
    }
    recordRevisions.forEach((revision, revisionIndex) => {
      const expected = revisionIndex + 1;
      if (revision.revision !== expected) {
        context.addIssue({
          code: "custom",
          path: ["records", index, "current_revision"],
          message: "Record revision numbers must be complete and consecutive",
        });
      }
    });
    const latestRevision = recordRevisions.at(-1);
    if (
      latestRevision !== undefined &&
      (latestRevision.revision !== record.current_revision ||
        JSON.stringify(latestRevision.data) !== JSON.stringify(record.data))
    ) {
      context.addIssue({
        code: "custom",
        path: ["records", index, "data"],
        message: "The current record must equal its current retained revision",
      });
    }

    const deletion = deletionByRecord.get(record.record_id);
    if (record.is_deleted && deletion === undefined) {
      context.addIssue({
        code: "custom",
        path: ["records", index, "is_deleted"],
        message: "A deleted current record needs its deletion record",
      });
    }
    if (!record.is_deleted && deletion !== undefined) {
      context.addIssue({
        code: "custom",
        path: ["records", index, "is_deleted"],
        message: "A current deletion record requires is_deleted",
      });
    }
    if (deletion !== undefined && deletion.record_revision !== record.current_revision) {
      context.addIssue({
        code: "custom",
        path: ["records", index, "current_revision"],
        message: "A deletion record must name the current retained revision",
      });
    }
  });

  const importKeyOwners = new Map<string, string>();
  importKeys.forEach((importKey, index) => {
    if (!recordsById.has(importKey.record_id)) {
      context.addIssue({
        code: "custom",
        path: ["record_import_keys", index, "record_id"],
        message: "A provider import key needs its current record",
      });
    }
    const key = `${importKey.match_key_method}\u0000${importKey.match_key}`;
    const existingOwner = importKeyOwners.get(key);
    if (existingOwner !== undefined && existingOwner !== importKey.record_id) {
      context.addIssue({
        code: "custom",
        path: ["record_import_keys", index, "match_key"],
        message: "One provider import key cannot identify two records",
      });
    }
    importKeyOwners.set(key, importKey.record_id);
  });
}

export const MeasurementSeriesFile = z
  .strictObject({
    athleticstandard_version: z.literal(ATHLETIC_STANDARD_VERSION),
    source_id: RecordId,
    quantity: SeriesQuantitySchema,
    start_at: UtcTimestamp,
    end_at: UtcTimestamp,
    records: z.array(MeasurementSampleRecord),
    record_revisions: z.array(MeasurementSampleRevision),
    record_deletions: z.array(RecordDeletion),
    record_import_keys: z.array(RecordImportKey),
  })
  .superRefine((value, context) => {
    validateCompleteRecordHistory(
      value.records,
      value.record_revisions,
      value.record_deletions,
      value.record_import_keys,
      context,
    );
    if (Date.parse(value.end_at) < Date.parse(value.start_at)) {
      context.addIssue({
        code: "custom",
        path: ["end_at"],
        message: "Series file end cannot precede its start",
      });
    }
    value.records.forEach((record, index) => {
      if (record.data.source_id !== value.source_id || record.data.quantity !== value.quantity) {
        context.addIssue({
          code: "custom",
          path: ["records", index],
          message: "Every sample must match the series source and quantity",
        });
      }
      const recordedAt = Date.parse(record.data.recorded_at);
      if (recordedAt < Date.parse(value.start_at) || recordedAt > Date.parse(value.end_at)) {
        context.addIssue({
          code: "custom",
          path: ["records", index, "data", "recorded_at"],
          message: "Every current sample must fall inside the series time bounds",
        });
      }
    });
  });

export const AthleticStandardPackage = z
  .strictObject({
    athleticstandard_version: z.literal(ATHLETIC_STANDARD_VERSION),
    database_id: DatabaseId,
    database_revision: z.number().int().nonnegative(),
    records: z.array(NonSampleRecord),
    record_revisions: z.array(NonSampleRecordRevision),
    record_deletions: z.array(RecordDeletion),
    record_erasure_markers: z.array(RecordErasureMarker),
    record_import_keys: z.array(RecordImportKey),
    measurement_series_files: z.array(MeasurementSeriesFileReference),
  })
  .superRefine((value, context) => {
    validateCompleteRecordHistory(
      value.records,
      value.record_revisions,
      value.record_deletions,
      value.record_import_keys,
      context,
    );

    const seriesFiles = new Set<string>();
    value.measurement_series_files.forEach((file, index) => {
      if (seriesFiles.has(file.file)) {
        context.addIssue({
          code: "custom",
          path: ["measurement_series_files", index, "file"],
          message: "A package cannot reference the same measurement-series file twice",
        });
      }
      seriesFiles.add(file.file);
    });

    const recordIds = new Set(value.records.map((record) => record.record_id));
    value.record_erasure_markers.forEach((marker, index) => {
      if (recordIds.has(marker.record_id)) {
        context.addIssue({
          code: "custom",
          path: ["record_erasure_markers", index, "record_id"],
          message: "An erased record cannot remain in current package records",
        });
      }
    });
  });

export const ManifestFile = z.strictObject({
  file: PackageRelativePath,
  sha256: Sha256,
  byte_count: z.number().int().nonnegative(),
});

export const ManifestRecordCount = z.strictObject({
  record_type: RecordType,
  current_record_count: z.number().int().nonnegative(),
  revision_count: z.number().int().nonnegative(),
  deletion_count: z.number().int().nonnegative(),
  import_key_count: z.number().int().nonnegative(),
});

export const ExportManifest = z.strictObject({
  athleticstandard_version: z.literal(ATHLETIC_STANDARD_VERSION),
  source_database_id: DatabaseId,
  database_revision: z.number().int().nonnegative(),
  exported_at: UtcTimestamp.optional(),
  files: z.array(ManifestFile),
  record_counts: z.array(ManifestRecordCount),
  erasure_marker_count: z.number().int().nonnegative(),
});

export type RecordIdT = z.infer<typeof RecordId>;
export type EvidenceReferenceT = z.infer<typeof EvidenceReference>;
export type RecordActorT = z.infer<typeof RecordActor>;
export type AuthoredRecordAuthorT = z.infer<typeof AuthoredRecordAuthor>;
export type InputTimestampReceiptT = z.infer<typeof InputTimestampReceipt>;
export type ProviderReportingPeriodT = z.infer<typeof ProviderReportingPeriod>;
export type CurrentRecordT = z.infer<typeof CurrentRecord>;
export type NonSampleRecordT = z.infer<typeof NonSampleRecord>;
export type RecordRevisionT = z.infer<typeof RecordRevision>;
export type RecordDeletionT = z.infer<typeof RecordDeletion>;
export type RecordErasureMarkerT = z.infer<typeof RecordErasureMarker>;
export type RecordImportKeyT = z.infer<typeof RecordImportKey>;
export type MeasurementSeriesFileT = z.infer<typeof MeasurementSeriesFile>;
export type AthleticStandardPackageT = z.infer<typeof AthleticStandardPackage>;
export type ExportManifestT = z.infer<typeof ExportManifest>;
