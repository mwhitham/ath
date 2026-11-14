//! Series and record views, compared with 2.1.0 outputs: the `--json`
//! objects and the human text.

mod common;

use athleticstandard::json::{self, Value};
use athleticstandard::model::{RecordType, SeriesQuantity};
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::Store;
use athleticstandard::view::calendar::{resolve_as_of_time, resolve_calendar_range, CalendarRangeRequest};
use athleticstandard::view::records::{list_records, render_record, render_record_list, show_record, ShowOptions};
use athleticstandard::view::series::{read_series_view, render_series_view, SeriesViewOptions};
use common::*;

pub fn view_call(store: &mut Store, call: &str, input: &Value) -> Value {
    match call {
        "series" => {
            let quantity = SeriesQuantity::parse(input["quantity"].as_str().unwrap()).unwrap();
            let options = &input["options"];
            let options = SeriesViewOptions {
                from: opt_string(options, "from"),
                to: opt_string(options, "to"),
                source_id: opt_string(options, "source_id"),
                time_zone: opt_string(options, "time_zone"),
                raw: flag(options, "raw"),
                limit: opt_u32(options, "limit"),
            };
            outcome(read_series_view(store, quantity, &options).map(|view| {
                object(vec![("view", json::to_value(&view)), ("rendered", Value::String(render_series_view(&view)))])
            }))
        }
        "calendar-range" => outcome(resolve_calendar_range(
            store.connection(),
            &CalendarRangeRequest {
                from: opt_string(input, "from"),
                to: opt_string(input, "to"),
                time_zone: opt_string(input, "time_zone"),
                raw: flag(input, "raw"),
            },
        )),
        "as-of" => outcome(resolve_as_of_time(
            store.connection(),
            input.get("value").and_then(Value::as_str),
            input.get("time_zone").and_then(Value::as_str),
            || unreachable!("every case names a time"),
        )),
        "records-list" => {
            let options = ListOptions {
                record_type: opt_string(input, "record_type").map(|text| RecordType::parse(&text).unwrap()),
                include_deleted: flag(input, "include_deleted"),
                limit: opt_u32(input, "limit").unwrap_or(100),
            };
            outcome(list_records(store, &options).map(|list| {
                object(vec![
                    ("list", json::to_value(&list)),
                    ("rendered", Value::String(render_record_list(&list.records))),
                ])
            }))
        }
        "records-show" => {
            let options = ShowOptions {
                revision: input.get("revision").and_then(Value::as_u64),
                history: flag(input, "history"),
                links: flag(input, "links"),
            };
            outcome(show_record(store, input["record_id"].as_str().unwrap(), options).map(|show| {
                object(vec![("show", json::to_value(&show)), ("rendered", Value::String(render_record(&show)))])
            }))
        }
        other => panic!("unknown call {other}"),
    }
}

#[test]
fn series_views_match_the_reference() {
    Scenario::open("view/series.json").check(|store, call, input, _| (view_call(store, call, input), Compare::Exact));
}

#[test]
fn record_views_match_the_reference() {
    Scenario::open("view/records.json").check(|store, call, input, _| (view_call(store, call, input), Compare::Exact));
}
