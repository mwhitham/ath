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
//! `stream_series` reads the file from disk in chunks, so neither the file
//! nor its tree is held. It walks the top level itself and hands each value
//! and each list item to serde_json as one slice. It hashes the bytes as they
//! pass, and applies the nesting limit as it finds where each value ends,
//! counting the brackets around the value. Each decoded item goes to a sink
//! as soon as it is checked: a restore writes it at once, records in a first
//! pass over the file and revisions in a second. What is kept per item is a
//! few short facts and a 32-byte digest of its data, which the history rules
//! compare.
//!
//! The quick reading only decides that a file is valid. A file with any
//! problem, or one it cannot follow (a repeated top-level key, a list that is
//! not an array), is read again whole, which reports the exact issues in the
//! 2.1.0 order. So the result never differs from the whole reading.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use sha2::{Digest, Sha256};

use super::load::parse_package_json;
use crate::error::{Error, Result};
use crate::files::{self, durable};
use crate::input_policy::{MAX_PACKAGE_DOCUMENT_BYTES, MAX_PACKAGE_JSON_NESTING};
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

/// Where a quick reading sends each decoded item, as soon as it is checked.
pub(crate) trait ItemSink {
    fn record(&mut self, record: CurrentRecord) -> Result<()>;
    fn revision(&mut self, revision: RecordRevision) -> Result<()>;
}

/// Keeps every decoded item: the in-memory reading of `read_series`.
#[derive(Default)]
struct Collect {
    records: Vec<CurrentRecord>,
    revisions: Vec<RecordRevision>,
}

impl ItemSink for Collect {
    fn record(&mut self, record: CurrentRecord) -> Result<()> {
        self.records.push(record);
        Ok(())
    }

    fn revision(&mut self, revision: RecordRevision) -> Result<()> {
        self.revisions.push(revision);
        Ok(())
    }
}

/// The quick reading of bytes already in memory. `None` means: read the
/// file whole.
fn quick_read(bytes: &[u8], decode: bool) -> Option<CheckedSeries> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut collect = Collect::default();
    let sink: Option<&mut dyn ItemSink> = if decode { Some(&mut collect) } else { None };
    let mut checked = scan(serde_json::Deserializer::from_str(text), sink).ok()??;
    checked.records = collect.records;
    checked.revisions = collect.revisions;
    Some(checked)
}

/// A series file read from disk one item at a time.
pub(crate) struct StreamedSeries {
    /// The checked file. Its decoded items went to the sink, so its own
    /// record and revision lists are empty.
    pub checked: CheckedSeries,
    pub sha256: String,
    pub byte_count: u64,
}

/// Read one series file from disk one item at a time, without holding the
/// file or its JSON tree. With a sink, the file is read twice: the first
/// pass sends each checked record, the second each checked revision.
///
/// `Ok(None)` means the file has a problem, or cannot be followed this way;
/// the caller reads it whole, which reports the exact problem. An error is
/// one the sink returned.
pub(crate) fn stream_series(path: &Path, sink: Option<&mut dyn ItemSink>) -> Result<Option<StreamedSeries>> {
    // The checks `load::safe_read_bytes` makes before it reads.
    if files::refuse_symbolic_link_components(path, false).is_err() {
        return Ok(None);
    }
    let Ok(metadata) = std::fs::symlink_metadata(path) else { return Ok(None) };
    if !metadata.is_file() || metadata.len() > MAX_PACKAGE_DOCUMENT_BYTES {
        return Ok(None);
    }
    let Some(sink) = sink else {
        let Some(pass) = read_pass(path, None, None)? else { return Ok(None) };
        let checked = finish_parts(pass.shell, pass.facts, pass.record_count, pass.revision_count);
        return Ok(checked.map(|checked| StreamedSeries { checked, sha256: pass.sha256, byte_count: pass.byte_count }));
    };
    // Records first, then revisions, as a whole-file restore writes them. A
    // parent row then exists before every row that refers to it, so SQLite
    // has no deferred foreign-key violation to look up as each parent row
    // arrives. Each pass hashes the whole file; both must read the same bytes.
    let Some(records) = read_pass(path, Some(List::Revisions), Some(&mut *sink))? else { return Ok(None) };
    let Some(revisions) = read_pass(path, Some(List::Records), Some(sink))? else { return Ok(None) };
    if records.sha256 != revisions.sha256
        || records.byte_count != revisions.byte_count
        || records.shell != revisions.shell
    {
        return Ok(None);
    }
    let facts = HistoryFacts { revisions: revisions.facts.revisions, ..records.facts };
    let checked = finish_parts(records.shell, facts, records.record_count, revisions.revision_count);
    Ok(checked.map(|checked| StreamedSeries { checked, sha256: records.sha256, byte_count: records.byte_count }))
}

