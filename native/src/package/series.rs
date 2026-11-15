//! Reading one measurement-series file without holding its whole JSON tree.
//!
//! A series file holds up to 50,000 samples with their revisions. As one JSON
//! value tree that is more than a gigabyte, so loading and restoring a large
//! package held gigabytes. This reader parses the two long lists one item at
//! a time. Each item is checked against the measurement-sample shape, reduced
//! to the facts the history rules read, decoded when asked, and released.
//! The rest of the file is checked with the series-file schema. The history
//! and series rules then run over the facts with the same code the schema
//! uses (`model::package::series_issues`).
//!
//! The quick reading only decides that a file is valid. A file with any
//! problem, or one it cannot follow (a repeated top-level key, a list that is
//! not an array), is read again whole, which reports the exact issues in the
//! 2.1.0 order. So the result never differs from the whole reading.

use std::path::Path;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

use super::load::parse_package_json;
use crate::error::Result;
use crate::json::{Map, Value};
use crate::model::package::{series_issues, HistoryFacts, RecordFacts, RevisionFacts};
use crate::model::schema::{from_checked_value, Issue};
use crate::model::*;

/// A series file that passed every check.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CheckedSeries {
    /// The file without its records and revisions: the version, source,
    /// quantity, bounds, deletions, and import keys.
    pub shell: Value,
    pub record_count: usize,
    pub revision_count: usize,
    /// What the history rules read from every item.
    pub facts: HistoryFacts,
    /// The decoded records and revisions, when decoding was asked for.
    pub records: Vec<CurrentRecord>,
    pub revisions: Vec<RecordRevision>,
}

impl CheckedSeries {
    pub(crate) fn text(&self, key: &str) -> &str {
        self.shell.get(key).and_then(Value::as_str).unwrap_or_default()
    }

    pub(crate) fn deletion_count(&self) -> usize {
        self.facts.deletions.len()
    }

    pub(crate) fn import_key_count(&self) -> usize {
        self.facts.import_keys.len()
    }

    pub(crate) fn deletions(&self) -> Result<Vec<RecordDeletion>> {
        Ok(from_checked_value(self.shell.get("record_deletions").unwrap_or(&Value::Null))?)
    }

    pub(crate) fn import_keys(&self) -> Result<Vec<RecordImportKey>> {
        Ok(from_checked_value(self.shell.get("record_import_keys").unwrap_or(&Value::Null))?)
    }
}

/// A series file read and checked.
pub(crate) enum SeriesRead {
    Valid(Box<CheckedSeries>),
    /// The whole file and its issues, in the 2.1.0 order.
    Invalid {
        value: Value,
        issues: Vec<Issue>,
    },
}

/// Read and check one series file. `decode` keeps the decoded records and
/// revisions. A JSON error is reported as the whole reading reports it.
pub(crate) fn read_series(bytes: &[u8], path: &Path, decode: bool) -> Result<SeriesRead> {
    if let Some(checked) = quick_read(bytes, decode) {
        return Ok(SeriesRead::Valid(Box::new(checked)));
    }
    let value = parse_package_json(bytes, path)?;
    let issues = measurement_series_file_schema().issues(&value);
    if !issues.is_empty() {
        return Ok(SeriesRead::Invalid { value, issues });
    }
    Ok(SeriesRead::Valid(Box::new(from_whole_value(value, decode)?)))
}

/// A valid file that the quick reading could not follow, read whole.
fn from_whole_value(mut value: Value, decode: bool) -> Result<CheckedSeries> {
    let facts = HistoryFacts::of(&value);
    let mut take = |key: &str| match value.get_mut(key).map(|list| std::mem::replace(list, Value::Array(Vec::new()))) {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    };
    let records = take("records");
    let revisions = take("record_revisions");
    let (record_count, revision_count) = (records.len(), revisions.len());
    let mut checked = CheckedSeries {
        shell: Value::Null,
        record_count,
        revision_count,
        facts,
        records: Vec::new(),
        revisions: Vec::new(),
    };
    if decode {
        checked.records = records.iter().map(CurrentRecord::from_checked).collect::<std::result::Result<_, _>>()?;
        checked.revisions =
            revisions.iter().map(RecordRevision::from_checked).collect::<std::result::Result<_, _>>()?;
    }
    checked.shell = value;
    Ok(checked)
}

