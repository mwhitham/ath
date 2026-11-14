//! Link a prediction revision to a benchmark result revision and grade it.
//!
//! A grade names the exact link revision it used. Grading the same pair again
//! reuses the link. Grading after the result was corrected revises the link,
//! so an older grade keeps its original link revision (D99). The arithmetic is
//! `model::grade`, the same rule the store checks on every saved grade.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::grade::grade_arithmetic;
use crate::model::*;
use crate::store::read;
use crate::store::write::StaleStyle;
use crate::store::{Changes, Correction, ExtensionsUpdate, NewRecord, Store};

pub const DEFAULT_GRADE_CALCULATION_VERSION: &str = "prediction-grade-v1";

/// The program as the author of a calculation or a write.
pub fn tool_actor() -> RecordActor {
    RecordActor::Tool { tool: "ath".into(), tool_version: ATHLETIC_STANDARD_VERSION.into() }
}

/// What to grade.
#[derive(Debug, Clone, PartialEq)]
pub struct GradeRequest {
    pub prediction_id: String,
    pub result_id: String,
    pub actor: RecordActor,
    /// `prediction-grade-v1` by default.
    pub calculation_version: Option<String>,
    /// The write time. Now by default.
    pub created_at: Option<String>,
}

/// A record the plan would create.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedRecord {
    pub record_id: String,
    pub created_at: String,
    pub changed_by: RecordActor,
    pub data: RecordData,
}

impl PlannedRecord {
    pub fn to_new_record(&self) -> NewRecord {
        let mut record = NewRecord::new(self.data.clone(), self.changed_by.clone());
        record.record_id = Some(self.record_id.clone());
        record.created_at = Some(self.created_at.clone());
        record
    }
}

/// A correction the plan would write.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedCorrection {
    pub record_id: String,
    pub expected_revision: u64,
    pub changed_by: RecordActor,
    pub became_known_at: String,
    pub data: RecordData,
}

/// A checked grade, ready to preview and write.
#[derive(Debug, Clone, PartialEq)]
pub struct PredictionGradePlan {
    pub prediction: CurrentRecord,
    pub result: CurrentRecord,
    pub existing_link: Option<CurrentRecord>,
    pub link_input: Option<PlannedRecord>,
    pub link_correction: Option<PlannedCorrection>,
    pub grade_input: PlannedRecord,
}

fn current_record(connection: &Connection, record_id: &str) -> Result<CurrentRecord> {
    crate::view::records::checked_record_id(record_id)?;
    match read::get_record(connection, record_id)? {
        Some(record) if !record.is_deleted => Ok(record),
        _ => Err(Error::not_found(format!("current record not found: {record_id}"))),
    }
}

fn reference(record: &CurrentRecord) -> EvidenceReference {
    EvidenceReference { record_id: record.record_id.clone(), revision: record.current_revision }
}

/// The 2.1.0 grade messages begin in lower case.
fn lower_first(message: String) -> String {
    let mut characters = message.chars();
    match characters.next() {
        Some(first) => first.to_lowercase().chain(characters).collect(),
        None => message,
    }
}