/// What one pass over a file found.
struct Pass {
    shell: Map<String, Value>,
    facts: HistoryFacts,
    record_count: usize,
    revision_count: usize,
    sha256: String,
    byte_count: u64,
}

/// Read the file once, parsing every list except `skip`. `Ok(None)` means:
/// read the file whole. An error is one the sink returned.
fn read_pass(path: &Path, skip: Option<List>, sink: Option<&mut dyn ItemSink>) -> Result<Option<Pass>> {
    let Ok(file) = File::open(path) else { return Ok(None) };
    let mut reader = HashingReader::new(file);
    let mut scan = Scan { sink, skip, ..Scan::default() };
    let walked = walk_file(&mut Chunks::new(&mut reader, STREAM_BUFFER_BYTES), &mut scan);
    if let Some(error) = scan.sink_error.take() {
        return Err(error);
    }
    if walked.is_none() {
        return Ok(None);
    }
    let Scan { shell, facts, record_count, revision_count, .. } = scan;
    Ok(Some(Pass {
        shell,
        facts,
        record_count,
        revision_count,
        sha256: durable::hex(&reader.hasher.finalize()),
        byte_count: reader.bytes,
    }))
}

/// Bytes read from disk at a time.
const STREAM_BUFFER_BYTES: usize = 256 * 1024;

/// Hashes and counts what it reads.
struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
    bytes: u64,
}

impl<R> HashingReader<R> {
    fn new(inner: R) -> Self {
        HashingReader { inner, hasher: Sha256::new(), bytes: 0 }
    }
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.hasher.update(&buffer[..count]);
        self.bytes += count as u64;
        Ok(count)
    }
}

/// Parse and check a series file. `Ok(None)` means: read the file whole.
/// An error is one the sink returned.
fn scan<'de, R: serde_json::de::Read<'de>>(
    mut deserializer: serde_json::Deserializer<R>,
    sink: Option<&mut dyn ItemSink>,
) -> Result<Option<CheckedSeries>> {
    let mut scan = Scan { sink, ..Scan::default() };
    let parsed = de::Deserializer::deserialize_map(&mut deserializer, FileVisitor(&mut scan));
    if let Some(error) = scan.sink_error.take() {
        return Err(error);
    }
    if parsed.is_err() || deserializer.end().is_err() {
        return Ok(None);
    }
    Ok(finish(scan))
}

/// The checks that need the whole file: the declared fields, the short
/// lists, and the history and series rules over the facts. `None` means:
/// read the file whole.
fn finish(scan: Scan<'_>) -> Option<CheckedSeries> {
    let Scan { shell, facts, record_count, revision_count, .. } = scan;
    finish_parts(shell, facts, record_count, revision_count)
}

fn finish_parts(
    shell: Map<String, Value>,
    facts: HistoryFacts,
    record_count: usize,
    revision_count: usize,
) -> Option<CheckedSeries> {
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
    Some(CheckedSeries { shell, record_count, revision_count, facts, records: Vec::new(), revisions: Vec::new() })
}

/// Parse a series file read from disk in chunks. The top level is walked
/// here, strictly; each top-level value and each list item is handed to
/// serde_json as one slice, which is as fast as parsing text in memory. Any
/// irregular byte gives `Ok(None)`, and the whole reading decides.
/// An error is one the sink returned.
#[cfg(test)]
fn scan_in_chunks(
    reader: impl Read,
    chunk_bytes: usize,
    sink: Option<&mut dyn ItemSink>,
) -> Result<Option<CheckedSeries>> {
    let mut scan = Scan { sink, ..Scan::default() };
    let walked = walk_file(&mut Chunks::new(reader, chunk_bytes), &mut scan);
    if let Some(error) = scan.sink_error.take() {
        return Err(error);
    }
    if walked.is_none() {
        return Ok(None);
    }
    Ok(finish(scan))
}

