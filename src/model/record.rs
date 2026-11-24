//! Records, revisions, deletions, erasure markers, and provider import keys.

use std::sync::LazyLock;

use super::common::*;
use super::data::*;
use super::extensions;
use super::schema::{
    custom, decode, from_checked_value, leak, literal, Field, Issue, Kind, PathSegment, Schema, SchemaRef, Shape,
    ValidationError,
};
use crate::json::{Map, Value};
use crate::model_object;

macro_rules! record_types {
    ($($variant:ident => $data:ident),* $(,)?) => {
        /// The data object of a record, by record type.
        #[derive(Debug, Clone, PartialEq)]
        pub enum RecordData {
            $($variant($data)),*
        }

        impl RecordData {
            pub fn record_type(&self) -> RecordType {
                match self {
                    $(RecordData::$variant(_) => RecordType::$variant),*
                }
            }

            /// The data as a JSON value with fields in published order.
            pub fn to_value(&self) -> Value {
                match self {
                    $(RecordData::$variant(data) => crate::json::to_value(data)),*
                }
            }

            /// Validate and decode a data object for a record type.
            pub fn decode(record_type: RecordType, value: &Value) -> Result<Self, ValidationError> {
                Ok(match record_type {
                    $(RecordType::$variant => RecordData::$variant(decode::<$data>(value)?)),*
                })
            }

            /// Decode a data object that already passed validation.
            fn from_checked(record_type: RecordType, value: &Value) -> Result<Self, ValidationError> {
                Ok(match record_type {
                    $(RecordType::$variant => RecordData::$variant(from_checked_value::<$data>(value)?)),*
                })
            }
        }

        /// The data shape for a record type.
        pub fn data_schema(record_type: RecordType) -> SchemaRef {
            match record_type {
                $(RecordType::$variant => <$data as Shape>::schema()),*
            }
        }
    };
}

record_types! {
    AthleteProfile => AthleteProfileData,
    Source => SourceData,
    SourceDevice => SourceDeviceData,
    PointMeasurement => PointMeasurementData,
    MeasurementSample => MeasurementSampleData,
    SleepSession => SleepSessionData,
    WorkoutSession => WorkoutSessionData,
    BenchmarkResult => BenchmarkResultData,
    ProviderSummary => ProviderSummaryData,
    ProviderScore => ProviderScoreData,
    AthleteReport => AthleteReportData,
    NutritionEntry => NutritionEntryData,
    Attachment => AttachmentData,
    BenchmarkDefinition => BenchmarkDefinitionData,
    Prediction => PredictionData,
    PredictionResultLink => PredictionResultLinkData,
    PredictionGrade => PredictionGradeData,
    PredictionErrorAnalysis => PredictionErrorAnalysisData,
    TrainingPlan => TrainingPlanData,
    PlannedSession => PlannedSessionData,
    CoachingNote => CoachingNoteData,
    SavedTrainingReview => SavedTrainingReviewData,
    Goal => GoalData,
    NutritionModelInterpretation => NutritionModelInterpretationData,
}

/// Which of the three record classes a type belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordClass {
    /// Device or provider data. Requires a source.
    Measured,
    /// What the athlete said. Cannot claim a source.
    AthleteReported,
    /// Plans, predictions, grades, and other authored work.
    Authored,
    /// Sources, devices, and the athlete profile.
    Reference,
}

impl RecordType {
    pub fn class(self) -> RecordClass {
        use RecordType::*;
        match self {
            PointMeasurement | MeasurementSample | SleepSession | WorkoutSession | BenchmarkResult
            | ProviderSummary | ProviderScore => RecordClass::Measured,
            AthleteReport | NutritionEntry | Attachment => RecordClass::AthleteReported,
            BenchmarkDefinition
            | Prediction
            | PredictionResultLink
            | PredictionGrade
            | PredictionErrorAnalysis
            | TrainingPlan
            | PlannedSession
            | CoachingNote
            | SavedTrainingReview
            | Goal
            | NutritionModelInterpretation => RecordClass::Authored,
            AthleteProfile | Source | SourceDevice => RecordClass::Reference,
        }
    }
}

