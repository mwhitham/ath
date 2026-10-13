import { describe, expect, it } from "vitest";
import {
  AgentAuthor,
  AthleticStandardPackage,
  AthleteProfileRecord,
  AuthoredRecordAuthor,
  AuthoredRecordMetadata,
  BecameKnownAtBasis,
  CurrentRecord,
  ExportManifest,
  MeasurementSeriesFile,
  PointMeasurementRecord,
  ProviderReportingPeriodType,
  ProviderScoreRecord,
  RecordId,
  RecordRevision,
  RecordType,
  SERIES_QUANTITY_UNITS,
  POINT_MEASUREMENT_UNITS,
} from "../src/v1/schema.js";

const ids = {
  database: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f01",
  source: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f02",
  sourceTwo: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f03",
  point: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f04",
  sample: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f05",
  report: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f06",
  prediction: "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f07",
} as const;

const timestampReceipt = {
  timestamp_text: "2026-08-09T06:12:00-05:00",
  utc_offset_text: "-05:00",
  utc_offset_meaning: "provider_cycle_offset" as const,
  timestamp_basis: "source_offset_timestamp" as const,
};

const athleteAuthor = { author_type: "athlete" as const };

const authorship = {
  author: {
    author_type: "agent" as const,
    agent: "ath-agent",
    model: "example-model",
    model_version: "2026-08",
  },
  authored_at: "2026-08-09T12:00:00Z",
  evidence: [],
  content_version: 1,
  status: "active" as const,
};

function sourceRecord() {
  return {
    record_id: ids.source,
    record_type: "source" as const,
    created_at: "2026-08-09T12:00:00Z",
    current_revision: 1,
    is_deleted: false,
    data: {
      source_type: "wearable" as const,
      data_writer: "WHOOP",
      data_route: "whoop_csv",
      vendor: "whoop",
    },
  };
}

function pointRecord() {
  return {
    record_id: ids.point,
    record_type: "point_measurement" as const,
    created_at: "2026-08-09T12:00:00Z",
    current_revision: 1,
    is_deleted: false,
    data: {
      source_id: ids.source,
      received_at: "2026-08-09T12:00:00Z",
      received_at_basis: "import_time" as const,
      quantity: "hrv_rmssd" as const,
      recorded_at: "2026-08-09T11:12:00Z",
      input_timestamp: timestampReceipt,
      value: {
        standard_value: 62,
        standard_unit: "ms",
        input_value: 62,
        input_unit: "ms",
      },
    },
  };
}

function sampleRecord() {
  return {
    record_id: ids.sample,
    record_type: "measurement_sample" as const,
    created_at: "2026-08-09T12:00:00Z",
    current_revision: 1,
    is_deleted: false,
    data: {
      source_id: ids.source,
      received_at: "2026-08-09T12:00:00Z",
      received_at_basis: "import_time" as const,
      quantity: "heart_rate" as const,
      recorded_at: "2026-08-09T11:12:00Z",
      input_timestamp: timestampReceipt,
      value: {
        standard_value: 142,
        standard_unit: "bpm",
        input_value: 142,
        input_unit: "count/min",
      },
    },
  };
}

function revisionFor(
  record: ReturnType<typeof sourceRecord> | ReturnType<typeof pointRecord> | ReturnType<typeof sampleRecord>,
) {
  return {
    record_id: record.record_id,
    record_type: record.record_type,
    revision: 1,
    previous_revision: null,
    became_known_at: record.created_at,
    became_known_at_basis: "provider_import" as const,
    changed_by: { author_type: "tool" as const, tool: "ath", tool_version: "1.0.0" },
    data: record.data,
  };
}