/// The quick reading. `None` means: read the file whole.
fn quick_read(bytes: &[u8], decode: bool) -> Option<CheckedSeries> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut scan = Scan { decode, ..Scan::default() };
    let mut deserializer = serde_json::Deserializer::from_str(text);
    de::Deserializer::deserialize_map(&mut deserializer, FileVisitor(&mut scan)).ok()?;
    deserializer.end().ok()?;
    let Scan { shell, facts, records, revisions, record_count, revision_count, .. } = scan;
    let shell = Value::Object(shell);
    // The declared fields, checked with every list empty, so the file's own
    // rules see no items here. Then each short list on its own. The long
    // lists were checked one item at a time.
    let mut declared = shell.clone();
    for (key, schema) in [
        ("record_deletions", Vec::<RecordDeletion>::schema()),
        ("record_import_keys", Vec::<RecordImportKey>::schema()),
    ] {
        if let Some(list) = declared.get_mut(key) {
            if !schema.issues(list).is_empty() {
                return None;
            }
            *list = Value::Array(Vec::new());
        }
    }
    if !measurement_series_file_schema().issues(&declared).is_empty() {
        return None;
    }
    let mut facts = facts;
    facts.add_deletions_and_import_keys(&shell);
    let mut issues = Vec::new();
    series_issues(&shell, &facts, &mut issues);
    if !issues.is_empty() {
        return None;
    }
    Some(CheckedSeries { shell, record_count, revision_count, facts, records, revisions })
}

#[derive(Default)]
struct Scan {
    decode: bool,
    shell: Map<String, Value>,
    facts: HistoryFacts,
    records: Vec<CurrentRecord>,
    revisions: Vec<RecordRevision>,
    record_count: usize,
    revision_count: usize,
}

#[derive(Clone, Copy)]
enum List {
    Records,
    Revisions,
}

impl Scan {
    /// Check, reduce, and decode one item. An error stops the quick reading.
    fn item(&mut self, list: List, item: Value) -> std::result::Result<(), &'static str> {
        match list {
            List::Records => {
                if !measurement_sample_record_schema().issues(&item).is_empty() {
                    return Err("record");
                }
                self.facts.records.push(RecordFacts::of(&item));
                if self.decode {
                    self.records.push(CurrentRecord::from_checked(&item).map_err(|_| "record")?);
                }
                self.record_count += 1;
            }
            List::Revisions => {
                if !measurement_sample_revision_schema().issues(&item).is_empty() {
                    return Err("revision");
                }
                self.facts.revisions.push(RevisionFacts::of(&item));
                if self.decode {
                    self.revisions.push(RecordRevision::from_checked(&item).map_err(|_| "revision")?);
                }
                self.revision_count += 1;
            }
        }
        Ok(())
    }
}

struct FileVisitor<'a>(&'a mut Scan);

impl<'de> Visitor<'de> for FileVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a measurement-series file object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            if self.0.shell.contains_key(&key) {
                return Err(de::Error::custom("repeated key"));
            }
            let list = match key.as_str() {
                "records" => Some(List::Records),
                "record_revisions" => Some(List::Revisions),
                _ => None,
            };
            match list {
                Some(list) => {
                    map.next_value_seed(Items { scan: self.0, list })?;
                    self.0.shell.insert(key, Value::Array(Vec::new()));
                }
                None => {
                    let value: Value = map.next_value()?;
                    self.0.shell.insert(key, value);
                }
            }
        }
        Ok(())
    }
}

struct Items<'a> {
    scan: &'a mut Scan,
    list: List,
}