/// The top level of a series file: `{`, keys and values, `}`, then only
/// whitespace. `None` when anything differs from strict JSON or from what
/// the quick reading follows.
fn walk_file<R: Read>(chunks: &mut Chunks<R>, scan: &mut Scan<'_>) -> Option<()> {
    chunks.skip_whitespace()?;
    chunks.expect(b'{')?;
    chunks.skip_whitespace()?;
    if chunks.peek()? == b'}' {
        chunks.advance(1);
    } else {
        loop {
            if chunks.peek()? != b'"' {
                return None;
            }
            let key: String = serde_json::from_slice(chunks.value(1)?).ok()?;
            chunks.consume_value();
            if scan.shell.contains_key(&key) {
                return None;
            }
            chunks.skip_whitespace()?;
            chunks.expect(b':')?;
            chunks.skip_whitespace()?;
            let list = match key.as_str() {
                "records" => Some(List::Records),
                "record_revisions" => Some(List::Revisions),
                _ => None,
            };
            match list {
                Some(list) => {
                    walk_list(chunks, scan, list)?;
                    scan.shell.insert(key, Value::Array(Vec::new()));
                }
                None => {
                    let value: Value = serde_json::from_slice(chunks.value(1)?).ok()?;
                    chunks.consume_value();
                    scan.shell.insert(key, value);
                }
            }
            chunks.skip_whitespace()?;
            match chunks.peek()? {
                b',' => {
                    chunks.advance(1);
                    chunks.skip_whitespace()?;
                }
                b'}' => {
                    chunks.advance(1);
                    break;
                }
                _ => return None,
            }
        }
    }
    chunks.skip_whitespace()?;
    chunks.at_end().then_some(())
}

/// One long list: `[`, objects separated by commas, `]`.
fn walk_list<R: Read>(chunks: &mut Chunks<R>, scan: &mut Scan<'_>, list: List) -> Option<()> {
    chunks.expect(b'[')?;
    chunks.skip_whitespace()?;
    if chunks.peek()? == b']' {
        chunks.advance(1);
        return Some(());
    }
    let parse = scan.skip != Some(list);
    loop {
        if chunks.peek()? != b'{' {
            return None;
        }
        if parse {
            let item: Value = serde_json::from_slice(chunks.value(2)?).ok()?;
            chunks.consume_value();
            scan.item(list, item).ok()?;
        } else {
            chunks.value(2)?;
            chunks.consume_value();
        }
        chunks.skip_whitespace()?;
        match chunks.peek()? {
            b',' => {
                chunks.advance(1);
                chunks.skip_whitespace()?;
            }
            b']' => {
                chunks.advance(1);
                return Some(());
            }
            _ => return None,
        }
    }
}

/// A file read in chunks. Bytes before `start` are consumed; `end` marks the
/// end of the value found by `value`.
struct Chunks<R> {
    reader: R,
    chunk_bytes: usize,
    buffer: Vec<u8>,
    start: usize,
    end: usize,
    eof: bool,
}

impl<R: Read> Chunks<R> {
    fn new(reader: R, chunk_bytes: usize) -> Self {
        Chunks { reader, chunk_bytes, buffer: Vec::with_capacity(chunk_bytes), start: 0, end: 0, eof: false }
    }

    /// Read another chunk. `false` at the end of the file or on an error.
    fn fill(&mut self) -> bool {
        if self.eof {
            return false;
        }
        if self.start > 0 && self.start * 2 >= self.buffer.len() {
            self.buffer.drain(..self.start);
            self.end -= self.start.min(self.end);
            self.start = 0;
        }
        let old = self.buffer.len();
        self.buffer.resize(old + self.chunk_bytes, 0);
        match self.reader.read(&mut self.buffer[old..]) {
            Ok(0) | Err(_) => {
                self.buffer.truncate(old);
                self.eof = true;
                false
            }
            Ok(count) => {
                self.buffer.truncate(old + count);
                true
            }
        }
    }

    fn peek(&mut self) -> Option<u8> {
        while self.start >= self.buffer.len() {
            if !self.fill() {
                return None;
            }
        }
        Some(self.buffer[self.start])
    }

