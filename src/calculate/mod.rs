//! Calculations: typed calculation settings, arithmetic, and receipts.
//!
//! `range` runs the eight named calculations over many records and returns a
//! receipt naming every input revision. `record` runs one editable calculation
//! file on one record. `definition` reads calculation files into typed
//! settings and chooses the installed or shipped copy. Reading a calculation
//! never writes an athlete record.

pub mod definition;
pub mod hyrox;
pub mod methods;
pub mod range;
pub mod record;
#[cfg(test)]
mod snapshot_tests;

use std::path::Path;

pub use definition::{CalculationFile, OneRecordCalculation};
pub use range::{
    render_calculation, CalculateOptions, CalculationReceipt, CalculationResult, Group, Operation, RecordKind,
    MAX_RECEIPT_REFERENCES, MAX_RUNTIME_INPUTS,
};
pub use record::{ApplyResult, FieldRead, OneRecordResult, UNKNOWN_OPERATION_HINT};

use crate::error::Result;
use crate::store::Store;

/// `ath calculate`: run a named range calculation. Every input comes from one
/// read snapshot, so a write that commits during the calculation is not seen.
pub fn calculate_range(store: &Store, options: &CalculateOptions) -> Result<CalculationReceipt> {
    store.read_snapshot(|| range::calculate(store.connection(), options))
}

/// `ath apply`: run the calculation file named `calculation` on one current
/// record. The installed copy in `working_directory/.agents/skills` wins over
/// the shipped copy. The record and the records it names come from one read
/// snapshot.
pub fn apply(store: &Store, calculation: &str, record_id: &str, working_directory: &Path) -> Result<ApplyResult> {
    let connection = store.connection();
    // The 2.1.0 order: choose the file, find the record, then read the file.
    let file = crate::assets::resolve_calculation(calculation, working_directory)?;
    store.read_snapshot(|| {
        let record = record::stored_record(connection, record_id)?;
        let parsed =
            CalculationFile::parse(&file.text).map_err(|error| coaching_refusal(&file.text).unwrap_or(error))?;
        if let OneRecordCalculation::Range(name) = &parsed.calculation {
            return Err(range_refusal(name));
        }
        crate::package::failpoint!("calculate.apply_after_target");
        record::apply_to(connection, &parsed, record)
    })
}

/// `ath apply` on a coaching skill: it gives advice, it does not calculate.
fn coaching_refusal(text: &str) -> Option<crate::error::Error> {
    use crate::assets::skill::{read_frontmatter, FrontmatterValue};
    let frontmatter = read_frontmatter(text);
    let Some(FrontmatterValue::Map(metadata)) = frontmatter.get("metadata") else {
        return None;
    };
    if metadata.get("kind").map(String::as_str) != Some("coaching") {
        return None;
    }
    let name = match frontmatter.get("name") {
        Some(FrontmatterValue::Text(name)) => name.clone(),
        _ => "this".to_string(),
    };
    Some(crate::error::Error::refused(format!(
        "{name} is a coaching skill, not a calculation. An agent reads it to plan and review training; `ath apply` runs calculation skills."
    )))
}

/// `ath apply` on a range calculation (D243): it needs a quantity and a
/// window, not one record, so name the command that runs it.
fn range_refusal(name: &str) -> crate::error::Error {
    let example = match name {
        "compare-sources" => "ath calculate compare-sources <quantity> --source <id> --compare-source <id>".to_string(),
        "correlation" => {
            "ath calculate correlation <quantity> --other-quantity <quantity> --timezone <zone>".to_string()
        }
        "workout-drift" => "ath calculate workout-drift heart_rate --session <workout-session-id>".to_string(),
        "provider-period-summary" => {
            "ath calculate provider-period-summary <quantity> --period-type <provider-period-type>".to_string()
        }
        "coverage" => "ath calculate coverage <quantity> --timezone <zone>".to_string(),
        other => format!("ath calculate {other} <quantity>"),
    };
    crate::error::Error::refused(format!(
        "{name} is a range calculation over many records, not one record. Run: {example}"
    ))
}

/// The human text `ath apply` printed.
pub fn render_apply_result(result: &ApplyResult) -> String {
    let mut lines = vec![format!("{} on {}", result.calculation, result.record_id)];
    for field in &result.fields_read {
        lines.push(format!("read {}: {}", field.field, if field.present { "present" } else { "missing" }));
    }
    if !result.missing.is_empty() {
        lines.push(format!("missing: {}", result.missing.join(", ")));
    }
    if let Some(inputs) = &result.inputs {
        let plural = if inputs.record_count == 1 { "" } else { "s" };
        lines.push(format!("compared with: {} other record{plural}", inputs.record_count));
    }
    for exclusion in &result.exclusions {
        lines.push(format!("left out: {exclusion}"));
    }
    if let Some(notice) = &result.method_notice {
        lines.push(format!("note: {notice}"));
    }
    lines.push(crate::json::encode(&result.result));
    lines.join("\n")
}
