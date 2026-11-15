//! The check reads each table once in key order and looks rows up by key.
//!
//! `check_plans_read_by_key` runs on the demo database. `check_timing` runs
//! only when `ATH_PERF_DATABASE` names a generated performance database; run
//! it with `cargo test --release --test check_performance -- --ignored
//! --nocapture`.

mod maintenance_support;

use std::time::Instant;

use athleticstandard::store::check::{check_database, check_statements};
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::{OpenOptions, Store};
use maintenance_support::*;
use rusqlite::Connection;

fn plan(connection: &Connection, sql: &str) -> Vec<String> {
    let mut statement = connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    let count = statement.parameter_count();
    let params: Vec<String> = (0..count).map(|_| "x".to_string()).collect();
    statement
        .query_map(rusqlite::params_from_iter(params), |row| row.get::<_, String>(3))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn check_plans_read_by_key() {
    let path = copy_fixture(&demo_fixture(), "ath-check-plans-", false);
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
    let connection = inspect(&path);
    let records =
        store.list_records(&ListOptions { include_deleted: true, limit: 1000, ..ListOptions::default() }).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for record in &records {
        for (index, sql) in check_statements(&record.data).into_iter().enumerate() {
            if !seen.insert(sql.clone()) {
                continue;
            }
            let steps = plan(&connection, &sql);
            let text = steps.join(" | ");
            // No statement sorts in a temporary B-tree.
            assert!(!text.contains("TEMP B-TREE"), "{sql}: {text}");
            if index < 2 {
                // The two passes walk a primary-key index in order.
                assert!(text.contains("USING INDEX sqlite_autoindex"), "{sql}: {text}");
            } else {
                // Every per-record read is a keyed search, never a scan.
                assert!(text.starts_with("SEARCH"), "{sql}: {text}");
                assert!(!text.contains("SCAN"), "{sql}: {text}");
            }
        }
    }
    assert!(seen.len() > 20, "{}", seen.len());
}

#[test]
#[ignore = "needs ATH_PERF_DATABASE"]
fn check_timing() {
    let Ok(path) = std::env::var("ATH_PERF_DATABASE") else { return };
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
    let connection = inspect(std::path::Path::new(&path));
    let started = Instant::now();
    let result = check_database(&store).unwrap();
    let elapsed = started.elapsed();
    let errors: Vec<_> = result.issues.iter().filter(|issue| issue.check != "schema_version").collect();
    eprintln!(
        "check: {} ms, {} records, valid apart from the schema step: {}",
        elapsed.as_millis(),
        result.record_count,
        errors.is_empty()
    );
    for (label, sql) in [
        ("records", athleticstandard::store::check::RECORDS_IN_ORDER),
        ("revisions", athleticstandard::store::check::REVISIONS_IN_ORDER),
    ] {
        eprintln!("plan {label}: {}", plan(&connection, sql).join(" | "));
    }
    assert!(errors.is_empty(), "{errors:?}");
}
