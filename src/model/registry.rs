//! Every published shape by its published export name.
//!
//! Conformance tests, schema generation, and the npm compatibility checks look
//! shapes up by the names the 2.1.0 JavaScript package exported.

use super::common::*;
use super::data::*;
use super::extensions::record_extensions;
use super::package::*;
use super::record::*;
use super::schema::{self, SchemaRef, Shape};

/// The shape exported under `name`, if there is one.
pub fn shape(name: &str) -> Option<SchemaRef> {
    if let Some(found) = record_shape(name) {
        return Some(found);
    }
    Some(match name {
        "ActivityTypeBasis" => ActivityTypeBasis::schema(),
        "AgentActor" => agent_actor(),
        "AgentAuthor" => agent_author(),
        "AthleteActor" => athlete_actor(),
        "AthleteReportType" => AthleteReportType::schema(),
        "AthleticStandardPackage" => package_schema(),
        "AuthoredRecordAuthor" => AuthoredRecordAuthor::schema(),
        "AuthoredRecordMetadata" => AuthoredRecordMetadata::schema(),
        "AuthoredRecordStatus" => AuthoredRecordStatus::schema(),
        "BecameKnownAtBasis" => BecameKnownAtBasis::schema(),
        "BenchmarkResultSegment" => BenchmarkResultSegment::schema(),
        "CalendarDate" => calendar_date(),
        "Confidence" => Confidence::schema(),
        "CurrentRecord" => current_record_schema(),
        "DatabaseId" => database_id(),
        "ErasureReasonCode" => ErasureReasonCode::schema(),
        "EvidenceReference" => EvidenceReference::schema(),
        "ExportManifest" => ExportManifest::schema(),
        "ExtensionId" => extension_id(),
        "FieldBasis" => FieldBasis::schema(),
        "HeartRateSample" => HeartRateSample::schema(),
        "InputTimestampBasis" => InputTimestampBasis::schema(),
        "InputTimestampReceipt" => InputTimestampReceipt::schema(),
        "ManifestFile" => ManifestFile::schema(),
        "ManifestRecordCount" => ManifestRecordCount::schema(),
        "MeasurementSeriesFile" => measurement_series_file_schema(),
        "MeasurementSeriesFileReference" => MeasurementSeriesFileReference::schema(),
        "MissingReason" => MissingReason::schema(),
        "NonSampleRecord" => non_sample_record_schema(),
        "NonSampleRecordRevision" => non_sample_revision_schema(),
        "PackageRelativePath" => package_relative_path(),
        "PlannedSessionBlock" => PlannedSessionBlock::schema(),
        "PointMeasurementCalculation" => PointMeasurementCalculation::schema(),
        "PointMeasurementTypeSchema" => PointMeasurementType::schema(),
        "PredictionErrorCause" => PredictionErrorCause::schema(),
        "ProviderReportingPeriod" => ProviderReportingPeriod::schema(),
        "ProviderReportingPeriodType" => ProviderReportingPeriodType::schema(),
        "ReceivedAtBasis" => ReceivedAtBasis::schema(),
        "RecordActor" => RecordActor::schema(),
        "RecordDeletion" => RecordDeletion::schema(),
        "RecordErasureMarker" => RecordErasureMarker::schema(),
        "RecordExtensions" => record_extensions(),
        "RecordFormatVersion" => RecordFormatVersion::schema(),
        "RecordId" => record_id(),
        "RecordImportKey" => RecordImportKey::schema(),
        "RecordRevision" => record_revision_schema(),
        "RecordRevisionNumber" => revision_number(),
        "RecordType" => RecordType::schema(),
        "ReportedNutrient" => ReportedNutrient::schema(),
        "ReportedNutritionItem" => ReportedNutritionItem::schema(),
        "RoutePoint" => RoutePoint::schema(),
        "Score" => Score::schema(),
        "Semver" => semver(),
        "SeriesQuantitySchema" => SeriesQuantity::schema(),
        "Sha256" => sha256(),
        "SleepStageMinutes" => sleep_stage_minutes(),
        "SourceReportedSummary" => SourceReportedSummary::schema(),
        "SourceTimeZoneMeaning" => SourceTimeZoneMeaning::schema(),
        "SourceType" => SourceType::schema(),
        "SourceValue" => SourceValue::schema(),
        "StrengthSet" => StrengthSet::schema(),
        "SuppliedFields" => schema::supplied_fields(),
        "ToolActor" => tool_actor(),
        "ToolAuthor" => tool_author(),
        "UtcOffsetMeaning" => UtcOffsetMeaning::schema(),
        "UtcTimestamp" => utc_timestamp(),
        "ValueBasis" => ValueBasis::schema(),
        "WorkoutSessionSegment" => WorkoutSessionSegment::schema(),
        _ => return None,
    })
}

/// `<Type>Data`, `<Type>Record`, and `<Type>Revision` for each record type.
fn record_shape(name: &str) -> Option<SchemaRef> {
    for record_type in RecordType::ALL {
        let pascal: String = record_type
            .as_str()
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                chars.next().map(|first| first.to_ascii_uppercase().to_string() + chars.as_str()).unwrap_or_default()
            })
            .collect();
        if let Some(suffix) = name.strip_prefix(pascal.as_str()) {
            match suffix {
                "Data" => return Some(data_schema(*record_type)),
                "Record" => return Some(current_record_variant(*record_type)),
                "Revision" => return Some(revision_variant(*record_type)),
                _ => {}
            }
        }
    }
    None
}
