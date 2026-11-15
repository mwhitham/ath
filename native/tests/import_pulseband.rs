//! The invented Pulse Band reader goes through the one import operation:
//! detection, staging, matching, the preview, and the write. It adds no SQL
//! and no command code.

mod import_support;

use athleticstandard::import::{self, ImportOptions};
use athleticstandard::model::*;
use athleticstandard::store::check::check_database;
use athleticstandard::store::read::ListOptions;
use import_support::{new_store, write_directory, TempDir};

const CSV: &str = "time,resting_heart_rate,hrv_rmssd\n\
2026-09-01T07:00:00-05:00,52,61.5\n\
2026-09-02T07:00:00-05:00,54,58\n\
,50,70\n";

#[test]
fn a_pulse_band_export_imports_through_the_shared_operation() {
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let export = directory.path().join("pulseband-export");
    write_directory(&export, &[("pulseband_daily.csv".to_string(), CSV.to_string())]);
    let options = ImportOptions { received_at: Some("2026-10-13T12:00:00.000Z".into()) };

    let preview = import::preview(&store, &export, &options).unwrap();
    assert_eq!(preview.provider, "pulseband");
    // One source and four point measurements; the row without a time is skipped.
    assert_eq!(preview.new_record_count(), 5);
    assert!(preview.conflicts.is_empty());
    preview.apply(&mut store).unwrap();

    let points = store
        .list_records(&ListOptions {
            record_type: Some(RecordType::PointMeasurement),
            include_deleted: false,
            limit: 100,
        })
        .unwrap();
    assert_eq!(points.len(), 4);
    let RecordData::PointMeasurement(first) = &points[0].data else { panic!("not a point measurement") };
    assert_eq!(first.recorded_at.as_deref(), Some("2026-09-01T12:00:00.000Z"));

    // The same export again matches every stored record.
    let again = import::preview(&store, &export, &options).unwrap();
    assert_eq!(again.new_record_count(), 0);
    assert_eq!(again.matching_record_count(), 5);
    assert!(check_database(&store).unwrap().valid);
}
