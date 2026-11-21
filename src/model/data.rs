//! The data object of each of the 24 record types.
//!
//! Field order matches the published format. Stored JSON lists fields in this
//! order, so changing it would change stored bytes.

use super::common::*;
use super::schema::{
    custom, integer, max, min, nonnegative, nonnegative_int, positive, positive_int, Issue, Schema, SchemaRef,
};
use crate::json::Value;
use crate::{model_enum, model_object};

fn has(value: &Value, key: &str) -> bool {
    value.get(key).is_some()
}

fn reporting_period_or_instant(value: &Value) -> bool {
    has(value, "recorded_at")
        || value.get("provider_reporting_periods").and_then(Value::as_array).is_some_and(|periods| !periods.is_empty())
}

// ---- Athlete and sources ------------------------------------------------

model_enum! {
    pub enum Sex {
        Male = "male",
        Female = "female",
    }
}

model_enum! {
    pub enum DisplayUnits {
        Metric = "metric",
        Imperial = "imperial",
    }
}

fn birth_year() -> SchemaRef {
    static SCHEMA: std::sync::LazyLock<Schema> =
        std::sync::LazyLock::new(|| integer().check(min(1900.0)).check(max(2100.0)));
    &SCHEMA
}

model_object! {
    pub struct AthleteProfileData {
        name?: String,
        birth_year?: u32 = birth_year(),
        sex?: Sex,
        display_units?: DisplayUnits,
        default_time_zone?: String = iana_time_zone(),
    }
}

model_enum! {
    pub enum SourceType {
        Wearable = "wearable",
        ExportFile = "export_file",
        Connector = "connector",
        Manual = "manual",
    }
}

model_object! {
    /// Where measured records came from: who wrote the data and by which route.
    pub struct SourceData {
        source_type: SourceType,
        data_writer: String,
        data_route: String,
        vendor?: String,
        sensor_type?: String,
        detail?: String,
        supplied_fields?: SuppliedFields = super::schema::supplied_fields(),
    }
}

model_object! {
    pub struct SourceDeviceData {
        source_id: String = record_id(),
        name?: String,
        manufacturer?: String,
        model?: String,
        hardware?: String,
        serial_number?: String,
        coverage_start_date?: String = calendar_date(),
        coverage_end_date?: String = calendar_date(),
        accepted_record_count?: u64 = nonnegative_int(),
    }
}

// ---- Quantities ---------------------------------------------------------

model_enum! {
    /// Single-value measurements. Each uses one standard unit.
    pub enum PointMeasurementType {
        HrvRmssd = "hrv_rmssd",
        HrvSdnn = "hrv_sdnn",
        RestingHeartRate = "resting_heart_rate",
        BodyWeight = "body_weight",
        RespiratoryRate = "respiratory_rate",
        Vo2Max = "vo2_max",
        HrRecovery = "hr_recovery",
        WalkingHeartRate = "walking_heart_rate",
        OxygenSaturation = "oxygen_saturation",
        BodyTemperature = "body_temperature",
        SkinTemperature = "skin_temperature",
        WristTemperatureSleeping = "wrist_temperature_sleeping",
        TemperatureDeviation = "temperature_deviation",
        Height = "height",
        LeanBodyMass = "lean_body_mass",
        BodyFatPercentage = "body_fat_percentage",
        BloodPressureSystolic = "blood_pressure_systolic",
        BloodPressureDiastolic = "blood_pressure_diastolic",
    }
}

impl PointMeasurementType {
    pub fn unit(self) -> &'static str {
        point_measurement_unit(self.as_str()).expect("every point quantity has a unit")
    }
}

/// The standard unit of a point measurement quantity.
pub fn point_measurement_unit(quantity: &str) -> Option<&'static str> {
    Some(match quantity {
        "hrv_rmssd" | "hrv_sdnn" => "ms",
        "resting_heart_rate" | "hr_recovery" | "walking_heart_rate" => "bpm",
        "body_weight" | "lean_body_mass" => "kg",
        "respiratory_rate" => "brpm",
        "vo2_max" => "ml/kg/min",
        "oxygen_saturation" | "body_fat_percentage" => "%",
        "body_temperature" | "skin_temperature" | "wrist_temperature_sleeping" | "temperature_deviation" => "°C",
        "height" => "cm",
        "blood_pressure_systolic" | "blood_pressure_diastolic" => "mmHg",
        _ => return None,
    })
}

