//! Calculation files: the editable `ath-calculation` block in a skill file.
//!
//! A block is a list of `key: value` lines. The reader turns it into typed
//! settings that choose one reviewed arithmetic operation and its parameters.
//! It is not an expression language: an unknown operation is reported, and a
//! setting that does not read as a number keeps its documented default.
//!
//! The files themselves are compiled into the program once, in
//! `crate::assets`. `ath apply` chooses its file with
//! `assets::resolve_calculation`: `.agents/skills/<name>/SKILL.md` in the
//! working directory when it exists, else the shipped copy. `ath calculate`
//! reads its method name from the shipped copy only, as 2.1.0 did.

use std::collections::BTreeMap;

use crate::error::{Error, Result};

/// Source labels that stay in `supplied_fields` and never feed a calculation (D186).
pub const SOURCE_LABELS: &[&str] = &["supplied:intensity", "supplied:Feeling"];

/// The `method:` line of a shipped range calculation, or an empty string.
pub fn shipped_method(operation: &str) -> String {
    crate::assets::shipped_calculation(operation).map(method_line).unwrap_or_default()
}

/// The first `method:` line of a skill file, trimmed.
pub fn method_line(text: &str) -> String {
    for line in text.split('\n') {
        let Some(rest) = line.strip_prefix("method:") else {
            continue;
        };
        let value = rest.trim_start();
        let value = value.split('\r').next().unwrap_or_default().trim();
        if !value.is_empty() {
            return value.to_string();
        }
    }
    String::new()
}

/// The raw settings of an `ath-calculation` block: each key with its values
/// in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculationBlock {
    pub name: String,
    pub settings: BTreeMap<String, Vec<String>>,
}

impl CalculationBlock {
    /// The first value of a setting.
    pub fn first(&self, key: &str) -> Option<&str> {
        self.settings.get(key).and_then(|values| values.first()).map(String::as_str)
    }

    /// Every value of a setting.
    pub fn all(&self, key: &str) -> Option<&[String]> {
        self.settings.get(key).map(Vec::as_slice)
    }
}

/// Read the `ath-calculation` block of a skill file.
pub fn parse_block(text: &str) -> Result<CalculationBlock> {
    const MARKER: &str = "```ath-calculation";
    let start = text.find(MARKER).ok_or_else(|| Error::other("the skill file has no ath-calculation block"))?;
    let rest = &text[start + MARKER.len()..];
    let end = rest.find("```").ok_or_else(|| Error::other("the calculation block is not closed"))?;
    let mut settings: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in rest[..end].lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once(':').ok_or_else(|| Error::other(format!("could not read '{line}'")))?;
        let value = value.trim();
        if SOURCE_LABELS.contains(&value) {
            return Err(Error::refused(format!("a calculation cannot read {value}")));
        }
        settings.entry(key.trim().to_string()).or_default().push(value.to_string());
    }
    let name = settings
        .get("name")
        .and_then(|values| values.first())
        .cloned()
        .ok_or_else(|| Error::other("the calculation block needs a name"))?;
    Ok(CalculationBlock { name, settings })
}

