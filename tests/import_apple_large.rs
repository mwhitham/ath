//! A large invented Apple Health export, for measuring the import.
//!
//! Run by hand; it is not part of the ordinary suite:
//!
//!   ATH_LARGE_SAMPLES=1000000 cargo test --release --test import_apple_large -- --ignored --nocapture
//!
//! The export is written under the temporary directory and removed after.
//! The test prints the time spent reading and staging, matching, writing, and
//! validating, so the cost of validating each record twice can be seen.

mod import_support;

use std::io::Write;
use std::time::Instant;

use athleticstandard::import::{self, ImportOptions};
use athleticstandard::json;
use athleticstandard::model::*;
use import_support::{direct, new_store, TempDir};

fn write_export(path: &std::path::Path, samples: usize) {
    let mut out = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<HealthData locale=\"en_US\">").unwrap();
    let start = instant(2024, 1, 1);
    for index in 0..samples {
        // A reading every 37 seconds from two writers.
        let at = start + index as i64 * 37_000;
        let writer = if index % 5 == 0 { "Chest Strap" } else { "Apple Watch" };
        let text = local(at);
        writeln!(
            out,
            " <Record type=\"HKQuantityTypeIdentifierHeartRate\" sourceName=\"{writer}\" sourceVersion=\"10.2\" device=\"&lt;&lt;HKDevice: 0x1&gt;, name:Apple Watch, manufacturer:Apple Inc., model:Watch, hardware:Watch6,2, software:10.2&gt;\" unit=\"count/min\" creationDate=\"{text}\" startDate=\"{text}\" endDate=\"{text}\" value=\"{}\"/>",
            55 + index % 60
        )
        .unwrap();
        if index % 1000 == 0 {
            writeln!(
                out,
                " <Record type=\"HKQuantityTypeIdentifierRestingHeartRate\" sourceName=\"Apple Watch\" unit=\"count/min\" startDate=\"{text}\" endDate=\"{text}\" value=\"{}\"/>",
                48 + index % 7
            )
            .unwrap();
        }
    }
    writeln!(out, "</HealthData>").unwrap();
}

fn instant(year: i64, month: u32, day: u32) -> i64 {
    athleticstandard::model::time::days_from_civil(year, month, day) * 86_400_000
}

fn local(millis: i64) -> String {
    // Written in -07:00, as Apple writes local time with its offset.
    let shifted = athleticstandard::model::time::format_utc_millis(millis - 7 * 3_600_000);
    format!("{} {} -0700", &shifted[0..10], &shifted[11..19])
}

/// The largest resident size `ps` reports for this process while `work` runs.
fn peak_resident_kib<T>(work: impl FnOnce() -> T) -> (T, u64) {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    let done = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(AtomicU64::new(0));
    let sampler = {
        let (done, peak) = (done.clone(), peak.clone());
        let pid = std::process::id().to_string();
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                if let Ok(output) = std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid]).output() {
                    if let Ok(kib) = String::from_utf8_lossy(&output.stdout).trim().parse::<u64>() {
                        peak.fetch_max(kib, Ordering::Relaxed);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        })
    };
    let result = work();
    done.store(true, Ordering::Relaxed);
    sampler.join().unwrap();
    (result, peak.load(Ordering::Relaxed))
}

#[test]
#[ignore]
fn a_large_export_imports_in_bounded_memory() {
    let samples: usize = std::env::var("ATH_LARGE_SAMPLES").ok().and_then(|text| text.parse().ok()).unwrap_or(200_000);
    let directory = TempDir::new();
    let export = directory.path().join("export.xml");
    let started = Instant::now();
    write_export(&export, samples);
    let bytes = std::fs::metadata(&export).unwrap().len();
    println!("wrote {samples} samples, {bytes} bytes, in {:.1?}", started.elapsed());

    let mut store = new_store(directory.path());
    let ((), peak) = peak_resident_kib(|| import_once(&mut store, &export));
    println!("peak resident size while previewing and writing: {:.0} MiB", peak as f64 / 1024.0);
    measure_validation(&store);
}

/// The Apple reader, timed: reading the export and staging each candidate.
struct TimedReader(std::cell::Cell<f64>);

impl import::ProviderReader for TimedReader {
    fn read(
        &self,
        export: &import::DetectedExport,
        sink: &mut import::CandidateSink<'_>,
    ) -> athleticstandard::Result<()> {
        let started = Instant::now();
        let result = import::apple::AppleReader.read(export, sink);
        self.0.set(started.elapsed().as_secs_f64());
        result
    }
}

fn import_once(store: &mut athleticstandard::store::Store, export: &std::path::Path) {
    let options = ImportOptions { received_at: Some("2026-10-13T12:00:00.000Z".into()) };
    let started = Instant::now();
    let reader = TimedReader(std::cell::Cell::new(0.0));
    let detected = import::detect_export(export).unwrap();
    let preview = import::preview_export(store, &reader, &detected, &options).unwrap();
    let preview_time = started.elapsed();
    println!(
        "preview: {} new records in {preview_time:.1?} ({:.1} µs each); reading and staging {:.1} s, matching {:.1} s",
        preview.new_record_count(),
        preview_time.as_secs_f64() * 1e6 / preview.new_record_count() as f64,
        reader.0.get(),
        preview_time.as_secs_f64() - reader.0.get()
    );
    let started = Instant::now();
    let result = preview.apply(store).unwrap();
    let apply_time = started.elapsed();
    println!(
        "apply: {} records in {apply_time:.1?} ({:.1} µs each)",
        result.new_record_count,
        apply_time.as_secs_f64() * 1e6 / result.new_record_count as f64
    );
}

/// Time one validation of a stored sample, as the sink and the write each do.
fn measure_validation(store: &athleticstandard::store::Store) {
    let (record_id, data_json, count): (String, String, i64) = direct(store.path())
        .query_row(
            "SELECT records.record_id, record_revisions.record_json,
                    (SELECT COUNT(*) FROM records)
               FROM records JOIN record_revisions USING (record_id)
              WHERE records.record_type = 'measurement_sample' LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let data = json::parse(&data_json).unwrap();
    let mut record = json::Map::new();
    record.insert("record_id".into(), json::Value::String(record_id));
    record.insert("record_type".into(), json::Value::String("measurement_sample".into()));
    record.insert("current_revision".into(), json::Value::from(1));
    record.insert("created_at".into(), json::Value::String("2026-10-13T12:00:00.000Z".into()));
    record.insert("is_deleted".into(), json::Value::Bool(false));
    record.insert("data".into(), data.clone());
    let record = json::Value::Object(record);
    let rounds = 100_000;
    let started = Instant::now();
    for _ in 0..rounds {
        CurrentRecord::from_value(&record).unwrap();
    }
    let validate = started.elapsed().as_secs_f64() * 1e6 / rounds as f64;
    let started = Instant::now();
    for _ in 0..rounds {
        RecordData::decode(RecordType::MeasurementSample, &data).unwrap();
    }
    let decode = started.elapsed().as_secs_f64() * 1e6 / rounds as f64;
    println!("one complete-record validation: {validate:.2} µs; one data decode: {decode:.2} µs");
    let records = count as f64;
    println!(
        "validation at staging: {:.1} s; decode and validation again at the write: {:.1} s",
        validate * records / 1e6,
        (validate + decode) * records / 1e6
    );
}