model_enum! {
    /// Time-series sample quantities. Each uses one standard unit.
    pub enum SeriesQuantity {
        HeartRate = "heart_rate",
        HrvBeats = "hrv_beats",
        EcgBeats = "ecg_beats",
        HrvSdnn = "hrv_sdnn",
        RespiratoryRate = "respiratory_rate",
        OxygenSaturation = "oxygen_saturation",
        Steps = "steps",
        ActiveEnergy = "active_energy",
        DistanceWalkingRunning = "distance_walking_running",
        DistanceCycling = "distance_cycling",
        DistanceSwimming = "distance_swimming",
        RunningSpeed = "running_speed",
        RunningPower = "running_power",
        RunningStrideLength = "running_stride_length",
        RunningVerticalOscillation = "running_vertical_oscillation",
        RunningGroundContactTime = "running_ground_contact_time",
        PhysicalEffort = "physical_effort",
        BasalEnergy = "basal_energy",
        ExerciseTime = "exercise_time",
        FlightsClimbed = "flights_climbed",
        WalkingSpeed = "walking_speed",
        WalkingStepLength = "walking_step_length",
        WalkingAsymmetryPercentage = "walking_asymmetry_percentage",
        TimeInDaylight = "time_in_daylight",
    }
}

impl SeriesQuantity {
    pub fn unit(self) -> &'static str {
        series_quantity_unit(self.as_str()).expect("every series quantity has a unit")
    }
}

/// The standard unit of a series quantity.
pub fn series_quantity_unit(quantity: &str) -> Option<&'static str> {
    Some(match quantity {
        "heart_rate" => "bpm",
        "hrv_beats" | "ecg_beats" | "hrv_sdnn" | "running_ground_contact_time" => "ms",
        "respiratory_rate" => "brpm",
        "oxygen_saturation" | "walking_asymmetry_percentage" => "%",
        "steps" | "flights_climbed" => "count",
        "active_energy" | "basal_energy" => "kcal",
        "distance_walking_running"
        | "distance_cycling"
        | "distance_swimming"
        | "running_stride_length"
        | "walking_step_length" => "m",
        "running_speed" | "walking_speed" => "m/s",
        "running_power" => "W",
        "running_vertical_oscillation" => "cm",
        "physical_effort" => "MET",
        "exercise_time" | "time_in_daylight" => "min",
        _ => return None,
    })
}

// ---- Measured records ---------------------------------------------------

model_object! {
    pub struct PointMeasurementCalculation {
        from_quantity: String,
        method: String,
        calculation_version: String,
        window_s?: f64 = positive(),
        usable_sample_count?: u64 = positive_int(),
        dropped_sample_count?: u64 = nonnegative_int(),
        signal_quality?: f64 = nonnegative(),
        input_records: Vec<EvidenceReference>,
    }
}

model_object! {
    pub struct PointMeasurementData {
        source_id: String = record_id(),
        provider_record_id?: String,
        provider_reporting_periods?: Vec<ProviderReportingPeriod>,
        received_at: String = utc_timestamp(),
        received_at_basis: ReceivedAtBasis,
        supplied_fields?: SuppliedFields = super::schema::supplied_fields(),
        field_basis?: FieldBasis,
        quantity: PointMeasurementType,
        recorded_at?: String = utc_timestamp(),
        input_timestamp: InputTimestampReceipt,
        value: SourceValue,
        calculations?: Vec<PointMeasurementCalculation>,
    }
    refine(refine_point_measurement)
}

fn refine_point_measurement(value: &Value, issues: &mut Vec<Issue>) {
    let quantity = value.get("quantity").and_then(Value::as_str).unwrap_or_default();
    let Some(expected) = point_measurement_unit(quantity) else {
        return;
    };
    let standard = value.get("value");
    if standard.and_then(|v| v.get("standard_unit")).and_then(Value::as_str) != Some(expected) {
        custom(issues, &["value", "standard_unit"], format!("{quantity} uses {expected}"));
    }
    let number = standard.and_then(|v| v.get("standard_value")).and_then(Value::as_f64).unwrap_or(f64::NAN);
    // `standard_value <= 0` is false for NaN, so NaN produces no issue.
    if quantity != "temperature_deviation" && number <= 0.0 {
        custom(issues, &["value", "standard_value"], format!("{quantity} must be positive"));
    }
}