/// The record types a coach may author and change (D255). A coach writes
/// advice, like an agent. A coach never writes a measured record or an
/// athlete report, so the two-tier wall holds.
pub const COACH_RECORD_TYPES: &[RecordType] =
    &[RecordType::CoachingNote, RecordType::TrainingPlan, RecordType::PlannedSession];

impl RecordType {
    /// Whether a coach may author this type or change one of its records.
    pub fn coach_may_write(self) -> bool {
        COACH_RECORD_TYPES.contains(&self)
    }
}

/// The refusal for a coach named on any other record type.
pub fn coach_record_type_message(record_type: &str) -> String {
    format!("A coach may write only coaching_note, training_plan, and planned_session records, not {record_type}")
}

/// The refusal for a deletion by a coach. A coach cannot delete history (D255).
pub const COACH_DELETION_MESSAGE: &str = "A coach cannot delete a record";

/// The refusal for an actor who cannot change a record of this type.
pub fn actor_problem(actor: &RecordActor, record_type: RecordType) -> Option<String> {
    (actor.is_coach() && !record_type.coach_may_write()).then(|| coach_record_type_message(record_type.as_str()))
}

/// Whether a JSON actor or author names a coach.
pub(crate) fn names_coach(value: Option<&Value>) -> bool {
    value.and_then(|value| value.get("author_type")).and_then(Value::as_str) == Some("coach")
}

/// The places in a record, revision, or deletion where an actor or author
/// is written: a revision's `changed_by`, a deletion's `deleted_by`, an
/// authored record's author, and a link's `created_by`.
pub(crate) const ACTOR_PATHS: &[&[&str]] =
    &[&["changed_by"], &["deleted_by"], &["data", "authorship", "author"], &["data", "created_by"]];

/// The actor paths of `value` that name a coach.
pub(crate) fn coach_paths(value: &Value) -> impl Iterator<Item = &'static [&'static str]> + '_ {
    ACTOR_PATHS
        .iter()
        .copied()
        .filter(move |path| names_coach(path.iter().try_fold(value, |current, key| current.get(*key))))
}

impl RecordData {
    /// The measured-record source, when the type has one.
    pub fn source_id(&self) -> Option<&str> {
        match self {
            RecordData::PointMeasurement(data) => Some(&data.source_id),
            RecordData::MeasurementSample(data) => Some(&data.source_id),
            RecordData::SleepSession(data) => Some(&data.source_id),
            RecordData::WorkoutSession(data) => Some(&data.source_id),
            RecordData::BenchmarkResult(data) => Some(&data.source_id),
            RecordData::ProviderSummary(data) => Some(&data.source_id),
            RecordData::ProviderScore(data) => Some(&data.source_id),
            RecordData::SourceDevice(data) => Some(&data.source_id),
            _ => None,
        }
    }

    /// The provider reporting periods, for measured types.
    pub fn provider_reporting_periods(&self) -> &[ProviderReportingPeriod] {
        let periods = match self {
            RecordData::PointMeasurement(data) => &data.provider_reporting_periods,
            RecordData::MeasurementSample(data) => &data.provider_reporting_periods,
            RecordData::SleepSession(data) => &data.provider_reporting_periods,
            RecordData::WorkoutSession(data) => &data.provider_reporting_periods,
            RecordData::BenchmarkResult(data) => &data.provider_reporting_periods,
            RecordData::ProviderSummary(data) => &data.provider_reporting_periods,
            RecordData::ProviderScore(data) => &data.provider_reporting_periods,
            _ => return &[],
        };
        periods.as_deref().unwrap_or(&[])
    }

    /// Authorship metadata, for authored types that carry it.
    pub fn authorship(&self) -> Option<&AuthoredRecordMetadata> {
        match self {
            RecordData::BenchmarkDefinition(data) => Some(&data.authorship),
            RecordData::Prediction(data) => Some(&data.authorship),
            RecordData::PredictionGrade(data) => Some(&data.authorship),
            RecordData::PredictionErrorAnalysis(data) => Some(&data.authorship),
            RecordData::TrainingPlan(data) => Some(&data.authorship),
            RecordData::PlannedSession(data) => Some(&data.authorship),
            RecordData::CoachingNote(data) => Some(&data.authorship),
            RecordData::SavedTrainingReview(data) => Some(&data.authorship),
            RecordData::Goal(data) => Some(&data.authorship),
            RecordData::NutritionModelInterpretation(data) => Some(&data.authorship),
            _ => None,
        }
    }