    fn advance(&mut self, count: usize) {
        self.start += count;
    }

    fn expect(&mut self, byte: u8) -> Option<()> {
        (self.peek()? == byte).then(|| self.advance(1))
    }

    /// JSON whitespace: space, tab, line feed, carriage return.
    fn skip_whitespace(&mut self) -> Option<()> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => self.advance(1),
                _ => return Some(()),
            }
        }
    }

    fn at_end(&mut self) -> bool {
        self.peek().is_none() && self.eof
    }

    /// The bytes of the value that starts here: an object or array to its
    /// closing bracket, a string to its closing quote, or anything else to
    /// the next delimiter. serde_json then decides whether they are valid.
    ///
    /// `depth` is the number of brackets open around the value. `None` when
    /// the value is cut off, or when it opens brackets past the package
    /// nesting limit, counted as `load::check_nesting` counts a whole file.
    fn value(&mut self, depth: usize) -> Option<&[u8]> {
        let first = self.peek()?;
        // Counted from `start`, which `fill` may move to the front.
        let mut offset = 0usize;
        if !matches!(first, b'{' | b'[' | b'"') {
            let delimiter = |byte: &u8| matches!(byte, b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r');
            loop {
                let rest = &self.buffer[self.start + offset..];
                if let Some(found) = rest.iter().position(delimiter) {
                    offset += found;
                    break;
                }
                offset += rest.len();
                // A scalar may end at the end of the file.
                if !self.fill() {
                    break;
                }
            }
            self.end = self.start + offset;
            return Some(&self.buffer[self.start..self.end]);
        }
        let outside = depth;
        let mut depth = depth;
        let mut in_string = false;
        loop {
            while self.start + offset >= self.buffer.len() {
                if !self.fill() {
                    return None;
                }
            }
            let rest = &self.buffer[self.start + offset..];
            if in_string {
                let Some(found) = rest.iter().position(|byte| matches!(byte, b'"' | b'\\')) else {
                    offset += rest.len();
                    continue;
                };
                if rest[found] == b'\\' {
                    // The escaped byte is passed over, even before it is read.
                    offset += found + 2;
                    continue;
                }
                offset += found + 1;
                in_string = false;
                if first == b'"' {
                    break;
                }
                continue;
            }
            let Some(found) = rest.iter().position(|byte| matches!(byte, b'"' | b'{' | b'[' | b'}' | b']')) else {
                offset += rest.len();
                continue;
            };
            let byte = rest[found];
            offset += found + 1;
            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > MAX_PACKAGE_JSON_NESTING {
                        return None;
                    }
                }
                _ => {
                    depth -= 1;
                    if depth == outside {
                        break;
                    }
                }
            }
        }
        self.end = self.start + offset;
        Some(&self.buffer[self.start..self.end])
    }

    fn consume_value(&mut self) {
        self.start = self.end;
    }
}

#[derive(Default)]
struct Scan<'s> {
    sink: Option<&'s mut dyn ItemSink>,
    /// An error the sink returned. It stops the reading and is reported as is.
    sink_error: Option<Error>,
    /// A list named here is only walked, not parsed: a restore reads the
    /// records in one pass and the revisions in the next.
    skip: Option<List>,
    shell: Map<String, Value>,
    facts: HistoryFacts,
    record_count: usize,
    revision_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum List {
    Records,
    Revisions,
}

impl Scan<'_> {
    /// Check, reduce, and decode one item, and hand it to the sink. An error
    /// stops the quick reading.
    fn item(&mut self, list: List, item: Value) -> std::result::Result<(), &'static str> {
        let delivered = match list {
            List::Records => {
                if !measurement_sample_record_schema().issues(&item).is_empty() {
                    return Err("record");
                }
                self.facts.records.push(RecordFacts::of(&item));
                self.record_count += 1;
                match self.sink.as_deref_mut() {
                    None => Ok(()),
                    Some(sink) => {
                        let record = CurrentRecord::from_checked(&item).map_err(|_| "record")?;
                        drop(item);
                        sink.record(record)
                    }
                }
            }
            List::Revisions => {
                if !measurement_sample_revision_schema().issues(&item).is_empty() {
                    return Err("revision");
                }
                self.facts.revisions.push(RevisionFacts::of(&item));
                self.revision_count += 1;
                match self.sink.as_deref_mut() {
                    None => Ok(()),
                    Some(sink) => {
                        let revision = RecordRevision::from_checked(&item).map_err(|_| "revision")?;
                        drop(item);
                        sink.revision(revision)
                    }
                }
            }
        };
        delivered.map_err(|error| {
            self.sink_error = Some(error);
            "sink"
        })
    }
}