model_object! {
    pub struct MeasurementSampleData {
        source_id: String = record_id(),
        provider_record_id?: String,
        provider_reporting_periods?: Vec<ProviderReportingPeriod>,
        received_at: String = utc_timestamp(),
        received_at_basis: ReceivedAtBasis,
        supplied_fields?: SuppliedFields = super::schema::supplied_fields(),
        field_basis?: FieldBasis,
        quantity: SeriesQuantity,
        recorded_at: String = utc_timestamp(),
        input_timestamp: InputTimestampReceipt,
        value: SourceValue,
        duration_ms?: u64 = nonnegative_int(),
        timestamp_base_text?: String,
        timestamp_reconstruction_method?: String,
    }
    refine(refine_measurement_sample)
}

fn refine_measurement_sample(value: &Value, issues: &mut Vec<Issue>) {
    let quantity = value.get("quantity").and_then(Value::as_str).unwrap_or_default();
    if let Some(expected) = series_quantity_unit(quantity) {
        let unit = value.get("value").and_then(|v| v.get("standard_unit")).and_then(Value::as_str);
        if unit != Some(expected) {
            custom(issues, &["value", "standard_unit"], format!("{quantity} uses {expected}"));
        }
    }
    let reconstructed = value.get("input_timestamp").and_then(|v| v.get("timestamp_basis")).and_then(Value::as_str)
        == Some("reconstructed_from_series_start");
    if reconstructed != (has(value, "timestamp_base_text") && has(value, "timestamp_reconstruction_method")) {
        custom(
            issues,
            &["timestamp_reconstruction_method"],
            "A reconstructed timestamp needs its base and method, and other timestamps cannot claim them",
        );
    }
}

fn stage_minutes() -> SchemaRef {
    sleep_stage_minutes()
}

model_object! {
    pub struct SleepSessionData {
        source_id: String = record_id(),
        provider_record_id?: String,
        provider_reporting_periods?: Vec<ProviderReportingPeriod>,
        received_at: String = utc_timestamp(),
        received_at_basis: ReceivedAtBasis,
        supplied_fields?: SuppliedFields = super::schema::supplied_fields(),
        field_basis?: FieldBasis,
        start_at: String = utc_timestamp(),
        end_at: String = utc_timestamp(),
        input_start_timestamp: InputTimestampReceipt,
        input_end_timestamp: InputTimestampReceipt,
        source_reported_summaries: Vec<SourceReportedSummary>,
        asleep_seconds?: f64 = nonnegative(),
        in_bed_seconds?: f64 = nonnegative(),
        interruptions?: u64 = nonnegative_int(),
        stage_minutes?: StageMinutes = stage_minutes(),
    }
    refine(refine_sleep_session)
}

fn refine_sleep_session(value: &Value, issues: &mut Vec<Issue>) {
    if !end_not_before_start(value, "start_at", "end_at") {
        custom(issues, &["end_at"], "Sleep session end cannot precede its start");
    }
}

model_object! {
    pub struct HeartRateSample {
        offset_ms: u64 = nonnegative_int(),
        bpm: f64 = positive(),
    }
}

model_enum! {
    /// What a strength set's `weight` measures (D239).
    pub enum LoadBasis {
        /// The whole load moved, such as a loaded barbell.
        Total = "total",
        /// The load of each implement, such as one of two dumbbells.
        PerImplement = "per_implement",
    }
}

model_object! {
    pub struct StrengthSet {
        reps: f64 = positive(),
        weight: f64 = positive(),
        weight_unit: String,
        /// The exercise, such as `back squat`.
        movement?: String,
        load_basis?: LoadBasis,
        /// How many implements carried `weight` each. Only with `per_implement`.
        implement_count?: u64 = positive_int(),
    }
    refine(refine_strength_set)
}

fn refine_strength_set(value: &Value, issues: &mut Vec<Issue>) {
    let per_implement = value.get("load_basis").and_then(Value::as_str) == Some("per_implement");
    if per_implement && !has(value, "implement_count") {
        custom(issues, &["implement_count"], "A per-implement load needs implement_count");
    }
    if !per_implement && has(value, "implement_count") {
        custom(issues, &["implement_count"], "implement_count is used only with a per_implement load_basis");
    }
}

fn latitude() -> SchemaRef {
    static SCHEMA: std::sync::LazyLock<Schema> =
        std::sync::LazyLock::new(|| super::schema::number().check(min(-90.0)).check(max(90.0)));
    &SCHEMA
}