/// The reviewed one-record operations and their parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum OneRecordCalculation {
    /// Seconds of heart-rate samples between fractions of the maximum.
    TimeBetweenCuts { sample_field: String, maximum_field: String, cuts: Vec<f64> },
    /// Heart rate at the last sample minus the sample a fixed time later.
    ValueAfterEnd { sample_field: String, later_offset_ms: i64 },
    /// The lowest heart rate held without change for a span.
    LowestHeld { sample_field: String, sustain_ms: i64 },
    /// Minutes weighted by the heart-rate reserve curve.
    HeartRateReserveCurve { sample_field: String, rest_field: String, maximum_field: String, factor: f64 },
    /// Minutes multiplied by an effort rating.
    Multiply { duration_field: String, effort_field: String },
    /// One numeric field.
    Field { field: String },
    /// Time asleep divided by time in bed.
    Divide { asleep_field: String, in_bed_field: String },
    /// Start time and the midpoint between start and end.
    Midpoint { start_field: String, end_field: String },
    /// Named numeric fields of an object and their total.
    SumFields { field: String },
    /// Minute-by-minute agreement of asleep windows on consecutive nights.
    NightAgreement { minimum_nights: usize },
    /// Repetitions times weight over every set.
    RepsTimesWeight { field: String },
    /// A point measurement minus the mean of the other values of its quantity.
    QuantityChange,
    /// A sleep field minus the mean of the other sessions' values.
    SleepChange { field: String },

    // ---- 2.1.2 operations (D230). The operations above keep their 2.1.1
    // arithmetic so an edited file still gives the answer it gave before.
    /// The day's value against earlier days, or a main sleep against earlier
    /// main sleeps, in a bounded window (D232, D233, D234).
    EarlierWindowChange(WindowChangeSettings),
    /// The heart-rate drop from the end of effort to a fixed time later (D235).
    DropAfterEffortEnd(RecoverySettings),
    /// The Sleep Regularity Index on sleep periods (D236).
    SleepRegularityIndex { window_days: u32, minimum_day_pairs: u32 },
    /// Repetitions times load in kilograms, with load basis (D237).
    VolumeLoad { field: String },
    /// A session-effort rating times the minutes it covers (D238).
    SessionEffortLoad { duration_field: String, effort_field: String },
    /// Banister's TRIMP with sex-specific weighting and covered time only (D240).
    BanisterTrimp(TrimpSettings),
    /// Covered seconds between fractions of the maximum (D240).
    TimeInZones { sample_field: String, maximum_field: String, cuts: Vec<f64>, max_gap_ms: i64 },
    /// The lowest mean heart rate over a sustained window (D241).
    LowestRollingMean(RollingMeanSettings),
    /// Time asleep over time in bed, checked (D242).
    SleepEfficiency { asleep_field: String, in_bed_field: String },
    /// Asleep and awake stage minutes apart, with stage shares (D242).
    SleepStageSplit { field: String },
    /// Start, midpoint, and end in the athlete's time zone (D242).
    SleepClockTimes { start_field: String, end_field: String },
    /// A HYROX result divided into runs, stations, Roxzone, and penalties (D245).
    HyroxSplits,
    /// A range calculation, which `ath apply` refuses (D243).
    Range(String),
    /// An operation this program does not have.
    Unknown(String),
}

/// Settings for `earlier_window_change`.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowChangeSettings {
    /// A session field such as `asleep_seconds`. None reads a measurement's
    /// `value.standard_value`.
    pub field: Option<String>,
    pub window_days: u32,
    pub minimum_days: u32,
    /// A sleep session shorter than this is a nap.
    pub main_sleep_hours: f64,
}

/// Settings for `drop_after_effort_end`.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoverySettings {
    pub sample_field: String,
    pub later_offset_ms: i64,
    pub tolerance_ms: i64,
    pub max_interpolation_gap_ms: i64,
    pub recovery_label: String,
}

/// Settings for `banister_trimp`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimpSettings {
    pub sample_field: String,
    pub rest_field: String,
    pub maximum_field: String,
    pub max_gap_ms: i64,
    pub male: (f64, f64),
    pub female: (f64, f64),
}

/// Settings for `lowest_rolling_mean`.
#[derive(Debug, Clone, PartialEq)]
pub struct RollingMeanSettings {
    pub sample_field: String,
    pub window_ms: i64,
    pub minimum_samples: usize,
    pub max_gap_ms: i64,
}