/// Check a prediction and result and plan the link and grade.
pub fn plan_prediction_grade(
    connection: &Connection,
    request: &GradeRequest,
    new_id: &mut dyn FnMut() -> String,
) -> Result<PredictionGradePlan> {
    let prediction_record = current_record(connection, &request.prediction_id)?;
    let RecordData::Prediction(prediction) = &prediction_record.data else {
        return Err(Error::refused(format!("{} is not a prediction", request.prediction_id)));
    };
    let result_record = current_record(connection, &request.result_id)?;
    let RecordData::BenchmarkResult(result) = &result_record.data else {
        return Err(Error::refused(format!("{} is not a benchmark result", request.result_id)));
    };
    if prediction.benchmark_definition_id != result.benchmark_definition_id {
        return Err(Error::refused("prediction and result use different benchmark definitions"));
    }
    let score_type: Option<String> = connection
        .query_row(
            "SELECT score_type FROM benchmark_definitions WHERE record_id = ?1",
            [&prediction.benchmark_definition_id],
            |row| row.get(0),
        )
        .optional()?;
    let score_type = score_type.ok_or_else(|| Error::not_found("prediction benchmark definition is missing"))?;
    let arithmetic = grade_arithmetic(prediction, &result.score, &score_type)
        .map_err(|message| Error::other(lower_first(message)))?;

    let existing_id: Option<String> = connection
        .query_row(
            "SELECT records.record_id
             FROM prediction_result_links
             JOIN records ON records.record_id = prediction_result_links.record_id
             WHERE records.is_deleted = 0
               AND prediction_result_links.prediction_id = ?1
               AND prediction_result_links.prediction_revision = ?2
               AND prediction_result_links.benchmark_result_id = ?3
             ORDER BY prediction_result_links.benchmark_result_revision = ?4 DESC,
                      records.created_at DESC,
                      records.record_id DESC
             LIMIT 1",
            params![
                prediction_record.record_id,
                prediction_record.current_revision as i64,
                result_record.record_id,
                result_record.current_revision as i64
            ],
            |row| row.get(0),
        )
        .optional()?;
    let existing_link = match existing_id {
        Some(id) => Some(current_record(connection, &id)?),
        None => None,
    };
    let existing_data = match &existing_link {
        Some(link) => match &link.data {
            RecordData::PredictionResultLink(data) => Some(data.clone()),
            _ => return Err(Error::other("stored prediction-result link has the wrong record type")),
        },
        None => None,
    };

    let created_at = request.created_at.clone().unwrap_or_else(time::now_utc);
    let link_id = existing_link.as_ref().map(|link| link.record_id.clone()).unwrap_or_else(&mut *new_id);
    let link_needs_revision =
        existing_data.as_ref().is_some_and(|data| data.benchmark_result.revision != result_record.current_revision);
    let link_revision = match &existing_link {
        None => 1,
        Some(link) => link.current_revision + u64::from(link_needs_revision),
    };
    let link_input = match &existing_link {
        None => Some(PlannedRecord {
            record_id: link_id.clone(),
            created_at: created_at.clone(),
            changed_by: request.actor.clone(),
            data: RecordData::PredictionResultLink(PredictionResultLinkData {
                prediction: reference(&prediction_record),
                benchmark_result: reference(&result_record),
                created_by: request.actor.clone(),
                linked_at: created_at.clone(),
                content_version: 1,
                status: AuthoredRecordStatus::Active,
            }),
        }),
        Some(_) => None,
    };
    let link_correction = match (&existing_link, &existing_data) {
        (Some(link), Some(data)) if link_needs_revision => Some(PlannedCorrection {
            record_id: link.record_id.clone(),
            expected_revision: link.current_revision,
            changed_by: request.actor.clone(),
            became_known_at: created_at.clone(),
            data: RecordData::PredictionResultLink(PredictionResultLinkData {
                benchmark_result: reference(&result_record),
                created_by: request.actor.clone(),
                linked_at: created_at.clone(),
                content_version: data.content_version + 1,
                ..data.clone()
            }),
        }),
        _ => None,
    };
    let calculation_version =
        request.calculation_version.clone().unwrap_or_else(|| DEFAULT_GRADE_CALCULATION_VERSION.to_string());
    let link_reference = EvidenceReference { record_id: link_id.clone(), revision: link_revision };
    let grade_input = PlannedRecord {
        record_id: new_id(),
        created_at: created_at.clone(),
        changed_by: request.actor.clone(),
        data: RecordData::PredictionGrade(PredictionGradeData {
            authorship: AuthoredRecordMetadata {
                author: AuthoredRecordAuthor::Tool {
                    tool: "ath".into(),
                    tool_version: ATHLETIC_STANDARD_VERSION.into(),
                    calculation_version,
                },
                authored_at: created_at.clone(),
                evidence: vec![link_reference.clone()],
                content_version: 1,
                status: AuthoredRecordStatus::Active,
            },
            prediction_result_link: link_reference,
            signed_error: arithmetic.signed_error,
            absolute_error_percentage: arithmetic.absolute_error_percentage,
            result_in_predicted_range: arithmetic.result_in_predicted_range,
            predicted_score: prediction.predicted_score.clone(),
            actual_score: result.score.clone(),
        }),
    };
    Ok(PredictionGradePlan {
        prediction: prediction_record,
        result: result_record,
        existing_link,
        link_input,
        link_correction,
        grade_input,
    })
}

/// A planned record in a preview.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlannedRecordPreview {
    pub record_id: String,
    #[serde(serialize_with = "serialize_data")]
    pub data: RecordData,
}

fn serialize_data<S: serde::Serializer>(data: &RecordData, serializer: S) -> std::result::Result<S::Ok, S::Error> {
    data.to_value().serialize(serializer)
}