fn longitude() -> SchemaRef {
    static SCHEMA: std::sync::LazyLock<Schema> =
        std::sync::LazyLock::new(|| super::schema::number().check(min(-180.0)).check(max(180.0)));
    &SCHEMA
}

model_object! {
    pub struct RoutePoint {
        recorded_at: String = utc_timestamp(),
        latitude_deg: f64 = latitude(),
        longitude_deg: f64 = longitude(),
        /// The time the source wrote, including its UTC offset. `recorded_at` is that instant in UTC.
        input_timestamp?: InputTimestampReceipt,
        supplied_fields?: SuppliedFields = super::schema::supplied_fields(),
    }
}

model_object! {
    pub struct WorkoutSessionSegment {
        label: String,
        start_offset_ms?: u64 = nonnegative_int(),
        end_offset_ms?: u64 = nonnegative_int(),
        source_reported_summaries: Vec<SourceReportedSummary>,
    }
}

model_object! {
    pub struct WorkoutSessionData {
        source_id: String = record_id(),
        provider_record_id?: String,
        provider_reporting_periods?: Vec<ProviderReportingPeriod>,
        received_at: String = utc_timestamp(),
        received_at_basis: ReceivedAtBasis,
        supplied_fields?: SuppliedFields = super::schema::supplied_fields(),
        field_basis?: FieldBasis,
        start_at: String = utc_timestamp(),
        end_at: String = utc_timestamp(),
        input_start_timestamp: InputTimestampReceipt,
        input_end_timestamp: InputTimestampReceipt,
        activity_type?: String,
        name?: String,
        description?: String,
        text_result?: String,
        maximum_heart_rate_bpm?: f64 = positive(),
        resting_heart_rate_bpm?: f64 = positive(),
        heart_rate_samples?: Vec<HeartRateSample>,
        sets?: Vec<StrengthSet>,
        route?: Vec<RoutePoint>,
        source_reported_summaries: Vec<SourceReportedSummary>,
        segments?: Vec<WorkoutSessionSegment>,
    }
    refine(refine_workout_session)
}

fn refine_workout_session(value: &Value, issues: &mut Vec<Issue>) {
    if !end_not_before_start(value, "start_at", "end_at") {
        custom(issues, &["end_at"], "Workout session end cannot precede its start");
    }
}

model_object! {
    pub struct BenchmarkResultSegment {
        label: String,
        score?: Score,
    }
}

model_enum! {
    pub enum Scaling {
        Rx = "rx",
        Scaled = "scaled",
    }
}

model_object! {
    pub struct BenchmarkResultData {
        source_id: String = record_id(),
        provider_record_id?: String,
        provider_reporting_periods?: Vec<ProviderReportingPeriod>,
        received_at: String = utc_timestamp(),
        received_at_basis: ReceivedAtBasis,
        supplied_fields?: SuppliedFields = super::schema::supplied_fields(),
        field_basis?: FieldBasis,
        benchmark_definition_id: String = record_id(),
        recorded_at: String = utc_timestamp(),
        input_timestamp: InputTimestampReceipt,
        score: Score,
        input_score_text?: String,
        scaling?: Scaling,
        workout_session_id?: String = record_id(),
        segments?: Vec<BenchmarkResultSegment>,
    }
}

model_object! {
    pub struct ProviderSummaryData {
        source_id: String = record_id(),
        provider_record_id?: String,
        provider_reporting_periods?: Vec<ProviderReportingPeriod>,
        received_at: String = utc_timestamp(),
        received_at_basis: ReceivedAtBasis,
        supplied_fields?: SuppliedFields = super::schema::supplied_fields(),
        field_basis?: FieldBasis,
        summary_type: String,
        value: SourceValue,
        recorded_at?: String = utc_timestamp(),
        input_timestamp: InputTimestampReceipt,
    }
    refine(refine_provider_summary)
}

fn refine_provider_summary(value: &Value, issues: &mut Vec<Issue>) {
    if !reporting_period_or_instant(value) {
        custom(
            issues,
            &["provider_reporting_periods"],
            "A provider summary needs an instant or a provider reporting period",
        );
    }
}