impl OneRecordCalculation {
    /// The operation name a result reports.
    pub fn operation(&self) -> &str {
        use OneRecordCalculation::*;
        match self {
            TimeBetweenCuts { .. } => "time_between_cuts",
            ValueAfterEnd { .. } => "value_after_end",
            LowestHeld { .. } => "lowest_held",
            HeartRateReserveCurve { .. } => "heart_rate_reserve_curve",
            Multiply { .. } => "multiply",
            Field { .. } => "field",
            Divide { .. } => "divide",
            Midpoint { .. } => "midpoint",
            SumFields { .. } => "sum_fields",
            NightAgreement { .. } => "night_agreement",
            RepsTimesWeight { .. } => "reps_times_weight",
            QuantityChange | SleepChange { .. } => "subtract_earlier_mean",
            EarlierWindowChange(_) => "earlier_window_change",
            DropAfterEffortEnd(_) => "drop_after_effort_end",
            SleepRegularityIndex { .. } => "sleep_regularity_index",
            VolumeLoad { .. } => "volume_load",
            SessionEffortLoad { .. } => "session_effort_load",
            BanisterTrimp(_) => "banister_trimp",
            TimeInZones { .. } => "time_in_zones",
            LowestRollingMean(_) => "lowest_rolling_mean",
            SleepEfficiency { .. } => "sleep_efficiency",
            SleepStageSplit { .. } => "sleep_stage_split",
            SleepClockTimes { .. } => "sleep_clock_times",
            HyroxSplits => "hyrox_splits",
            Range(name) | Unknown(name) => name,
        }
    }

    /// For an operation replaced in 2.1.2, what replaced it and why (D230).
    pub fn method_notice(&self) -> Option<String> {
        use OneRecordCalculation::*;
        let (replacement, reason) = match self {
            TimeBetweenCuts { .. } => ("time_in_zones", "it credits long gaps between samples as time in a zone"),
            ValueAfterEnd { .. } => {
                ("drop_after_effort_end", "it looks for a sample after the last sample, so it cannot succeed")
            }
            LowestHeld { .. } => {
                ("lowest_rolling_mean", "it needs a heart rate that does not change at all for a minute")
            }
            HeartRateReserveCurve { .. } => {
                ("banister_trimp", "it uses the male weighting without its coefficient for everyone")
            }
            Multiply { .. } => ("session_effort_load", "it multiplies any rating, including soreness, by any duration"),
            Divide { .. } => ("sleep_efficiency", "it accepts more sleep than time in bed"),
            Midpoint { .. } => ("sleep_clock_times", "it reports UTC times only"),
            SumFields { .. } => ("sleep_stage_split", "its total adds awake minutes to sleep stages"),
            NightAgreement { .. } => {
                ("sleep_regularity_index", "it compares nights at the same clock instant instead of 24 hours apart")
            }
            RepsTimesWeight { .. } => ("volume_load", "it adds pounds and kilograms together"),
            QuantityChange | SleepChange { .. } => {
                ("earlier_window_change", "it averages later readings into the earlier mean")
            }
            _ => return None,
        };
        Some(format!(
            "{} was replaced in 2.1.2 by {replacement}, because {reason}. `ath skill install` updates an unedited copy; an edited copy keeps this method.",
            self.operation()
        ))
    }
}

/// A calculation file read into typed settings.
#[derive(Debug, Clone, PartialEq)]
pub struct CalculationFile {
    pub name: String,
    pub calculation: OneRecordCalculation,
}

fn text_setting(block: &CalculationBlock, key: &str, default: &str) -> String {
    block.first(key).unwrap_or(default).to_string()
}

fn number_setting<T: std::str::FromStr>(block: &CalculationBlock, key: &str, default: T) -> T {
    block.first(key).and_then(|value| value.parse().ok()).unwrap_or(default)
}

impl CalculationFile {
    /// Read a skill file's calculation block into typed settings.
    pub fn parse(text: &str) -> Result<CalculationFile> {
        let block = parse_block(text)?;
        Ok(CalculationFile { name: block.name.clone(), calculation: Self::from_block(&block) })
    }

