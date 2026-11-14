//! Calculation files: the editable `ath-calculation` block in a skill file.
//!
//! A block is a list of `key: value` lines. The reader turns it into typed
//! settings that choose one reviewed arithmetic operation and its parameters.
//! It is not an expression language: an unknown operation is reported, and a
//! setting that does not read as a number keeps its documented default.
//!
//! `ath apply` reads `.agents/skills/<name>/SKILL.md` in the working directory
//! when it exists, else the copy shipped with this program. `ath calculate`
//! reads its method name from the shipped copy.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The folder an installed skill copy lives in, under the working directory.
pub const INSTALLED_SKILLS_DIR: &str = ".agents/skills";

/// Source labels that stay in `supplied_fields` and never feed a calculation (D186).
pub const SOURCE_LABELS: &[&str] = &["supplied:intensity", "supplied:Feeling"];

macro_rules! shipped {
    ($($name:literal),* $(,)?) => {
        /// Every shipped skill file, by folder name.
        pub const SHIPPED_SKILL_FILES: &[(&str, &str)] = &[
            $(($name, include_str!(concat!("../../../skills/", $name, "/SKILL.md")))),*
        ];
    };
}

shipped!(
    "athletic-standard",
    "baseline",
    "compare-sources",
    "correlation",
    "coverage",
    "heart-rate-recovery",
    "heart-rate-zones",
    "hrv-change",
    "provider-period-summary",
    "resting-heart-rate",
    "resting-heart-rate-change",
    "sleep-change",
    "sleep-duration",
    "sleep-efficiency",
    "sleep-interruptions",
    "sleep-regularity",
    "sleep-stages",
    "sleep-timing",
    "strength-volume",
    "summary",
    "training-load-effort",
    "training-load-heart-rate",
    "trend",
    "workout-drift",
);

/// The shipped `SKILL.md` text for a skill folder name.
pub fn shipped_skill_file(name: &str) -> Option<&'static str> {
    SHIPPED_SKILL_FILES.iter().find(|(shipped, _)| *shipped == name).map(|(_, text)| *text)
}

/// Where a calculation file came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalculationFileOrigin {
    /// An installed copy in the working directory. It wins over the shipped copy.
    Installed(PathBuf),
    /// The copy shipped with this program.
    Shipped,
}

/// A calculation file chosen for a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculationFileText {
    /// The skill folder name.
    pub name: String,
    pub origin: CalculationFileOrigin,
    pub text: String,
}

/// The skill folder name in a calculation argument: the last path part,
/// without `.md`.
pub fn calculation_name(argument: &str) -> &str {
    let without_suffix = argument.strip_suffix(".md").unwrap_or(argument);
    without_suffix.rsplit(['/', '\\']).next().unwrap_or(without_suffix)
}

/// The installed calculation file in `working_directory`, or the shipped one.
pub fn resolve_calculation_file(argument: &str, working_directory: &Path) -> Result<CalculationFileText> {
    let name = calculation_name(argument).to_string();
    let installed = working_directory.join(INSTALLED_SKILLS_DIR).join(&name).join("SKILL.md");
    if installed.exists() {
        let text = std::fs::read_to_string(&installed)?;
        return Ok(CalculationFileText { name, origin: CalculationFileOrigin::Installed(installed), text });
    }
    match shipped_skill_file(&name) {
        Some(text) => Ok(CalculationFileText { name, origin: CalculationFileOrigin::Shipped, text: text.to_string() }),
        None => Err(Error::not_found(format!("no calculation skill named {name}"))),
    }
}

/// The `method:` line of a shipped range calculation, or an empty string.
pub fn shipped_method(operation: &str) -> String {
    shipped_skill_file(operation).map(method_line).unwrap_or_default()
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
    /// An operation this program does not have.
    Unknown(String),
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
            other => Unknown(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_files_read_into_typed_settings() {
        let zones = CalculationFile::parse(shipped_skill_file("heart-rate-zones").unwrap()).unwrap();
        assert_eq!(
            zones.calculation,
            OneRecordCalculation::TimeBetweenCuts {
                sample_field: "heart_rate_samples".into(),
                maximum_field: "maximum_heart_rate_bpm".into(),
                cuts: vec![0.6, 0.7, 0.8, 0.9],
            }
        );
        let change = CalculationFile::parse(shipped_skill_file("hrv-change").unwrap()).unwrap();
        assert_eq!(change.name, "same-quantity-change");
        assert_eq!(change.calculation, OneRecordCalculation::QuantityChange);
        let sleep = CalculationFile::parse(shipped_skill_file("sleep-change").unwrap()).unwrap();
        assert_eq!(sleep.calculation, OneRecordCalculation::SleepChange { field: "asleep_seconds".into() });
        let summary = CalculationFile::parse(shipped_skill_file("summary").unwrap()).unwrap();
        assert_eq!(summary.calculation, OneRecordCalculation::Unknown("count".into()));
        for (name, text) in SHIPPED_SKILL_FILES {
            if *name != "athletic-standard" {
                CalculationFile::parse(text).unwrap();
            }
        }
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
        assert_eq!(calculation_name("skills/heart-rate-zones.md"), "heart-rate-zones");
        assert_eq!(calculation_name("a\\b\\sleep-duration"), "sleep-duration");
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