model_object! {
    pub struct ProviderScoreData {
        source_id: String = record_id(),
        provider_record_id?: String,
        provider_reporting_periods?: Vec<ProviderReportingPeriod>,
        received_at: String = utc_timestamp(),
        received_at_basis: ReceivedAtBasis,
        supplied_fields?: SuppliedFields = super::schema::supplied_fields(),
        field_basis?: FieldBasis,
        metric: String,
        value: SourceValue,
        scale: String,
        recorded_at?: String = utc_timestamp(),
        input_timestamp: InputTimestampReceipt,
    }
    refine(refine_provider_score)
}

fn refine_provider_score(value: &Value, issues: &mut Vec<Issue>) {
    if !reporting_period_or_instant(value) {
        custom(
            issues,
            &["provider_reporting_periods"],
            "A provider score needs an instant or a provider reporting period",
        );
    }
}

// ---- Athlete reports ----------------------------------------------------

model_enum! {
    pub enum AthleteReportType {
        SleepQuality = "sleep_quality",
        Soreness = "soreness",
        Stress = "stress",
        Mood = "mood",
        Energy = "energy",
        Note = "note",
        /// The athlete's rating of a whole session, such as a session RPE (D238).
        SessionEffort = "session_effort",
    }
}

model_enum! {
    pub enum ReportedVia {
        Text = "text",
        Voice = "voice",
        Photo = "photo",
    }
}

model_object! {
    pub struct AthleteReportData {
        report_type: AthleteReportType,
        reported_at: String = utc_timestamp(),
        input_reported_at: InputTimestampReceipt,
        reported_via: ReportedVia,
        rating?: f64,
        rating_scale?: String,
        body_region?: String,
        note?: String,
        duration_seconds?: f64 = positive(),
        attachment_ids?: Vec<String> = record_id_array(),
        /// The workout this report describes (D238).
        workout_session_id?: String = record_id(),
    }
    refine(refine_athlete_report)
}

fn refine_athlete_report(value: &Value, issues: &mut Vec<Issue>) {
    if has(value, "rating") && !has(value, "rating_scale") {
        custom(issues, &["rating_scale"], "A rating needs its scale");
    }
    if value.get("report_type").and_then(Value::as_str) == Some("session_effort") && !has(value, "rating") {
        custom(issues, &["rating"], "A session effort report needs a rating");
    }
}

model_object! {
    pub struct ReportedNutrient {
        nutrient_name: String,
        amount: f64,
        unit: String,
    }
}

model_object! {
    pub struct ReportedNutritionItem {
        food_description: String,
        amount?: f64,
        amount_unit?: String,
        nutrients?: Vec<ReportedNutrient>,
    }
}

model_object! {
    pub struct NutritionEntryData {
        reported_at: String = utc_timestamp(),
        input_reported_at: InputTimestampReceipt,
        reported_via: ReportedVia,
        athlete_description: String,
        reported_items: Vec<ReportedNutritionItem>,
        attachment_ids?: Vec<String> = record_id_array(),
    }
}

model_object! {
    pub struct AttachmentData {
        file_name: String,
        media_type: String,
        byte_count: u64 = nonnegative_int(),
        sha256: String = sha256(),
        description?: String,
    }
}

// ---- Authored records ---------------------------------------------------

model_enum! {
    pub enum BenchmarkType {
        NamedWod = "named_wod",
        Run = "run",
        Hyrox = "hyrox",
        Lift = "lift",
        Custom = "custom",
    }
}

model_enum! {
    pub enum ScoreType {
        Time = "time",
        Reps = "reps",
        Load = "load",
    }
}

model_object! {
    pub struct BenchmarkDefinitionData {
        authorship: AuthoredRecordMetadata,
        benchmark_key: String = benchmark_key(),
        benchmark_type: BenchmarkType,
        score_type: ScoreType,
        definition: String,
        tags?: Vec<String> = text_array(),
    }
}

model_object! {
    pub struct ScoreRange {
        low: Score,
        high: Score,
    }
}

model_object! {
    pub struct PredictionData {
        authorship: AuthoredRecordMetadata,
        benchmark_definition_id: String = record_id(),
        predicted_score: Score,
        predicted_range?: ScoreRange,
        confidence: Confidence,
        reasoning: String,
    }
}

model_object! {
    pub struct PredictionResultLinkData {
        prediction: EvidenceReference,
        benchmark_result: EvidenceReference,
        created_by: RecordActor,
        linked_at: String = utc_timestamp(),
        content_version: u64 = positive_int(),
        status: AuthoredRecordStatus,
    }
}