    /// Attachment IDs referenced by athlete-reported types.
    pub fn attachment_ids(&self) -> &[String] {
        match self {
            RecordData::AthleteReport(data) => data.attachment_ids.as_deref().unwrap_or(&[]),
            RecordData::NutritionEntry(data) => data.attachment_ids.as_deref().unwrap_or(&[]),
            _ => &[],
        }
    }

    /// The compact stored JSON text for this data.
    pub fn to_json(&self) -> String {
        crate::json::to_string(&self.to_value())
    }
}

/// Optional record extensions, validated against the published limits.
pub type Extensions = Map<String, Value>;

/// The current state of a record.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentRecord {
    pub record_id: String,
    pub created_at: String,
    pub current_revision: u64,
    pub is_deleted: bool,
    pub extensions: Option<Extensions>,
    pub data: RecordData,
}

impl CurrentRecord {
    pub fn record_type(&self) -> RecordType {
        self.data.record_type()
    }

    /// Validate and decode a current record.
    pub fn from_value(value: &Value) -> Result<Self, ValidationError> {
        current_record_schema().validate(value)?;
        Self::from_checked(value)
    }

    /// Validate a current record that must not be a measurement sample.
    pub fn from_value_non_sample(value: &Value) -> Result<Self, ValidationError> {
        non_sample_record_schema().validate(value)?;
        Self::from_checked(value)
    }

    /// Decode a value that already passed this record's published shape, for
    /// example an item of a package file whose schema checked every item.
    /// The check is not repeated.
    pub(crate) fn from_checked(value: &Value) -> Result<Self, ValidationError> {
        let record_type = record_type_of(value)?;
        Ok(CurrentRecord {
            record_id: string_field(value, "record_id"),
            created_at: string_field(value, "created_at"),
            current_revision: value["current_revision"].as_f64().unwrap_or(0.0) as u64,
            is_deleted: value["is_deleted"].as_bool().unwrap_or(false),
            extensions: value.get("extensions").and_then(Value::as_object).cloned(),
            data: RecordData::from_checked(record_type, &value["data"])?,
        })
    }

    /// The record with fields in published order.
    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("record_id".into(), Value::String(self.record_id.clone()));
        map.insert("created_at".into(), Value::String(self.created_at.clone()));
        map.insert("current_revision".into(), Value::from(self.current_revision));
        map.insert("is_deleted".into(), Value::Bool(self.is_deleted));
        if let Some(extensions) = &self.extensions {
            map.insert("extensions".into(), Value::Object(extensions.clone()));
        }
        map.insert("record_type".into(), Value::String(self.record_type().as_str().into()));
        map.insert("data".into(), self.data.to_value());
        Value::Object(map)
    }
}

/// One retained revision of a record.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordRevision {
    pub record_id: String,
    pub revision: u64,
    pub previous_revision: Option<u64>,
    pub became_known_at: String,
    pub became_known_at_basis: BecameKnownAtBasis,
    pub changed_by: RecordActor,
    pub data: RecordData,
    pub extensions: Option<Extensions>,
}

impl RecordRevision {
    pub fn record_type(&self) -> RecordType {
        self.data.record_type()
    }

    pub fn from_value(value: &Value) -> Result<Self, ValidationError> {
        record_revision_schema().validate(value)?;
        Self::from_checked(value)
    }

    pub fn from_value_non_sample(value: &Value) -> Result<Self, ValidationError> {
        non_sample_revision_schema().validate(value)?;
        Self::from_checked(value)
    }