impl<'de> DeserializeSeed<'de> for Items<'_> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> std::result::Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Items<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an array of records")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
        while let Some(item) = seq.next_element::<Value>()? {
            self.scan.item(self.list, item).map_err(de::Error::custom)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;
    use serde_json::json as value;

    const SOURCE: &str = "01890f3e-e7d3-7cc0-88c8-000000000003";

    fn data(at: &str, bpm: f64) -> Value {
        value!({
            "input_timestamp": {
                "timestamp_basis": "source_offset_timestamp",
                "timestamp_text": at,
                "utc_offset_meaning": "provider_cycle_offset",
                "utc_offset_text": "Z"
            },
            "quantity": "heart_rate",
            "received_at": "2026-08-09T12:00:00.000Z",
            "received_at_basis": "import_time",
            "recorded_at": at,
            "source_id": SOURCE,
            "supplied_fields": { "b": 1, "a": 2 },
            "value": { "input_unit": "count/min", "input_value": bpm, "standard_unit": "bpm", "standard_value": bpm }
        })
    }

    fn id(number: u32) -> String {
        format!("01890f3e-e7d3-7cc0-88c8-{number:012x}")
    }

    fn record(number: u32, at: &str, bpm: f64, current_revision: u64, is_deleted: bool) -> Value {
        value!({
            "created_at": "2026-08-09T12:00:00.000Z",
            "current_revision": current_revision,
            "data": data(at, bpm),
            "is_deleted": is_deleted,
            "record_id": id(number),
            "record_type": "measurement_sample"
        })
    }

    fn revision(number: u32, revision: u64, at: &str, bpm: f64) -> Value {
        value!({
            "became_known_at": "2026-08-09T12:00:00.000Z",
            "became_known_at_basis": "provider_import",
            "changed_by": { "author_type": "tool", "tool": "ath", "tool_version": "1.0.0" },
            "data": data(at, bpm),
            "previous_revision": if revision == 1 { Value::Null } else { Value::from(revision - 1) },
            "record_id": id(number),
            "record_type": "measurement_sample",
            "revision": revision
        })
    }

    /// Three samples: one plain, one corrected once, and one deleted, with
    /// import keys and a deletion record.
    fn valid_file() -> Value {
        value!({
            "athleticstandard_version": "2.1.0",
            "end_at": "2026-08-09T11:14:00.000Z",
            "quantity": "heart_rate",
            "record_deletions": [{
                "deleted_at": "2026-08-10T00:00:00.000Z",
                "deleted_by": { "author_type": "athlete" },
                "record_id": id(3),
                "record_revision": 1
            }],
            "record_import_keys": [
                { "imported_at": "2026-08-09T12:00:00.000Z", "match_key": "k1", "match_key_method": "m/v1", "record_id": id(1) },
                { "imported_at": "2026-08-09T12:00:00.000Z", "match_key": "k2", "match_key_method": "m/v1", "record_id": id(2) }
            ],
            "record_revisions": [
                revision(1, 1, "2026-08-09T11:12:00.000Z", 142.0),
                revision(2, 1, "2026-08-09T11:13:00.000Z", 150.0),
                revision(2, 2, "2026-08-09T11:13:00.000Z", 151.0),
                revision(3, 1, "2026-08-09T11:14:00.000Z", 160.0)
            ],
            "records": [
                record(1, "2026-08-09T11:12:00.000Z", 142.0, 1, false),
                record(2, "2026-08-09T11:13:00.000Z", 151.0, 2, false),
                record(3, "2026-08-09T11:14:00.000Z", 160.0, 1, true)
            ],
            "source_id": SOURCE,
            "start_at": "2026-08-09T11:12:00.000Z"
        })
    }

    fn bytes(value: &Value) -> Vec<u8> {
        json::to_deterministic_pretty(value).into_bytes()
    }

    fn whole(text: &[u8], decode: bool) -> Option<CheckedSeries> {
        let value = parse_package_json(text, Path::new("file.json")).ok()?;
        if !measurement_series_file_schema().issues(&value).is_empty() {
            return None;
        }
        Some(from_whole_value(value, decode).unwrap())
    }

    #[test]
    fn the_quick_reading_equals_the_whole_reading_of_a_valid_file() {
        let file = valid_file();
        assert!(measurement_series_file_schema().issues(&file).is_empty());
        for decode in [false, true] {
            let quick = quick_read(&bytes(&file), decode).expect("valid file");
            assert_eq!(Some(quick.clone()), whole(&bytes(&file), decode));
            assert_eq!((quick.record_count, quick.revision_count), (3, 4));
            assert_eq!((quick.deletion_count(), quick.import_key_count()), (1, 2));
            assert_eq!(quick.records.len(), if decode { 3 } else { 0 });
            if decode {
                let records: Vec<CurrentRecord> = file["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| CurrentRecord::from_value(item).unwrap())
                    .collect();
                assert_eq!(quick.records, records);
                let revisions: Vec<RecordRevision> = file["record_revisions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| RecordRevision::from_value(item).unwrap())
                    .collect();
                assert_eq!(quick.revisions, revisions);
            }
        }
    }

    type Mutation = (&'static str, Box<dyn Fn(&mut Value)>);

    /// Every broken file goes to the whole reading, which reports exactly the
    /// issues the series schema reports.
    #[test]
    fn a_broken_file_reports_the_issues_of_the_whole_reading() {
        let mutations: Vec<Mutation> = vec![
            ("sample time", Box::new(|file| file["records"][0]["data"]["recorded_at"] = value!("not a time"))),
            ("outside bounds", Box::new(|file| file["end_at"] = value!("2026-08-09T11:13:00.000Z"))),
            (
                "revision gap",
                Box::new(|file| {
                    file["record_revisions"].as_array_mut().unwrap().remove(1);
                }),
            ),
            ("current differs", Box::new(|file| file["records"][1]["data"]["value"]["standard_value"] = value!(150))),
            ("missing deletion", Box::new(|file| file["record_deletions"] = value!([]))),
            ("unknown key", Box::new(|file| file["extra"] = value!(1))),
            ("records not a list", Box::new(|file| file["records"] = value!({}))),
            ("revisions not a list", Box::new(|file| file["record_revisions"] = value!(null))),
            ("wrong source", Box::new(|file| file["records"][0]["data"]["source_id"] = value!(id(99)))),
            ("shared import key", Box::new(|file| file["record_import_keys"][1]["match_key"] = value!("k1"))),
            ("revision for no record", Box::new(|file| file["record_revisions"][0]["record_id"] = value!(id(77)))),
            (
                "missing records",
                Box::new(|file| {
                    file.as_object_mut().unwrap().remove("records");
                }),
            ),
            ("not an object", Box::new(|file| *file = value!([1, 2]))),
        ];
        for (name, mutate) in mutations {
            let mut file = valid_file();
            mutate(&mut file);
            let text = bytes(&file);
            let parsed = json::parse_bytes(&text).unwrap();
            assert!(quick_read(&text, true).is_none(), "{name}");
            match read_series(&text, Path::new("file.json"), true).unwrap() {
                SeriesRead::Invalid { value: read, issues } => {
                    assert_eq!(read, parsed, "{name}");
                    assert_eq!(issues, measurement_series_file_schema().issues(&parsed), "{name}");
                    assert!(!issues.is_empty(), "{name}");
                }
                SeriesRead::Valid(_) => panic!("{name} was read as valid"),
            }
        }
    }

    /// A repeated top-level key keeps the last value, as `JSON.parse` does.
    /// The quick reading does not follow that, so the whole reading decides.
    #[test]
    fn a_valid_file_the_quick_reading_cannot_follow_is_read_whole() {
        let file = valid_file();
        let text = String::from_utf8(bytes(&file)).unwrap().replacen('{', "{\n  \"quantity\": \"steps\",", 1);
        assert!(quick_read(text.as_bytes(), true).is_none());
        let SeriesRead::Valid(read) = read_series(text.as_bytes(), Path::new("file.json"), true).unwrap() else {
            panic!("the last value of the repeated key is valid");
        };
        assert_eq!(Some(*read), whole(text.as_bytes(), true));
    }

    #[test]
    fn invalid_json_is_refused_as_the_whole_reading_refuses_it() {
        let mut text = bytes(&valid_file());
        text.extend_from_slice(b"x");
        let expected = parse_package_json(&text, Path::new("file.json")).unwrap_err().to_string();
        let Err(error) = read_series(&text, Path::new("file.json"), true) else { panic!("refused") };
        assert_eq!(error.to_string(), expected);
    }
}