describe("v1 record IDs and types", () => {
  it("accepts lowercase UUIDv7 and rejects UUIDv4", () => {
    expect(RecordId.safeParse(ids.point).success).toBe(true);
    expect(RecordId.safeParse("550e8400-e29b-41d4-a716-446655440000").success).toBe(false);
  });

  it("lists every approved stable record type and no legacy-only type", () => {
    expect(RecordType.options).toEqual([
      "athlete_profile",
      "source",
      "source_device",
      "point_measurement",
      "measurement_sample",
      "sleep_session",
      "workout_session",
      "benchmark_result",
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
    expect(RecordType.options).not.toContain("legacy_sample_group");
  });
});

describe("v1 two-tier wall", () => {
  it("requires a source on a measured record", () => {
    const record = pointRecord();
    const data = { ...record.data } as Record<string, unknown>;
    delete data.source_id;
    expect(PointMeasurementRecord.safeParse({ ...record, data }).success).toBe(false);
  });

  it("rejects a source on an athlete report", () => {
    const report = {
      record_id: ids.report,
      record_type: "athlete_report",
      created_at: "2026-08-09T12:00:00Z",
      current_revision: 1,
      is_deleted: false,
      data: {
        report_type: "soreness",
        reported_at: "2026-08-09T12:00:00Z",
        input_reported_at: timestampReceipt,
        reported_via: "text",
        rating: 3,
        rating_scale: "1-5",
        source_id: ids.source,
      },
    };
    expect(CurrentRecord.safeParse(report).success).toBe(false);
  });

  it("does not let an athlete note appear source-backed on a measured record", () => {
    const record = pointRecord();
    expect(
      PointMeasurementRecord.safeParse({
        ...record,
        data: { ...record.data, note: "I felt exhausted" },
      }).success,
    ).toBe(false);
  });

  it("keeps SDNN and RMSSD as separate quantities", () => {
    expect(POINT_MEASUREMENT_UNITS.hrv_sdnn).toBe("ms");
    expect(POINT_MEASUREMENT_UNITS.hrv_rmssd).toBe("ms");
    expect(Object.keys(POINT_MEASUREMENT_UNITS)).toContain("hrv_sdnn");
    expect(Object.keys(POINT_MEASUREMENT_UNITS)).toContain("hrv_rmssd");
    expect(SERIES_QUANTITY_UNITS.hrv_sdnn).toBe("ms");
  });
});

describe("v1 authorship", () => {
  it("requires an agent and model for an agent author", () => {
    expect(
      AgentAuthor.safeParse({
        author_type: "agent",
        agent: "ath-agent",
      }).success,
    ).toBe(false);
  });

  it("does not accept the removed unknown author type", () => {
    expect(AuthoredRecordAuthor.safeParse({ author_type: "unknown" }).success).toBe(false);
  });

  it("requires authored records to carry evidence, version, and status", () => {
    const withoutEvidence = { ...authorship } as Record<string, unknown>;
    delete withoutEvidence.evidence;
    expect(AuthoredRecordMetadata.safeParse(withoutEvidence).success).toBe(false);
  });
});

describe("v1 time and provider periods", () => {
  it("keeps every approved provider period meaning distinct", () => {
    expect(ProviderReportingPeriodType.options).toEqual([
      "whoop_physiological_cycle",
      "oura_activity_day",
      "oura_sleep_day",
      "oura_calendar_day",
      "oura_day_unspecified",
      "apple_activity_summary_day",
    ]);
  });

  it("accepts a date-only provider score without inventing a timestamp", () => {
    const record = {
      record_id: ids.point,
      record_type: "provider_score",
      created_at: "2026-08-09T12:00:00Z",
      current_revision: 1,
      is_deleted: false,
      data: {
        source_id: ids.source,
        received_at: "2026-08-09T12:00:00Z",
        received_at_basis: "import_time",
        provider_reporting_periods: [
          {
            period_type: "oura_activity_day",
            provider_date: "2026-08-08",
          },
        ],
        metric: "activity_score",
        value: {
          standard_value: 83,
          standard_unit: "score",
          input_value: 83,
          input_unit: "score",
        },
        scale: "0-100",
        input_timestamp: {
          timestamp_missing_reason: "not_provided_by_source",
          utc_offset_missing_reason: "not_provided_by_source",
        },
      },
    };
    const parsed = ProviderScoreRecord.safeParse(record);
    expect(parsed.success).toBe(true);
    if (parsed.success) {
      expect(parsed.data.data.recorded_at).toBeUndefined();
    }
  });

  it("contains no legacy time assumption basis", () => {
    expect(BecameKnownAtBasis.options).toEqual([
      "local_write",
      "provider_import",
      "package_restore",
      "package_merge",
    ]);
  });

  it("requires one source value or one reason that it is missing", () => {
    const record = pointRecord();
    const value = { ...record.data.value } as Record<string, unknown>;
    delete value.input_value;
    expect(
      PointMeasurementRecord.safeParse({
        ...record,
        data: { ...record.data, value },
      }).success,
    ).toBe(false);

    expect(
      PointMeasurementRecord.safeParse({
        ...record,
        data: {
          ...record.data,
          value: {
            ...record.data.value,
            input_value_missing_reason: "not_provided_by_source",
          },
        },
      }).success,
    ).toBe(false);
  });

  it("rejects normalized timestamps that are not UTC", () => {
    const record = pointRecord();
    record.data.recorded_at = "2026-08-09T06:12:00-05:00";
    expect(PointMeasurementRecord.safeParse(record).success).toBe(false);
  });

  it("validates saved calendar timezones as IANA names", () => {
    const record = {
      record_id: ids.report,
      record_type: "athlete_profile",
      created_at: "2026-08-09T12:00:00Z",
      current_revision: 1,
      is_deleted: false,
      data: { default_time_zone: "America/Chicago" },
    };
    expect(AthleteProfileRecord.safeParse(record).success).toBe(true);
    expect(
      AthleteProfileRecord.safeParse({
        ...record,
        data: { default_time_zone: "Chicago time" },
      }).success,
    ).toBe(false);
  });
});

describe("v1 revisions and portable package", () => {
  it("requires an exact prior revision link", () => {
    const revision = {
      record_id: ids.source,
      record_type: "source",
      revision: 2,
      previous_revision: 1,
      became_known_at: "2026-08-10T12:00:00Z",
      became_known_at_basis: "provider_import",
      changed_by: athleteAuthor,
      data: sourceRecord().data,
    };
    expect(RecordRevision.safeParse(revision).success).toBe(true);
    expect(RecordRevision.safeParse({ ...revision, previous_revision: null }).success).toBe(false);
  });

  it("refuses prerelease package versions", () => {
    const packageDocument = {
      athleticstandard_version: "0.4.3",
      database_id: ids.database,
      database_revision: 0,
      records: [],
      record_revisions: [],
      record_deletions: [],
      record_erasure_markers: [],
      record_import_keys: [],
      measurement_series_files: [],
    };
    expect(AthleticStandardPackage.safeParse(packageDocument).success).toBe(false);
  });

  it("refuses a package that omits current record history", () => {
    const packageDocument = {
      athleticstandard_version: "1.0.0",
      database_id: ids.database,
      database_revision: 1,
      records: [sourceRecord()],
      record_revisions: [],
      record_deletions: [],
      record_erasure_markers: [],
      record_import_keys: [],
      measurement_series_files: [],
    };
    expect(AthleticStandardPackage.safeParse(packageDocument).success).toBe(false);
  });

  it("keeps provider import keys and value-free erasure markers", () => {
    const packageDocument = {
      athleticstandard_version: "1.0.0",
      database_id: ids.database,
      database_revision: 3,
      records: [sourceRecord(), pointRecord()],
      record_revisions: [revisionFor(sourceRecord()), revisionFor(pointRecord())],
      record_deletions: [],
      record_erasure_markers: [
        {
          record_id: ids.report,
          erased_at: "2026-08-10T12:00:00Z",
          reason_code: "athlete_request",
        },
      ],
      record_import_keys: [
        {
          record_id: ids.point,
          match_key_method: "whoop-csv-point-v1",
          match_key: "hrv|2026-08-09T06:12:00-05:00",
          imported_at: "2026-08-09T12:00:00Z",
        },
      ],
      measurement_series_files: [],
    };
    expect(AthleticStandardPackage.safeParse(packageDocument).success).toBe(true);
    expect(
      AthleticStandardPackage.safeParse({
        ...packageDocument,
        record_erasure_markers: [
          {
            record_id: ids.report,
            erased_at: "2026-08-10T12:00:00Z",
            reason: "free-text reason",
          },
        ],
      }).success,
    ).toBe(false);
  });

  it("keeps dense samples in generated measurement-series files", () => {
    const series = {
      athleticstandard_version: "1.0.0",
      source_id: ids.source,
      quantity: "heart_rate",
      start_at: "2026-08-09T11:12:00Z",
      end_at: "2026-08-09T11:12:00Z",
      records: [sampleRecord()],
      record_revisions: [revisionFor(sampleRecord())],
      record_deletions: [],
      record_import_keys: [],
    };
    expect(MeasurementSeriesFile.safeParse(series).success).toBe(true);
    expect(
      MeasurementSeriesFile.safeParse({
        ...series,
        source_id: ids.sourceTwo,
      }).success,
    ).toBe(false);

    const packageWithInlineSample = {
      athleticstandard_version: "1.0.0",
      database_id: ids.database,
      database_revision: 1,
      records: [sampleRecord()],
      record_revisions: [],
      record_deletions: [],
      record_erasure_markers: [],
      record_import_keys: [],
      measurement_series_files: [],
    };
    expect(AthleticStandardPackage.safeParse(packageWithInlineSample).success).toBe(false);
  });
});

describe("v1 export manifest", () => {
  it("does not add an export time unless the caller supplies one", () => {
    const manifest = {
      athleticstandard_version: "1.0.0",
      source_database_id: ids.database,
      database_revision: 3,
      files: [
        {
          file: "athlete.ath.json",
          sha256: "a".repeat(64),
          byte_count: 123,
        },
      ],
      record_counts: [],
      erasure_marker_count: 0,
    };
    const parsed = ExportManifest.parse(manifest);
    expect(parsed.exported_at).toBeUndefined();
  });

  it("rejects package traversal paths", () => {
    const manifest = {
      athleticstandard_version: "1.0.0",
      source_database_id: ids.database,
      database_revision: 3,
      files: [
        {
          file: "../athlete.ath.json",
          sha256: "a".repeat(64),
          byte_count: 123,
        },
      ],
      record_counts: [],
      erasure_marker_count: 0,
    };
    expect(ExportManifest.safeParse(manifest).success).toBe(false);
  });
});
