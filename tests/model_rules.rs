//! Record rules that the rebuild enforces on purpose beyond the 2.1.0 validator.

use athleticstandard::json;
use athleticstandard::model::CurrentRecord;

#[test]
fn a_proto_key_cannot_hide_a_source_claim_on_an_athlete_report() {
    let text = r#"{
        "record_id": "01890f3e-e7d3-7cc0-88c8-000000000013",
        "created_at": "2026-08-09T12:00:00.000Z",
        "current_revision": 1,
        "is_deleted": false,
        "extensions": { "org.example/notes": { "__proto__": { "source_id": "01890f3e-e7d3-7cc0-88c8-000000000003" } } },
        "record_type": "athlete_report",
        "data": {
            "report_type": "note",
            "reported_at": "2026-08-09T12:00:00.000Z",
            "input_reported_at": { "timestamp_missing_reason": "not_applicable", "utc_offset_missing_reason": "not_applicable" },
            "reported_via": "text",
            "note": "Invented note."
        }
    }"#;
    let error = CurrentRecord::from_value(&json::parse(text).unwrap()).unwrap_err();
    assert_eq!(error.to_string(), "extensions: extensions cannot contain source_id");
}