    /// Choose the operation from `use`, else from the name.
    pub fn from_block(block: &CalculationBlock) -> OneRecordCalculation {
        use OneRecordCalculation::*;
        let action = block.first("use").unwrap_or(block.name.as_str());
        match action {
            "heart-rate-zones" | "time_between_cuts" => TimeBetweenCuts {
                sample_field: text_setting(block, "sample_field", "heart_rate_samples"),
                maximum_field: text_setting(block, "maximum_field", "maximum_heart_rate_bpm"),
                cuts: block
                    .all("zone")
                    .map(|values| values.iter().filter_map(|value| value.parse().ok()).collect())
                    .unwrap_or_else(|| vec![0.6, 0.7, 0.8, 0.9]),
            },
            "heart-rate-recovery" | "value_after_end" => ValueAfterEnd {
                sample_field: text_setting(block, "sample_field", "heart_rate_samples"),
                later_offset_ms: number_setting(block, "later_offset_ms", 60_000),
            },
            "resting-heart-rate" | "lowest_held" => LowestHeld {
                sample_field: text_setting(block, "sample_field", "heart_rate_samples"),
                sustain_ms: number_setting(block, "sustain_ms", 60_000),
            },
            "training-load-heart-rate" | "heart_rate_reserve_curve" => HeartRateReserveCurve {
                sample_field: text_setting(block, "sample_field", "heart_rate_samples"),
                rest_field: text_setting(block, "rest_field", "resting_heart_rate_bpm"),
                maximum_field: text_setting(block, "maximum_field", "maximum_heart_rate_bpm"),
                factor: number_setting(block, "factor", 1.92),
            },
            "training-load-effort" | "multiply" => Multiply {
                duration_field: text_setting(block, "duration_field", "duration_seconds"),
                effort_field: text_setting(block, "effort_field", "rating"),
            },
            "sleep-duration" | "sleep-interruptions" | "field" => {
                Field { field: text_setting(block, "field", "asleep_seconds") }
            }
            "sleep-efficiency" | "divide" => Divide {
                asleep_field: text_setting(block, "asleep_field", "asleep_seconds"),
                in_bed_field: text_setting(block, "in_bed_field", "in_bed_seconds"),
            },
            "sleep-timing" | "midpoint" => Midpoint {
                start_field: text_setting(block, "start_field", "start_at"),
                end_field: text_setting(block, "end_field", "end_at"),
            },
            "sleep-stages" | "sum_fields" => SumFields { field: text_setting(block, "field", "stage_minutes") },
            "sleep-regularity" | "night_agreement" => {
                NightAgreement { minimum_nights: number_setting(block, "minimum_nights", 7) }
            }
            "strength-volume" | "reps_times_weight" => RepsTimesWeight { field: text_setting(block, "field", "sets") },
            "same-quantity-change" | "subtract_earlier_mean" if block.name == "sleep-change" => {
                SleepChange { field: text_setting(block, "field", "asleep_seconds") }
            }
            "same-quantity-change" | "subtract_earlier_mean" => QuantityChange,
            "sleep-change" => SleepChange { field: text_setting(block, "field", "asleep_seconds") },
            "earlier_window_change" => EarlierWindowChange(WindowChangeSettings {
                field: block.first("field").map(str::to_string),
                window_days: number_setting(block, "window_days", 28),
                minimum_days: number_setting(block, "minimum_days", 7),
                main_sleep_hours: number_setting(block, "main_sleep_hours", 3.0),
            }),
            "drop_after_effort_end" => DropAfterEffortEnd(RecoverySettings {
                sample_field: text_setting(block, "sample_field", "heart_rate_samples"),
                later_offset_ms: number_setting(block, "later_offset_ms", 60_000),
                tolerance_ms: number_setting(block, "tolerance_ms", 5_000),
                max_interpolation_gap_ms: number_setting(block, "max_interpolation_gap_ms", 10_000),
                recovery_label: text_setting(block, "recovery_label", "recovery"),
            }),
            "sleep_regularity_index" => SleepRegularityIndex {
                window_days: number_setting(block, "window_days", 14),
                minimum_day_pairs: number_setting(block, "minimum_day_pairs", 6),
            },
            "volume_load" => VolumeLoad { field: text_setting(block, "field", "sets") },
            "session_effort_load" => SessionEffortLoad {
                duration_field: text_setting(block, "duration_field", "duration_seconds"),
                effort_field: text_setting(block, "effort_field", "rating"),
            },
            "banister_trimp" => BanisterTrimp(TrimpSettings {
                sample_field: text_setting(block, "sample_field", "heart_rate_samples"),
                rest_field: text_setting(block, "rest_field", "resting_heart_rate_bpm"),
                maximum_field: text_setting(block, "maximum_field", "maximum_heart_rate_bpm"),
                max_gap_ms: number_setting(block, "max_gap_ms", 60_000),
                male: (number_setting(block, "male_coefficient", 0.64), number_setting(block, "male_exponent", 1.92)),
                female: (
                    number_setting(block, "female_coefficient", 0.86),
                    number_setting(block, "female_exponent", 1.67),
                ),
            }),
            "time_in_zones" => TimeInZones {
                sample_field: text_setting(block, "sample_field", "heart_rate_samples"),
                maximum_field: text_setting(block, "maximum_field", "maximum_heart_rate_bpm"),
                cuts: block
                    .all("zone")
                    .map(|values| values.iter().filter_map(|value| value.parse().ok()).collect())
                    .unwrap_or_else(|| vec![0.6, 0.7, 0.8, 0.9]),
                max_gap_ms: number_setting(block, "max_gap_ms", 60_000),
            },
            "lowest_rolling_mean" => LowestRollingMean(RollingMeanSettings {
                sample_field: text_setting(block, "sample_field", "heart_rate_samples"),
                window_ms: number_setting(block, "window_ms", 300_000),
                minimum_samples: number_setting(block, "minimum_samples", 3),
                max_gap_ms: number_setting(block, "max_gap_ms", 120_000),
            }),
            "sleep_efficiency" => SleepEfficiency {
                asleep_field: text_setting(block, "asleep_field", "asleep_seconds"),
                in_bed_field: text_setting(block, "in_bed_field", "in_bed_seconds"),
            },
            "sleep_stage_split" => SleepStageSplit { field: text_setting(block, "field", "stage_minutes") },
            "sleep_clock_times" => SleepClockTimes {
                start_field: text_setting(block, "start_field", "start_at"),
                end_field: text_setting(block, "end_field", "end_at"),
            },
            "hyrox_splits" => HyroxSplits,
            other if is_range_block(block, other) => Range(block.name.clone()),
            other => Unknown(other.to_string()),
        }
    }
}

