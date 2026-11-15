//! Range and one-record calculations, compared with 2.1.0 outputs on the same
//! invented records. The golden files come from
//! `tools/reference/calculation-golden.ts`.

#[path = "scenario_support/mod.rs"]
mod common;

use std::path::Path;

use athleticstandard::calculate::{self, CalculateOptions, Group, Operation, RecordKind};
use athleticstandard::json::Value;
use athleticstandard::model::{ProviderReportingPeriodType, SeriesQuantity};
use athleticstandard::store::query::{self, AsOfQuery, MeasurementQuery, Order, SeriesQuery, SessionQuery, TimeRange};
use athleticstandard::store::Store;
use athleticstandard::view::series::{read_series_view, render_series_view, SeriesViewOptions};
use common::*;

fn options(input: &Value) -> CalculateOptions {
    let mut options = CalculateOptions::new(
        Operation::parse(input["operation"].as_str().unwrap()).unwrap(),
        input["quantity"].as_str().unwrap(),
    );
    if let Some(kind) = opt_string(input, "record_kind") {
        options.record_kind = RecordKind::parse(&kind).unwrap();
    }
    options.provider_period_type =
        opt_string(input, "provider_period_type").map(|text| ProviderReportingPeriodType::parse(&text).unwrap());
    options.source_id = opt_string(input, "source_id");
    options.compare_source_id = opt_string(input, "compare_source_id");
    options.other_quantity = opt_string(input, "other_quantity");
    options.workout_session_id = opt_string(input, "workout_session_id");
    options.from = opt_string(input, "from");
    options.to = opt_string(input, "to");
    options.time_zone = opt_string(input, "time_zone");
    if let Some(group) = opt_string(input, "group") {
        options.group = Group::parse(&group).unwrap();
    }
    options
}

fn records(result: athleticstandard::Result<Vec<athleticstandard::model::CurrentRecord>>) -> Value {
    outcome(result.map(|records| records.iter().map(|record| record.to_value()).collect::<Vec<_>>()))
}

fn range(store: &mut Store, call: &str, input: &Value, _: &Path) -> (Value, Compare) {
    let connection = store.connection();
    let output = match call {
        "calculate" => outcome(calculate::calculate_range(store, &options(input)).map(|receipt| {
            object(vec![
                ("receipt", athleticstandard::json::to_value(&receipt)),
                ("rendered", Value::String(calculate::render_calculation(&receipt))),
            ])
        })),
        "series" => {
            let quantity = SeriesQuantity::parse(input["quantity"].as_str().unwrap()).unwrap();
            outcome(read_series_view(store, quantity, &SeriesViewOptions::default()).map(|view| {
                object(vec![
                    ("view", athleticstandard::json::to_value(&view)),
                    ("rendered", Value::String(render_series_view(&view))),
                ])
            }))
        }
        "query-measurements" => records(query::query_measurements(
            connection,
            &MeasurementQuery {
                range: TimeRange { from: opt_string(input, "from"), to: opt_string(input, "to") },
                quantity: opt_string(input, "quantity")
                    .map(|text| athleticstandard::model::PointMeasurementType::parse(&text).unwrap()),
                source_id: opt_string(input, "source_id"),
                include_manual: flag(input, "include_manual"),
                limit: opt_u32(input, "limit"),
            },
        )),
        "query-series" => {
            let mut series = SeriesQuery::new(SeriesQuantity::parse(input["quantity"].as_str().unwrap()).unwrap());
            series.limit = opt_u32(input, "limit");
            series.source_id = opt_string(input, "source_id");
            if opt_string(input, "order").as_deref() == Some("descending") {
                series.order = Order::Descending;
            }
            records(query::query_series(connection, &series))
        }
        "query-sessions" => records(query::query_sessions(
            connection,
            &SessionQuery {
                session_type: opt_string(input, "session_type").map(|text| match text.as_str() {
                    "sleep_session" => query::SessionType::Sleep,
                    _ => query::SessionType::Workout,
                }),
                source_id: opt_string(input, "source_id"),
                limit: opt_u32(input, "limit"),
                ..SessionQuery::default()
            },
        )),
        "query-as-of" => records(query::query_records_as_of(
            connection,
            &AsOfQuery {
                as_of: opt_string(input, "as_of").unwrap(),
                record_types: input
                    .get("record_types")
                    .and_then(Value::as_array)
                    .map(|types| {
                        types
                            .iter()
                            .map(|item| athleticstandard::model::RecordType::parse(item.as_str().unwrap()).unwrap())
                            .collect()
                    })
                    .unwrap_or_default(),
                limit: opt_u32(input, "limit"),
            },
        )),
        other => panic!("unknown call {other}"),
    };
    (output, Compare::Exact)
}

#[test]
fn range_calculations_match_the_reference() {
    Scenario::open("calculate/range.json").check(range);
}

#[test]
fn one_record_calculations_match_the_reference() {
    Scenario::open("calculate/apply.json").check(|store, call, input, directory| {
        assert_eq!(call, "apply");
        let working = temporary_dir("ath-apply-");
        if let Some(installed) = input.get("installed").and_then(Value::as_object) {
            for (name, text) in installed {
                let folder = working.join(".agents/skills").join(name);
                std::fs::create_dir_all(&folder).unwrap();
                std::fs::write(folder.join("SKILL.md"), text.as_str().unwrap()).unwrap();
            }
        }
        let _ = directory;
        let output = outcome(calculate::apply(
            store,
            input["calculation"].as_str().unwrap(),
            input["record_id"].as_str().unwrap(),
            &working,
        ));
        std::fs::remove_dir_all(&working).unwrap();
        (output, Compare::Exact)
    });
}