struct FileVisitor<'a, 's>(&'a mut Scan<'s>);

impl<'de> Visitor<'de> for FileVisitor<'_, '_> {
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

struct Items<'a, 's> {
    scan: &'a mut Scan<'s>,
    list: List,
}

impl<'de> DeserializeSeed<'de> for Items<'_, '_> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> std::result::Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Items<'_, '_> {
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

    fn write_temporary(name: &str, bytes: &[u8]) -> (std::path::PathBuf, std::path::PathBuf) {
        let directory = files::make_temporary_directory("ath-series-stream-").unwrap();
        let path = directory.join(name);
        std::fs::write(&path, bytes).unwrap();
        (directory, path)
    }

    /// Reading from disk gives the in-memory reading's result, the file's
    /// hash and size, and the same decoded items, in file order.
    #[test]
    fn streaming_from_disk_equals_the_reading_in_memory() {
        let text = bytes(&valid_file());
        let (directory, path) = write_temporary("valid.json", &text);
        let mut collect = Collect::default();
        let streamed = stream_series(&path, Some(&mut collect)).unwrap().expect("valid file");
        let whole = quick_read(&text, true).unwrap();
        assert_eq!(streamed.sha256, durable::sha256_hex(&text));
        assert_eq!(streamed.byte_count, text.len() as u64);
        assert_eq!(collect.records, whole.records);
        assert_eq!(collect.revisions, whole.revisions);
        assert_eq!(streamed.checked, CheckedSeries { records: Vec::new(), revisions: Vec::new(), ..whole });
        std::fs::remove_dir_all(directory).unwrap();
    }