    /// Decode a value that already passed this revision's published shape.
    /// The check is not repeated. `RecordRevision` is a plain union of 24
    /// variants, so checking it again tries every earlier variant first.
    pub(crate) fn from_checked(value: &Value) -> Result<Self, ValidationError> {
        let record_type = record_type_of(value)?;
        Ok(RecordRevision {
            record_id: string_field(value, "record_id"),
            revision: value["revision"].as_f64().unwrap_or(0.0) as u64,
            previous_revision: value["previous_revision"].as_f64().map(|number| number as u64),
            became_known_at: string_field(value, "became_known_at"),
            became_known_at_basis: from_checked_value(&value["became_known_at_basis"])?,
            changed_by: from_checked_value(&value["changed_by"])?,
            data: RecordData::from_checked(record_type, &value["data"])?,
            extensions: value.get("extensions").and_then(Value::as_object).cloned(),
        })
    }

    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("record_id".into(), Value::String(self.record_id.clone()));
        map.insert("revision".into(), Value::from(self.revision));
        map.insert("previous_revision".into(), self.previous_revision.map_or(Value::Null, Value::from));
        map.insert("became_known_at".into(), Value::String(self.became_known_at.clone()));
        map.insert("became_known_at_basis".into(), Value::String(self.became_known_at_basis.as_str().into()));
        map.insert("changed_by".into(), crate::json::to_value(&self.changed_by));
        map.insert("record_type".into(), Value::String(self.record_type().as_str().into()));
        map.insert("data".into(), self.data.to_value());
        if let Some(extensions) = &self.extensions {
            map.insert("extensions".into(), Value::Object(extensions.clone()));
        }
        Value::Object(map)
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn record_type_of(value: &Value) -> Result<RecordType, ValidationError> {
    value
        .get("record_type")
        .and_then(Value::as_str)
        .and_then(RecordType::parse)
        .ok_or_else(|| ValidationError::single(vec![PathSegment::Key("record_type".into())], "Unknown record type"))
}

model_object! {
    /// A normal deletion. The record and its history stay; the record is hidden.
    pub struct RecordDeletion {
        record_id: String = record_id(),
        record_revision: u64 = revision_number(),
        deleted_at: String = utc_timestamp(),
        deleted_by: RecordActor,
    }
    refine(refine_deletion_actor)
}

fn refine_deletion_actor(value: &Value, issues: &mut Vec<Issue>) {
    if names_coach(value.get("deleted_by")) {
        custom(issues, &["deleted_by"], COACH_DELETION_MESSAGE);
    }
}

model_object! {
    /// The value-free trace a privacy erasure leaves behind.
    pub struct RecordErasureMarker {
        record_id: String = record_id(),
        erased_at: String = utc_timestamp(),
        reason_code: ErasureReasonCode,
    }
}

model_object! {
    /// A provider matching key. It is separate from the stable record ID.
    pub struct RecordImportKey {
        record_id: String = record_id(),
        match_key_method: String,
        match_key: String,
        imported_at: String = utc_timestamp(),
    }
}

// ---- Record and revision shapes ------------------------------------------

fn refine_record_extensions(value: &Value, issues: &mut Vec<Issue>) {
    let record_type = value.get("record_type").and_then(Value::as_str).unwrap_or_default();
    if let Some(message) = extensions::source_claim_problem(record_type, value.get("extensions")) {
        custom(issues, &["extensions"], message);
    }
}

/// A coach may author or change only the types in `COACH_RECORD_TYPES`.
fn refine_coach_actor(value: &Value, issues: &mut Vec<Issue>) {
    let record_type = value.get("record_type").and_then(Value::as_str).unwrap_or_default();
    if RecordType::parse(record_type).is_some_and(RecordType::coach_may_write) {
        return;
    }
    for path in coach_paths(value) {
        custom(issues, path, coach_record_type_message(record_type));
    }
}

fn refine_previous_revision(value: &Value, issues: &mut Vec<Issue>) {
    let revision = value.get("revision").and_then(Value::as_f64);
    let previous = value.get("previous_revision");
    let ok = match (revision, previous) {
        (Some(revision), Some(Value::Null)) => revision == 1.0,
        (Some(revision), Some(previous)) => previous.as_f64() == Some(revision - 1.0) && revision != 1.0,
        _ => false,
    };
    if !ok {
        custom(
            issues,
            &["previous_revision"],
            "previous_revision must be null for revision 1 or the prior revision number",
        );
    }
}

/// The current-record shape for one record type.
pub fn current_record_variant(record_type: RecordType) -> SchemaRef {
    static VARIANTS: LazyLock<Vec<SchemaRef>> = LazyLock::new(|| {
        RecordType::ALL
            .iter()
            .map(|record_type| {
                let fields = vec![
                    Field { name: "record_id", schema: record_id(), optional: false },
                    Field { name: "created_at", schema: utc_timestamp(), optional: false },
                    Field { name: "current_revision", schema: revision_number(), optional: false },
                    Field { name: "is_deleted", schema: super::schema::boolean(), optional: false },
                    Field { name: "extensions", schema: extensions::record_extensions(), optional: true },
                    Field { name: "record_type", schema: literal(record_type.as_str()), optional: false },
                    Field { name: "data", schema: data_schema(*record_type), optional: false },
                ];
                leak(Schema::new(Kind::Object { fields }).refine(refine_record_extensions).refine(refine_coach_actor))
            })
            .collect()
    });
    VARIANTS[record_type_index(record_type)]
}

/// The revision shape for one record type.
pub fn revision_variant(record_type: RecordType) -> SchemaRef {
    static VARIANTS: LazyLock<Vec<SchemaRef>> = LazyLock::new(|| {
        RecordType::ALL
            .iter()
            .map(|record_type| {
                let fields = vec![
                    Field { name: "record_id", schema: record_id(), optional: false },
                    Field { name: "revision", schema: revision_number(), optional: false },
                    Field { name: "previous_revision", schema: previous_revision(), optional: false },
                    Field { name: "became_known_at", schema: utc_timestamp(), optional: false },
                    Field { name: "became_known_at_basis", schema: BecameKnownAtBasis::schema(), optional: false },
                    Field { name: "changed_by", schema: RecordActor::schema(), optional: false },
                    Field { name: "record_type", schema: literal(record_type.as_str()), optional: false },
                    Field { name: "data", schema: data_schema(*record_type), optional: false },
                    Field { name: "extensions", schema: extensions::record_extensions(), optional: true },
                ];
                leak(
                    Schema::new(Kind::Object { fields })
                        .refine(refine_previous_revision)
                        .refine(refine_record_extensions)
                        .refine(refine_coach_actor),
                )
            })
            .collect()
    });
    VARIANTS[record_type_index(record_type)]
}

fn record_type_index(record_type: RecordType) -> usize {
    RecordType::ALL.iter().position(|item| *item == record_type).expect("known record type")
}

fn non_sample_types() -> impl Iterator<Item = RecordType> {
    RecordType::ALL.iter().copied().filter(|record_type| *record_type != RecordType::MeasurementSample)
}

/// `CurrentRecord`: a discriminated union on `record_type`.
pub fn current_record_schema() -> SchemaRef {
    static SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
        let mut options: Vec<(&'static str, SchemaRef)> =
            non_sample_types().map(|t| (t.as_str(), current_record_variant(t))).collect();
        options.push(("measurement_sample", current_record_variant(RecordType::MeasurementSample)));
        Schema::new(Kind::Discriminated { key: "record_type", options })
    });
    &SCHEMA
}