/// A range calculation file names what it needs (`needs:`) or how it groups
/// (`per:`), or is named after a range operation.
fn is_range_block(block: &CalculationBlock, action: &str) -> bool {
    block.settings.contains_key("needs")
        || block.settings.contains_key("per")
        || super::range::Operation::parse(&block.name).is_some()
        || super::range::Operation::parse(action).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::shipped_calculation as shipped_skill_file;

    #[test]
    fn shipped_files_read_into_typed_settings() {
        let zones = CalculationFile::parse(shipped_skill_file("heart-rate-zones").unwrap()).unwrap();
        assert_eq!(
            zones.calculation,
            OneRecordCalculation::TimeInZones {
                sample_field: "heart_rate_samples".into(),
                maximum_field: "maximum_heart_rate_bpm".into(),
                cuts: vec![0.6, 0.7, 0.8, 0.9],
                max_gap_ms: 60_000,
            }
        );
        let change = CalculationFile::parse(shipped_skill_file("hrv-change").unwrap()).unwrap();
        assert_eq!(change.name, "hrv-change");
        assert_eq!(
            change.calculation,
            OneRecordCalculation::EarlierWindowChange(WindowChangeSettings {
                field: None,
                window_days: 28,
                minimum_days: 7,
                main_sleep_hours: 3.0,
            })
        );
        let sleep = CalculationFile::parse(shipped_skill_file("sleep-change").unwrap()).unwrap();
        assert!(matches!(
            sleep.calculation,
            OneRecordCalculation::EarlierWindowChange(WindowChangeSettings { field: Some(ref field), .. }) if field == "asleep_seconds"
        ));
        let summary = CalculationFile::parse(shipped_skill_file("summary").unwrap()).unwrap();
        assert_eq!(summary.calculation, OneRecordCalculation::Range("summary".into()));
        let calculation_files = crate::assets::skill_files().iter().filter(|file| {
            file.path.ends_with("/SKILL.md") && std::str::from_utf8(file.bytes).unwrap().contains("```ath-calculation")
        });
        for file in calculation_files {
            let parsed = CalculationFile::parse(std::str::from_utf8(file.bytes).unwrap()).unwrap();
            assert!(parsed.calculation.method_notice().is_none(), "{} ships a replaced operation", file.path);
        }
    }

    #[test]
    fn the_2_1_1_operations_still_read_their_settings() {
        let text = "```ath-calculation\nname: same-quantity-change\nuse: subtract_earlier_mean\n```\n";
        let old = CalculationFile::parse(text).unwrap();
        assert_eq!(old.calculation, OneRecordCalculation::QuantityChange);
        assert!(old.calculation.method_notice().unwrap().starts_with("subtract_earlier_mean was replaced in 2.1.2"));
    }

    #[test]
    fn supplied_intensity_and_feeling_are_refused() {
        for name in ["intensity", "Feeling"] {
            let text = format!("```ath-calculation\nname: label-read\nuse: field\nfield: supplied:{name}\n```\n");
            let error = CalculationFile::parse(&text).unwrap_err();
            assert_eq!(error.to_string(), format!("a calculation cannot read supplied:{name}"));
        }
        let other = "```ath-calculation\nname: other-read\nuse: field\nfield: supplied:Gear\n```\n";
        assert!(CalculationFile::parse(other).is_ok());
    }

    #[test]
    fn block_errors_name_the_problem() {
        let message = |text: &str| CalculationFile::parse(text).unwrap_err().to_string();
        assert_eq!(message("no block"), "the skill file has no ath-calculation block");
        assert_eq!(message("```ath-calculation\nname: x\n"), "the calculation block is not closed");
        assert_eq!(message("```ath-calculation\nname: x\nbad line\n```"), "could not read 'bad line'");
        assert_eq!(message("```ath-calculation\nuse: field\n```"), "the calculation block needs a name");
    }

    #[test]
    fn methods_come_from_the_shipped_range_files() {
        assert_eq!(shipped_method("summary"), "count, minimum, maximum, mean, and sum per source");
        assert_eq!(shipped_method("heart-rate-zones"), "");
    }

    #[test]
    fn unreadable_numbers_keep_their_defaults() {
        let text = "```ath-calculation\nname: x\nuse: lowest_held\nsustain_ms: soon\n```";
        assert_eq!(
            CalculationFile::parse(text).unwrap().calculation,
            OneRecordCalculation::LowestHeld { sample_field: "heart_rate_samples".into(), sustain_ms: 60_000 }
        );
        let text = "```ath-calculation\nname: x\nuse: time_between_cuts\nzone: half\nzone: 0.5\n```";
        let OneRecordCalculation::TimeBetweenCuts { cuts, .. } = CalculationFile::parse(text).unwrap().calculation
        else {
            panic!("zones")
        };
        assert_eq!(cuts, vec![0.5]);
    }
}
