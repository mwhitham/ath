//! Calculations: typed calculation settings, arithmetic, and receipts.
//!
//! `range` runs the eight named calculations over many records and returns a
//! receipt naming every input revision. `record` runs one editable calculation
//! file on one record. `definition` reads calculation files into typed
//! settings and chooses the installed or shipped copy. Reading a calculation
//! never writes an athlete record.

pub mod definition;
pub mod range;
pub mod record;

use std::path::Path;

pub use definition::{CalculationFile, OneRecordCalculation};
pub use range::{
    render_calculation, CalculateOptions, CalculationReceipt, CalculationResult, Group, Operation, RecordKind,
    MAX_RECEIPT_REFERENCES, MAX_RUNTIME_INPUTS,
};
pub use record::{ApplyResult, FieldRead, OneRecordResult};

use crate::error::Result;
use crate::store::Store;

/// `ath calculate`: run a named range calculation.
pub fn calculate_range(store: &Store, options: &CalculateOptions) -> Result<CalculationReceipt> {
    range::calculate(store.connection(), options)
}

/// `ath apply`: run the calculation file named `calculation` on one current
/// record. The installed copy in `working_directory/.agents/skills` wins over
/// the shipped copy.
pub fn apply(store: &Store, calculation: &str, record_id: &str, working_directory: &Path) -> Result<ApplyResult> {
    let connection = store.connection();
    // The 2.1.0 order: choose the file, find the record, then read the file.
    let file = crate::assets::resolve_calculation(calculation, working_directory)?;
    let record = record::stored_record(connection, record_id)?;
    let parsed = CalculationFile::parse(&file.text)?;
    record::apply_to(connection, &parsed, record)
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
    lines.push(crate::json::encode(&result.result));
    lines.join("\n")
}