    /// Every broken file, and a file nested past the package limit, makes the
    /// streamed reading step aside for the whole reading.
    #[test]
    fn a_broken_or_too_deep_file_is_left_to_the_whole_reading() {
        let mut broken = valid_file();
        broken["records"][0]["data"]["recorded_at"] = value!("not a time");
        let deep = format!("{}{}", "[".repeat(MAX_PACKAGE_JSON_NESTING + 1), "]".repeat(MAX_PACKAGE_JSON_NESTING + 1));
        for (name, text) in [("broken.json", bytes(&broken)), ("deep.json", deep.into_bytes())] {
            let (directory, path) = write_temporary(name, &text);
            let mut collect = Collect::default();
            assert!(stream_series(&path, Some(&mut collect)).unwrap().is_none(), "{name}");
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    fn chunked(text: &[u8], chunk_bytes: usize) -> Option<(CheckedSeries, Vec<CurrentRecord>, Vec<RecordRevision>)> {
        let mut collect = Collect::default();
        let checked = scan_in_chunks(text, chunk_bytes, Some(&mut collect)).unwrap()?;
        Some((checked, collect.records, collect.revisions))
    }

    /// Values that cross chunk boundaries read the same at every chunk size,
    /// in the compact and the indented layout, and with escaped key names.
    #[test]
    fn every_chunk_size_reads_the_same_file() {
        let pretty = bytes(&valid_file());
        let compact = json::to_string(&valid_file()).into_bytes();
        let escaped =
            String::from_utf8(pretty.clone()).unwrap().replacen("\"records\"", "\"rec\\u006frds\"", 1).into_bytes();
        for text in [&pretty, &compact, &escaped] {
            let expected = quick_read(text, true).expect("valid file");
            for chunk_bytes in [1, 2, 3, 7, 64, 1000, 1 << 20] {
                let (checked, records, revisions) = chunked(text, chunk_bytes).expect("valid file");
                assert_eq!(records, expected.records, "{chunk_bytes}");
                assert_eq!(revisions, expected.revisions, "{chunk_bytes}");
                assert_eq!(checked.facts, expected.facts, "{chunk_bytes}");
                assert_eq!(checked.record_count, expected.record_count, "{chunk_bytes}");
            }
        }
    }

    /// Text that is not strict JSON is left to the whole reading, which
    /// refuses it, at every chunk size.
    #[test]
    fn irregular_text_is_left_to_the_whole_reading() {
        let pretty = String::from_utf8(bytes(&valid_file())).unwrap();
        let cases = [
            ("trailing comma in a list", pretty.replacen("\n  ],\n  \"source_id\"", ",\n  ],\n  \"source_id\"", 1)),
            ("missing comma between items", pretty.replacen("},\n    {", "}\n    {", 1)),
            ("trailing comma in the object", pretty.replacen("\n}", ",\n}", 1)),
            ("text after the object", format!("{pretty}x")),
            ("byte order mark", format!("\u{feff}{pretty}")),
            ("unquoted key", pretty.replacen("\"quantity\"", "quantity", 1)),
            ("number with two points", pretty.replacen("\"current_revision\": 1", "\"current_revision\": 1.0.0", 1)),
            ("unclosed list", pretty.replacen("\n}", "", 1).replacen("\n  ]\n", "\n", 1)),
        ];
        for (name, text) in cases {
            assert_ne!(text, pretty, "{name}");
            assert!(parse_package_json(text.as_bytes(), Path::new("file.json")).is_err(), "{name}");
            for chunk_bytes in [1, 5, 4096] {
                assert!(chunked(text.as_bytes(), chunk_bytes).is_none(), "{name} at {chunk_bytes}");
            }
        }
    }

    /// An error from the sink stops the reading and is returned as it is. A
    /// restore reads records first.
    #[test]
    fn a_sink_error_is_returned() {
        struct Refuse;
        impl ItemSink for Refuse {
            fn record(&mut self, _: CurrentRecord) -> Result<()> {
                Err(Error::refused("no records"))
            }
            fn revision(&mut self, _: RecordRevision) -> Result<()> {
                Err(Error::refused("no revisions"))
            }
        }
        let (directory, path) = write_temporary("valid.json", &bytes(&valid_file()));
        let Err(error) = stream_series(&path, Some(&mut Refuse)) else { panic!("the sink refused") };
        assert_eq!(error.to_string(), "no records");
        std::fs::remove_dir_all(directory).unwrap();
    }

    /// A value ends at its own closing bracket or quote at every chunk size.
    /// Brackets and quotes inside strings, and escaped quotes, do not count.
    #[test]
    fn a_value_ends_where_its_brackets_close() {
        let cases: [(&[u8], &[u8]); 5] = [
            (br#"{"a":"x}\"]\\","b":[1,{"c":"\\\""}]} ,"#, br#"{"a":"x}\"]\\","b":[1,{"c":"\\\""}]}"#),
            (br#""text with \" and \\ and ]" ,"#, br#""text with \" and \\ and ]""#),
            (b"12.5e3,", b"12.5e3"),
            (b"true", b"true"),
            (b"[[],{}]]", b"[[],{}]"),
        ];
        for (text, expected) in cases {
            for chunk_bytes in [1, 2, 3, 7, 1000] {
                assert_eq!(Chunks::new(text, chunk_bytes).value(2), Some(expected), "{chunk_bytes}");
            }
        }
    }

    /// A value may open brackets up to the package nesting limit, counted
    /// with the brackets around it, and no further. A value that is cut off
    /// is not a value.
    #[test]
    fn a_value_past_the_nesting_limit_or_cut_off_is_refused() {
        let nested = |count: usize| format!("{}{}", "[".repeat(count), "]".repeat(count)).into_bytes();
        let at_limit = nested(MAX_PACKAGE_JSON_NESTING - 2);
        let past = nested(MAX_PACKAGE_JSON_NESTING - 1);
        for chunk_bytes in [1, 7, 1 << 20] {
            assert_eq!(Chunks::new(&at_limit[..], chunk_bytes).value(2), Some(&at_limit[..]));
            assert_eq!(Chunks::new(&past[..], chunk_bytes).value(2), None);
            for cut in [&b"{\"a\":[1,2"[..], b"\"open", b"\"escape\\"] {
                assert_eq!(Chunks::new(cut, chunk_bytes).value(2), None);
            }
        }
    }
}