model_object! {
    pub struct PredictionGradeData {
        authorship: AuthoredRecordMetadata,
        prediction_result_link: EvidenceReference,
        signed_error: f64,
        absolute_error_percentage: f64 = nonnegative(),
        result_in_predicted_range: bool,
        predicted_score: Score,
        actual_score: Score,
    }
}

model_object! {
    pub struct PredictionErrorCause {
        evidence: EvidenceReference,
        explanation: String,
    }
}

model_enum! {
    pub enum ErrorDirection {
        Faster = "faster",
        Slower = "slower",
        Higher = "higher",
        Lower = "lower",
    }
}

model_enum! {
    pub enum ErrorSeverity {
        Minor = "minor",
        Significant = "significant",
        Severe = "severe",
    }
}

model_object! {
    pub struct PredictionErrorAnalysisData {
        authorship: AuthoredRecordMetadata,
        prediction_grade: EvidenceReference,
        error_direction: ErrorDirection,
        error_severity: ErrorSeverity,
        causes: Vec<PredictionErrorCause>,
        unexplained: bool,
        lesson?: String,
    }
    refine(refine_error_analysis)
}

fn refine_error_analysis(value: &Value, issues: &mut Vec<Issue>) {
    let unexplained = value.get("unexplained").and_then(Value::as_bool).unwrap_or(false);
    let causes = value.get("causes").and_then(Value::as_array).map_or(0, Vec::len);
    if !unexplained && causes == 0 {
        custom(issues, &["unexplained"], "An analysis without causes must mark the error unexplained");
    }
}

model_object! {
    pub struct TrainingPlanData {
        authorship: AuthoredRecordMetadata,
        title: String,
        objective?: String,
        start_date?: String = calendar_date(),
        end_date?: String = calendar_date(),
    }
}

model_object! {
    pub struct PlannedSessionBlock {
        block_type: String,
        title: String,
        instructions?: String,
        planned_duration_s?: f64 = positive(),
        planned_distance_m?: f64 = positive(),
        planned_reps?: u64 = positive_int(),
        planned_weight_kg?: f64 = positive(),
    }
}

model_enum! {
    pub enum CompletionStatus {
        Planned = "planned",
        Completed = "completed",
        Skipped = "skipped",
        Cancelled = "cancelled",
    }
}

model_object! {
    pub struct PlannedSessionData {
        authorship: AuthoredRecordMetadata,
        training_plan_id?: String = record_id(),
        title: String,
        scheduled_date?: String = calendar_date(),
        scheduled_start_at?: String = utc_timestamp(),
        activity_type?: String,
        blocks: Vec<PlannedSessionBlock>,
        completion_status: CompletionStatus,
        completed_workout_session_id?: String = record_id(),
    }
    refine(refine_planned_session)
}

fn refine_planned_session(value: &Value, issues: &mut Vec<Issue>) {
    if !has(value, "scheduled_date") && !has(value, "scheduled_start_at") {
        custom(issues, &["scheduled_date"], "A planned session needs a date or start instant");
    }
}

model_object! {
    pub struct CoachingNoteData {
        authorship: AuthoredRecordMetadata,
        title?: String,
        note: String,
        applies_from?: String = calendar_date(),
        applies_to?: String = calendar_date(),
    }
}

model_object! {
    pub struct SavedTrainingReviewData {
        authorship: AuthoredRecordMetadata,
        period_start: String = calendar_date(),
        period_end: String = calendar_date(),
        summary: String,
        recommendations?: Vec<String> = text_array(),
    }
}

model_object! {
    pub struct GoalData {
        authorship: AuthoredRecordMetadata,
        title: String,
        description?: String,
        target_date?: String = calendar_date(),
        target_benchmark_definition_id?: String = record_id(),
        target_score?: Score,
    }
}

fn unit_interval() -> SchemaRef {
    static SCHEMA: std::sync::LazyLock<Schema> =
        std::sync::LazyLock::new(|| super::schema::number().check(min(0.0)).check(max(1.0)));
    &SCHEMA
}

model_object! {
    pub struct NutritionModelInterpretationData {
        authorship: AuthoredRecordMetadata,
        nutrition_entry_id: String = record_id(),
        confidence: f64 = unit_interval(),
        interpreted_items: Vec<ReportedNutritionItem>,
        explanation?: String,
    }
}
