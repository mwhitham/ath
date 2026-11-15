//! Native library latency for the nine release measurements.
//!
//! This is the native counterpart of `scripts/performance/run-benchmarks.ts`.
//! It calls the library directly, in one process, with one open read-only
//! connection for the reads. Each read runs once as a discarded warmup and
//! then `--repetitions` times. Export, check, initial import, and incremental
//! import run `--bulk-repetitions` times each.
//!
//! The operations and their parameters match the 2.1.0 runner:
//!
//! - `latest_value`: the newest `hrv_sdnn` sample of source A.
//! - `evidence_28d`: evidence for `fran` as of 2026-10-13T23:59:59.999Z, 28 days.
//! - `baseline_90d`: `hrv_sdnn` baseline for source A over the last 90 days.
//! - `workout_drift`: heart-rate drift of the fixture workout for source A.
//! - `cross_source_comparison`: `hrv_sdnn`, source A against source B, 90 days.
//! - `full_json_export`: a complete package into a new folder.
//! - `database_check`: the complete database check.
//! - `initial_import`: create an empty database, then preview and apply the package.
//! - `incremental_import`: preview and apply a one-record package from the same database.
//!
//! With `--plans`, one extra pass records every distinct read statement each
//! operation runs, with its first bound arguments, and its SQLite query plan.
//! That pass is not timed.
//!
//! Run from the repository root:
//!
//! ```sh
//! cargo build --release --offline --example performance-library
//! target/release/examples/performance-library --database <migrated.ath.sqlite> \
//!   --package <package folder> --delta <one-record package folder> \
//!   --work <empty folder> --output <report.json> --repetitions 50 --bulk-repetitions 3 --plans
//! ```

use std::collections::BTreeMap;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use athleticstandard::calculate::{calculate_range, CalculateOptions, Operation};
use athleticstandard::model::time::{format_utc_millis, parse_utc_timestamp};
use athleticstandard::model::SeriesQuantity;
use athleticstandard::package::{apply_package_import, export_package, preview_package_import, ExportOptions};
use athleticstandard::prediction::{build_evidence, EvidenceRequest};
use athleticstandard::store::check::check_database;
use athleticstandard::store::query::{query_series, Order, SeriesQuery};
use athleticstandard::store::{CreateOptions, OpenOptions, Store};
use rusqlite::ffi;
use serde_json::{json, Value};

const SOURCE_A: &str = "01890f3e-e7d3-7cc0-88c8-000000000003";
const SOURCE_B: &str = "01890f3e-e7d3-7cc0-88c8-000000000064";
const WORKOUT_SESSION: &str = "01890f3e-e7d3-7cc0-88c8-00000000000a";
const EVIDENCE_AS_OF: &str = "2026-10-13T23:59:59.999Z";
const CREATED_AT: &str = "2026-10-14T00:00:00.000Z";
const DAY_MS: i64 = 86_400_000;

type Failure = Box<dyn std::error::Error>;

struct Options {
    database: PathBuf,
    package: PathBuf,
    delta: PathBuf,
    work: PathBuf,
    output: PathBuf,
    repetitions: usize,
    bulk_repetitions: usize,
    plans: bool,
}

fn options() -> Result<Options, Failure> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| -> Option<String> {
        args.iter().position(|arg| arg == name).and_then(|index| args.get(index + 1)).cloned()
    };
    let required = |name: &str| -> Result<PathBuf, Failure> {
        value(name).map(PathBuf::from).ok_or_else(|| format!("{name} is required").into())
    };
    let count = |name: &str, default: usize| -> Result<usize, Failure> {
        match value(name) {
            None => Ok(default),
            Some(text) => match text.parse::<usize>() {
                Ok(number) if number > 0 => Ok(number),
                _ => Err(format!("{name} must be a positive whole number").into()),
            },
        }
    };
    Ok(Options {
        database: required("--database")?,
        package: required("--package")?,
        delta: required("--delta")?,
        work: required("--work")?,
        output: required("--output")?,
        repetitions: count("--repetitions", 3)?,
        bulk_repetitions: count("--bulk-repetitions", 1)?,
        plans: args.iter().any(|arg| arg == "--plans"),
    })
}