/// `NonSampleRecord`: every current-record type except measurement samples.
pub fn non_sample_record_schema() -> SchemaRef {
    static SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
        let options = non_sample_types().map(|t| (t.as_str(), current_record_variant(t))).collect();
        Schema::new(Kind::Discriminated { key: "record_type", options })
    });
    &SCHEMA
}

/// `RecordRevision`: a plain union of every revision variant.
pub fn record_revision_schema() -> SchemaRef {
    static SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
        let mut options: Vec<SchemaRef> = non_sample_types().map(revision_variant).collect();
        options.push(revision_variant(RecordType::MeasurementSample));
        Schema::new(Kind::Union(options))
    });
    &SCHEMA
}

/// `NonSampleRecordRevision`.
pub fn non_sample_revision_schema() -> SchemaRef {
    static SCHEMA: LazyLock<Schema> =
        LazyLock::new(|| Schema::new(Kind::Union(non_sample_types().map(revision_variant).collect())));
    &SCHEMA
}

/// `MeasurementSampleRecord` and `MeasurementSampleRevision`.
pub fn measurement_sample_record_schema() -> SchemaRef {
    current_record_variant(RecordType::MeasurementSample)
}

pub fn measurement_sample_revision_schema() -> SchemaRef {
    revision_variant(RecordType::MeasurementSample)
}
