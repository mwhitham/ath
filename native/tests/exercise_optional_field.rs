//! Maintainability exercise (temporary): an invented optional field on an
//! athlete report goes through the one encoder, the package, and the check.

use athleticstandard::json::{self, Value};
use athleticstandard::model::*;
use athleticstandard::package::{apply_package_import, export_package, preview_package_import, ExportOptions};
use athleticstandard::store::check::check_database;
use athleticstandard::store::{Changes, CreateOptions, NewRecord, Store};

#[test]
fn an_invented_optional_field_round_trips() {
    let directory = athleticstandard::files::make_temporary_directory("ath-exercise-field-").unwrap();
    let mut store = Store::create(directory.join("a.ath.sqlite"), CreateOptions::default()).unwrap();
    let data = RecordData::decode(
        RecordType::AthleteReport,
        &serde_json::json!({
            "report_type": "note",
            "reported_at": "2026-11-14T08:00:00.000Z",
            "input_reported_at": { "timestamp_missing_reason": "not_applicable", "utc_offset_missing_reason": "not_applicable" },
            "reported_via": "text",
            "note": "steady",
            "exercise_mood_label": "calm",
        }),
    )
    .unwrap();
    let applied = store
        .apply(Changes { creates: vec![NewRecord::new(data, RecordActor::Athlete)], ..Changes::default() })
        .unwrap();
    let record_id = applied.records[0].record_id.clone();
    let stored: String = store
        .connection()
        .query_row("SELECT record_json FROM record_revisions WHERE record_id = ?1", [&record_id], |row| row.get(0))
        .unwrap();
    assert_eq!(json::parse(&stored).unwrap()["exercise_mood_label"], Value::from("calm"));
    assert!(check_database(&store).unwrap().valid);

    export_package(&store, &directory.join("package"), &ExportOptions::default()).unwrap();
    let mut restored = Store::create(directory.join("b.ath.sqlite"), CreateOptions::default()).unwrap();
    let preview = preview_package_import(&restored, &directory.join("package")).unwrap();
    apply_package_import(&mut restored, &preview, &[]).unwrap();
    let again: String = restored
        .connection()
        .query_row("SELECT data_json FROM athlete_reports WHERE record_id = ?1", [&record_id], |row| row.get(0))
        .unwrap();
    assert_eq!(json::parse(&again).unwrap()["exercise_mood_label"], Value::from("calm"));
    assert!(check_database(&restored).unwrap().valid);
    std::fs::remove_dir_all(directory).unwrap();
}
