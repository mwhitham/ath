//! The published documents agree with the format the code implements.

use athleticstandard::model::RecordType;

const DOCUMENTS: &[&str] = &["SPEC.md", "README.md", "skills/athletic-standard/references/format.md"];

fn read(relative: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)).unwrap()
}

#[test]
fn documents_state_the_record_type_count() {
    let count = RecordType::ALL.len();
    assert_eq!(count, 24);
    for file in DOCUMENTS {
        let text = read(file);
        assert!(text.contains(&count.to_string()), "{file} does not state {count}");
        assert!(!text.contains("23 record types") && !text.contains("one of 23"), "{file} names the old count");
    }
}

#[test]
fn documents_name_the_chunked_series_files() {
    for file in DOCUMENTS {
        let text = read(file);
        assert!(text.contains("<source-id>-<quantity>-000001.ath-series.json"), "{file}");
        assert!(!text.contains("<source-id>-<quantity>.ath-series.json"), "{file} names the unchunked file");
    }
}