// ---- Timing ---------------------------------------------------------------

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let index = ((sorted.len() as f64 * fraction).ceil() as usize).saturating_sub(1).min(sorted.len() - 1);
    sorted[index]
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn distribution(samples: &[f64]) -> Value {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    json!({
        "min": round3(sorted[0]),
        "p50": round3(percentile(&sorted, 0.5)),
        "p95": round3(percentile(&sorted, 0.95)),
        "p99": round3(percentile(&sorted, 0.99)),
        "max": round3(sorted[sorted.len() - 1]),
        "mean": round3(sorted.iter().sum::<f64>() / sorted.len() as f64),
    })
}

/// Run `operation` `repetitions` times after an optional discarded warmup.
fn measure(
    id: &str,
    api: &str,
    repetitions: usize,
    warmup: bool,
    mut operation: impl FnMut(usize) -> Result<(), Failure>,
) -> Result<Value, Failure> {
    if warmup {
        operation(0)?;
    }
    let mut samples = Vec::with_capacity(repetitions);
    for run in 1..=repetitions {
        let started = Instant::now();
        operation(run)?;
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    eprintln!("{id}: p50 {:.3} ms over {repetitions}", {
        let mut sorted = samples.clone();
        sorted.sort_by(f64::total_cmp);
        percentile(&sorted, 0.5)
    });
    Ok(json!({
        "id": id,
        "api": api,
        "repetitions": repetitions,
        "warmup": warmup,
        "samples_ms": samples.iter().map(|sample| round3(*sample)).collect::<Vec<_>>(),
        "summary_ms": distribution(&samples),
    }))
}

// ---- Statement capture for query plans -----------------------------------

/// Unexpanded SQL to the first expanded SQL seen while capture is on.
static CAPTURED: Mutex<Option<BTreeMap<String, String>>> = Mutex::new(None);

unsafe extern "C" fn trace(kind: c_uint, _context: *mut c_void, statement: *mut c_void, text: *mut c_void) -> c_int {
    if kind != ffi::SQLITE_TRACE_STMT as c_uint || statement.is_null() {
        return 0;
    }
    // Statements run by triggers arrive as comments; their plans belong to the outer statement.
    let text = if text.is_null() { "" } else { CStr::from_ptr(text as *const c_char).to_str().unwrap_or("") };
    if text.starts_with("--") {
        return 0;
    }
    let statement = statement as *mut ffi::sqlite3_stmt;
    let sql = ffi::sqlite3_sql(statement);
    if sql.is_null() {
        return 0;
    }
    let sql = CStr::from_ptr(sql).to_string_lossy().into_owned();
    let lead = sql.trim_start().to_ascii_uppercase();
    if !(lead.starts_with("SELECT") || lead.starts_with("WITH")) {
        return 0;
    }
    let mut captured = CAPTURED.lock().expect("capture lock");
    let Some(map) = captured.as_mut() else { return 0 };
    if map.contains_key(&sql) {
        return 0;
    }
    let expanded = ffi::sqlite3_expanded_sql(statement);
    if expanded.is_null() {
        return 0;
    }
    let expanded_text = CStr::from_ptr(expanded).to_string_lossy().into_owned();
    ffi::sqlite3_free(expanded as *mut c_void);
    map.insert(sql, expanded_text);
    0
}

fn handle(store: &Store) -> *mut ffi::sqlite3 {
    // SAFETY: the handle is used only while the store is alive.
    unsafe { store.connection().handle() }
}

fn set_trace(handle: *mut ffi::sqlite3, on: bool) {
    // SAFETY: the callback reads only the statement it is given, during the
    // call, and the caller keeps the connection open.
    unsafe {
        if on {
            ffi::sqlite3_trace_v2(handle, ffi::SQLITE_TRACE_STMT as c_uint, Some(trace), std::ptr::null_mut());
        } else {
            ffi::sqlite3_trace_v2(handle, 0, None, std::ptr::null_mut());
        }
    }
}

/// Run `operation` once with statement capture, then explain each statement.
fn capture_plans(
    connection: *mut ffi::sqlite3,
    explain_with: &Store,
    operation: impl FnOnce() -> Result<(), Failure>,
) -> Result<Value, Failure> {
    *CAPTURED.lock().expect("capture lock") = Some(BTreeMap::new());
    set_trace(connection, true);
    let result = operation();
    set_trace(connection, false);
    let captured = CAPTURED.lock().expect("capture lock").take().unwrap_or_default();
    result?;
    let mut plans = Vec::new();
    for (sql, expanded) in captured {
        let mut statement = explain_with.connection().prepare(&format!("EXPLAIN QUERY PLAN {expanded}"))?;
        let steps: Vec<String> = statement.query_map([], |row| row.get::<_, String>(3))?.collect::<Result<_, _>>()?;
        plans.push(json!({ "sql": collapse(&sql), "plan": steps }));
    }
    Ok(Value::Array(plans))
}

fn collapse(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---- Memory ---------------------------------------------------------------

#[repr(C)]
#[derive(Default)]
struct RUsage {
    user: [i64; 2],
    system: [i64; 2],
    max_rss: i64,
    rest: [i64; 13],
}

extern "C" {
    fn getrusage(who: c_int, usage: *mut RUsage) -> c_int;
}

/// The process's peak resident set in bytes: kilobytes on Linux, bytes on macOS.
fn peak_rss_bytes() -> i64 {
    let mut usage = RUsage::default();
    // SAFETY: `RUsage` matches `struct rusage` on 64-bit macOS and Linux.
    let status = unsafe { getrusage(0, &mut usage) };
    if status != 0 {
        return -1;
    }
    if cfg!(target_os = "macos") {
        usage.max_rss
    } else {
        usage.max_rss * 1024
    }
}

// ---- The run --------------------------------------------------------------

fn remove_database(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

fn main() -> Result<(), Failure> {
    let options = options()?;
    std::fs::create_dir_all(&options.work)?;
    let store = Store::open(&options.database, OpenOptions { read_only: true, allow_outdated_schema: false })?;
    let connection = store.connection();
    let quantity = SeriesQuantity::parse("hrv_sdnn").ok_or("hrv_sdnn is not a series quantity")?;

    let to: String = connection.query_row(
        "SELECT MAX(recorded_at) FROM measurement_samples WHERE quantity = 'hrv_sdnn' AND source_id = ?1",
        [SOURCE_A],
        |row| row.get(0),
    )?;
    let from = format_utc_millis(parse_utc_timestamp(&to).ok_or("bad recorded_at")? - 90 * DAY_MS);
    let counts: Value = connection.query_row(
        "SELECT (SELECT COUNT(*) FROM records), (SELECT COUNT(*) FROM records WHERE is_deleted = 0),
                (SELECT COUNT(*) FROM records WHERE is_deleted = 1), (SELECT COUNT(*) FROM record_revisions),
                (SELECT COUNT(*) FROM measurement_samples)",
        [],
        |row| {
            Ok(json!({
                "records_total": row.get::<_, i64>(0)?,
                "records_current": row.get::<_, i64>(1)?,
                "records_deleted": row.get::<_, i64>(2)?,
                "record_revisions": row.get::<_, i64>(3)?,
                "measurement_samples": row.get::<_, i64>(4)?,
            }))
        },
    )?;

    let latest = || -> Result<(), Failure> {
        let mut query = SeriesQuery::new(quantity);
        query.source_id = Some(SOURCE_A.into());
        query.order = Order::Descending;
        query.limit = Some(1);
        if query_series(connection, &query)?.len() != 1 {
            return Err("latest value was not found".into());
        }
        Ok(())
    };
    let evidence = || -> Result<(), Failure> {
        let mut request = EvidenceRequest::new("fran", EVIDENCE_AS_OF);
        request.window_days = Some(28);
        build_evidence(connection, &request)?;
        Ok(())
    };
    let baseline = || -> Result<(), Failure> {
        let mut request = CalculateOptions::new(Operation::Baseline, "hrv_sdnn");
        request.source_id = Some(SOURCE_A.into());
        request.from = Some(from.clone());
        request.to = Some(to.clone());
        calculate_range(&store, &request)?;
        Ok(())
    };
    let drift = || -> Result<(), Failure> {
        let mut request = CalculateOptions::new(Operation::WorkoutDrift, "heart_rate");
        request.workout_session_id = Some(WORKOUT_SESSION.into());
        request.source_id = Some(SOURCE_A.into());
        calculate_range(&store, &request)?;
        Ok(())
    };
    let compare = || -> Result<(), Failure> {
        let mut request = CalculateOptions::new(Operation::CompareSources, "hrv_sdnn");
        request.source_id = Some(SOURCE_A.into());
        request.compare_source_id = Some(SOURCE_B.into());
        request.from = Some(from.clone());
        request.to = Some(to.clone());
        calculate_range(&store, &request)?;
        Ok(())
    };

    let reps = options.repetitions;
    let mut operations = vec![
        measure("latest_value", "store::query::query_series", reps, true, |_| latest())?,
        measure("evidence_28d", "prediction::build_evidence", reps, true, |_| evidence())?,
        measure("baseline_90d", "calculate::calculate_range", reps, true, |_| baseline())?,
        measure("workout_drift", "calculate::calculate_range", reps, true, |_| drift())?,
        measure("cross_source_comparison", "calculate::calculate_range", reps, true, |_| compare())?,
    ];

    let bulk = options.bulk_repetitions;
    let export_path = |run: usize| options.work.join(format!("export-{run}"));
    operations.push(measure("full_json_export", "package::export_package", bulk, false, |run| {
        let path = export_path(run);
        let _ = std::fs::remove_dir_all(&path);
        export_package(&store, &path, &ExportOptions::default())?;
        Ok(())
    })?);
    for run in 1..=bulk {
        let _ = std::fs::remove_dir_all(export_path(run));
    }
    operations.push(measure("database_check", "store::check::check_database", bulk, false, |_| {
        let checked = check_database(&store)?;
        if !checked.valid {
            return Err(format!("database check reported {} issue(s)", checked.issues.len()).into());
        }
        Ok(())
    })?);

    let restored_path = |run: usize| options.work.join(format!("restored-{run}.ath.sqlite"));
    let mut restored: Vec<Store> = Vec::new();
    operations.push(measure(
        "initial_import",
        "Store::create + package::preview_package_import + package::apply_package_import",
        bulk,
        false,
        |run| {
            let path = restored_path(run);
            remove_database(&path);
            let mut destination = Store::create(
                &path,
                CreateOptions { created_at: Some(CREATED_AT.into()), ..CreateOptions::default() },
            )?;
            let preview = preview_package_import(&destination, &options.package)?;
            if !preview.restore || !preview.conflicts.is_empty() {
                return Err("initial import did not produce a clean restore".into());
            }
            apply_package_import(&mut destination, &preview, &[])?;
            restored.push(destination);
            Ok(())
        },
    )?);
    let mut targets = restored.into_iter();
    operations.push(measure(
        "incremental_import",
        "package::preview_package_import + package::apply_package_import",
        bulk,
        false,
        |_| {
            let mut destination = targets.next().ok_or("initial import did not leave a restored database")?;
            let preview = preview_package_import(&destination, &options.delta)?;
            if preview.new_record_count != 1 || !preview.conflicts.is_empty() {
                return Err("incremental import did not find one clean new record".into());
            }
            apply_package_import(&mut destination, &preview, &[])?;
            destination.close()?;
            Ok(())
        },
    )?);
    for run in 1..=bulk {
        remove_database(&restored_path(run));
    }

    let mut plans = serde_json::Map::new();
    if options.plans {
        plans.insert("latest_value".into(), capture_plans(handle(&store), &store, latest)?);
        plans.insert("evidence_28d".into(), capture_plans(handle(&store), &store, evidence)?);
        plans.insert("baseline_90d".into(), capture_plans(handle(&store), &store, baseline)?);
        plans.insert("workout_drift".into(), capture_plans(handle(&store), &store, drift)?);
        plans.insert("cross_source_comparison".into(), capture_plans(handle(&store), &store, compare)?);
        let path = options.work.join("export-plans");
        let _ = std::fs::remove_dir_all(&path);
        plans.insert(
            "full_json_export".into(),
            capture_plans(handle(&store), &store, || {
                export_package(&store, &path, &ExportOptions::default())?;
                Ok(())
            })?,
        );
        let _ = std::fs::remove_dir_all(&path);
        plans.insert(
            "database_check".into(),
            capture_plans(handle(&store), &store, || {
                check_database(&store)?;
                Ok(())
            })?,
        );
        let path = options.work.join("restored-plans.ath.sqlite");
        remove_database(&path);
        let mut destination =
            Store::create(&path, CreateOptions { created_at: Some(CREATED_AT.into()), ..CreateOptions::default() })?;
        plans.insert(
            "initial_import".into(),
            capture_plans(handle(&destination), &store, || {
                let preview = preview_package_import(&destination, &options.package)?;
                apply_package_import(&mut destination, &preview, &[])?;
                Ok(())
            })?,
        );
        plans.insert(
            "incremental_import".into(),
            capture_plans(handle(&destination), &store, || {
                let preview = preview_package_import(&destination, &options.delta)?;
                apply_package_import(&mut destination, &preview, &[])?;
                Ok(())
            })?,
        );
        destination.close()?;
        remove_database(&path);
    }

    let pragma = |name: &str| -> Result<Value, Failure> {
        let value: rusqlite::types::Value = connection.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))?;
        Ok(match value {
            rusqlite::types::Value::Integer(number) => json!(number),
            rusqlite::types::Value::Text(text) => json!(text),
            other => json!(format!("{other:?}")),
        })
    };
    let sqlite_version: String = connection.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
    let report = json!({
        "report_version": 1,
        "implementation": "native",
        "library_version": env!("CARGO_PKG_VERSION"),
        "database_path": options.database.display().to_string(),
        "package_path": options.package.display().to_string(),
        "delta_path": options.delta.display().to_string(),
        "counts": counts,
        "parameters": { "from": from, "to": to, "evidence_as_of": EVIDENCE_AS_OF, "evidence_window_days": 28 },
        "sqlite": {
            "version": sqlite_version,
            "journal_mode": pragma("journal_mode")?,
            "page_size": pragma("page_size")?,
            "page_count": pragma("page_count")?,
            "cache_size": pragma("cache_size")?,
            "mmap_size": pragma("mmap_size")?,
            "synchronous": pragma("synchronous")?,
        },
        "cache_reset": {
            "profile": "warm_same_connection",
            "method": "one discarded warmup, then repeated reads through one open read-only connection",
            "limitations": "The run does not clear the SQLite page cache or operating-system file cache.",
        },
        "read_repetitions": reps,
        "bulk_repetitions": bulk,
        "operations": operations,
        "query_plans": Value::Object(plans),
        "process_peak_rss_bytes": peak_rss_bytes(),
    });
    std::fs::write(&options.output, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    drop(store);
    Ok(())
}