/// What a grade write does with the prediction-result link.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum LinkAction {
    Create {
        record_id: String,
        #[serde(serialize_with = "serialize_data")]
        data: RecordData,
    },
    Revise {
        record_id: String,
        current_revision: u64,
        new_revision: u64,
        #[serde(serialize_with = "serialize_data")]
        data: RecordData,
    },
    Reuse {
        record_id: String,
        revision: u64,
    },
}

/// The `ath grade --json` preview.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GradePreview {
    pub file: String,
    pub prediction: EvidenceReference,
    pub result: EvidenceReference,
    pub prediction_result_link: LinkAction,
    pub grade: PlannedRecordPreview,
    /// Always false: a preview never writes.
    pub write_confirmed: bool,
}

impl PredictionGradePlan {
    pub fn preview(&self, file: &str) -> GradePreview {
        let prediction_result_link = if let Some(link) = &self.link_input {
            LinkAction::Create { record_id: link.record_id.clone(), data: link.data.clone() }
        } else if let (Some(correction), Some(existing)) = (&self.link_correction, &self.existing_link) {
            LinkAction::Revise {
                record_id: existing.record_id.clone(),
                current_revision: existing.current_revision,
                new_revision: existing.current_revision + 1,
                data: correction.data.clone(),
            }
        } else {
            let existing = self.existing_link.as_ref().expect("a plan without a new link reuses one");
            LinkAction::Reuse { record_id: existing.record_id.clone(), revision: existing.current_revision }
        };
        GradePreview {
            file: file.to_string(),
            prediction: reference(&self.prediction),
            result: reference(&self.result),
            prediction_result_link,
            grade: PlannedRecordPreview {
                record_id: self.grade_input.record_id.clone(),
                data: self.grade_input.data.clone(),
            },
            write_confirmed: false,
        }
    }
}

/// The records a grade write produced.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GradeWritten {
    #[serde(serialize_with = "crate::view::serialize_record")]
    pub link: CurrentRecord,
    #[serde(serialize_with = "crate::view::serialize_record")]
    pub grade: CurrentRecord,
    pub database_revision: u64,
}

impl GradeWritten {
    /// `hit — signed error …` or `miss — …`, as `ath grade` printed.
    pub fn summary_line(&self) -> String {
        let RecordData::PredictionGrade(grade) = &self.grade.data else {
            return String::new();
        };
        format!(
            "{} — signed error {}, absolute error {}%",
            if grade.result_in_predicted_range { "hit" } else { "miss" },
            crate::json::format_number(grade.signed_error),
            crate::json::format_number(grade.absolute_error_percentage)
        )
    }
}

/// Write a planned grade in one transaction. The prediction, the result, and
/// any reused link must still be at the planned revisions.
pub fn grade_prediction(store: &mut Store, plan: &PredictionGradePlan) -> Result<GradeWritten> {
    let mut expected = vec![
        (plan.prediction.record_id.clone(), plan.prediction.current_revision),
        (plan.result.record_id.clone(), plan.result.current_revision),
    ];
    if let Some(link) = &plan.existing_link {
        expected.push((link.record_id.clone(), link.current_revision));
    }
    let corrections = plan
        .link_correction
        .iter()
        .map(|correction| Correction {
            record_id: correction.record_id.clone(),
            expected_revision: correction.expected_revision,
            data: correction.data.clone(),
            extensions: ExtensionsUpdate::Keep,
            changed_by: correction.changed_by.clone(),
            became_known_at: Some(correction.became_known_at.clone()),
            became_known_at_basis: BecameKnownAtBasis::LocalWrite,
            stale_style: StaleStyle::Preview,
        })
        .collect();
    let mut creates: Vec<NewRecord> = plan.link_input.iter().map(PlannedRecord::to_new_record).collect();
    creates.push(plan.grade_input.to_new_record());
    let applied =
        store.apply(Changes { expected_current_revisions: expected, corrections, creates, ..Changes::default() })?;
    let link = applied
        .records
        .iter()
        .find(|record| record.record_type() == RecordType::PredictionResultLink)
        .cloned()
        .or_else(|| plan.existing_link.clone());
    let grade = applied.records.iter().find(|record| record.record_type() == RecordType::PredictionGrade).cloned();
    match (link, grade) {
        (Some(link), Some(grade)) => Ok(GradeWritten { link, grade, database_revision: applied.database_revision }),
        _ => Err(Error::other("prediction grade write did not return its link and grade")),
    }
}
