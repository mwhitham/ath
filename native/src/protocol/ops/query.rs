//! Bounded queries, calendar views, range calculations, and their text.

use crate::error::Error;
use crate::json::{self, Value};
use crate::model::{CurrentRecord, PointMeasurementType, RecordRevision, RecordType, SeriesQuantity};
use crate::protocol::definition::{Kind, Operation, TargetUse};
use crate::protocol::worker::{Call, Failure, Outcome};
use crate::store::query::{
    self, AsOfQuery, MeasurementQuery, Order, SeriesQuery, SessionQuery, SessionType, TimeRange,
};
use crate::view::calendar::{self, CalendarRangeRequest};
use crate::view::series::{SeriesDay, SeriesView, SeriesViewOptions};

use super::input;

const RECORDS: Option<&str> = Some("Model.CurrentRecordT");

pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "query.measurements",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ query: unknown }",
        output: "null",
        items: RECORDS,
        callbacks: &[],
        run: measurements,
    },
    Operation {
        name: "query.series",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ query: unknown }",
        output: "null",
        items: RECORDS,
        callbacks: &[],
        run: series,
    },
    Operation {
        name: "query.sessions",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ query: unknown }",
        output: "null",
        items: RECORDS,
        callbacks: &[],
        run: sessions,
    },
    Operation {
        name: "query.as_of",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ query: unknown }",
        output: "null",
        items: RECORDS,
        callbacks: &[],
        run: as_of,
    },
    Operation {
        name: "view.series",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ quantity: unknown; options: unknown }",
        output: "Api.SeriesView",
        items: None,
        callbacks: &[],
        run: series_view,
    },
    Operation {
        name: "view.calendar_range",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ options: unknown }",
        output: "Api.InstantRange",
        items: None,
        callbacks: &[],
        run: calendar_range,
    },
    Operation {
        name: "view.as_of_time",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ value?: string; time_zone?: string }",
        output: "{ as_of: string; time_zone?: string }",
        items: None,
        callbacks: &[],
        run: as_of_time,
    },
    Operation {
        name: "calculate.range",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ options: unknown }",
        output: "Api.CalculationResult",
        items: None,
        callbacks: &[],
        run: calculate,
    },
    Operation {
        name: "render.record_list",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ records: readonly unknown[] }",
        output: "string",
        items: None,
        callbacks: &[],
        run: render_record_list,
    },
    Operation {
        name: "render.record",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ record: unknown; revision?: unknown; history?: readonly unknown[]; links?: readonly unknown[] }",
        output: "string",
        items: None,
        callbacks: &[],
        run: render_record,
    },
    Operation {
        name: "render.series_view",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ view: unknown }",
        output: "string",
        items: None,
        callbacks: &[],
        run: render_series_view,
    },
    Operation {
        name: "render.stats",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ stats: unknown }",
        output: "string",
        items: None,
        callbacks: &[],
        run: render_stats,
    },
    Operation {
        name: "render.calculation",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ calculation: unknown }",
        output: "string",
        items: None,
        callbacks: &[],
        run: render_calculation,
    },
];

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key).filter(|value| !value.is_null())
}

fn text(value: &Value, key: &str) -> Result<Option<String>, Failure> {
    match field(value, key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(other) => Ok(Some(js_string(other))),
    }
}

/// `String(value)` for the values a caller might pass where text belongs.
fn js_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => json::format_number(json::number_as_f64(number)),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".into(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// A query limit. A value that is not a whole number in range takes the
/// limit refusal, as 2.1.0's `Number.isInteger` check did.
fn limit(value: &Value) -> Option<u32> {
    field(value, "limit").map(|limit| input::whole_number(limit).and_then(|n| u32::try_from(n).ok()).unwrap_or(0))
}

fn range(value: &Value) -> Result<TimeRange, Failure> {
    Ok(TimeRange { from: text(value, "from")?, to: text(value, "to")? })
}

fn include_manual(value: &Value) -> bool {
    field(value, "include_manual") == Some(&Value::Bool(true))
}

fn stream(call: &Call<'_>, records: Vec<CurrentRecord>) -> Outcome {
    for record in &records {
        call.item(record.to_value());
    }
    Ok(Value::Null)
}

/// `queryMeasurements(database, query)`.
fn measurements(call: &mut Call<'_>) -> Outcome {
    let request = call.input.get("query").cloned().unwrap_or(Value::Null);
    let quantity = match field(&request, "quantity") {
        Some(value) => Some(input::decode::<PointMeasurementType>(value)?),
        None => None,
    };
    let query = MeasurementQuery {
        range: range(&request)?,
        quantity,
        source_id: text(&request, "source_id")?,
        include_manual: include_manual(&request),
        limit: limit(&request),
    };
    let records = query::query_measurements(call.store()?.connection(), &query)?;
    stream(call, records)
}

/// `querySeries(database, query)`.
fn series(call: &mut Call<'_>) -> Outcome {
    let request = call.input.get("query").cloned().unwrap_or(Value::Null);
    let quantity: SeriesQuantity = input::decode(request.get("quantity").unwrap_or(&Value::Null))?;
    let mut query = SeriesQuery::new(quantity);
    query.range = range(&request)?;
    query.source_id = text(&request, "source_id")?;
    query.include_manual = include_manual(&request);
    query.limit = limit(&request);
    query.order =
        if field(&request, "order") == Some(&Value::from("descending")) { Order::Descending } else { Order::Ascending };
    let records = query::query_series(call.store()?.connection(), &query)?;
    stream(call, records)
}

/// `querySessions(database, query)`.
fn sessions(call: &mut Call<'_>) -> Outcome {
    let request = call.input.get("query").cloned().unwrap_or(Value::Null);
    let session_type = match field(&request, "session_type").and_then(Value::as_str) {
        None => None,
        Some("sleep_session") => Some(SessionType::Sleep),
        Some("workout_session") => Some(SessionType::Workout),
        Some(other) => return Err(Error::other(format!("Unknown session type: {other}")).into()),
    };
    let query = SessionQuery {
        range: range(&request)?,
        session_type,
        source_id: text(&request, "source_id")?,
        include_manual: include_manual(&request),
        limit: limit(&request),
    };
    let records = query::query_sessions(call.store()?.connection(), &query)?;
    stream(call, records)
}

/// `queryRecordsAsOf(database, query)`.
fn as_of(call: &mut Call<'_>) -> Outcome {
    let request = call.input.get("query").cloned().unwrap_or(Value::Null);
    let as_of = input::checked_text("UtcTimestamp", request.get("as_of").unwrap_or(&Value::Null))?;
    let record_type = field(&request, "record_type");
    let record_types = field(&request, "record_types");
    if record_type.is_some() && record_types.is_some() {
        return Err(Error::other("Use record_type or record_types, not both").into());
    }
    let record_type = match record_type {
        Some(value) => Some(input::decode::<RecordType>(value)?),
        None => None,
    };
    let record_types = match record_types {
        Some(Value::Array(items)) => {
            Some(items.iter().map(input::decode::<RecordType>).collect::<Result<Vec<_>, _>>()?)
        }
        Some(_) => return Err(Failure::Input("record_types must be an array".into())),
        None => None,
    };
    let query = AsOfQuery::from_parts(as_of, record_type, record_types, limit(&request))?;
    let records = query::query_records_as_of(call.store()?.connection(), &query)?;
    stream(call, records)
}

fn series_options(options: &Value) -> Result<SeriesViewOptions, Failure> {
    Ok(SeriesViewOptions {
        from: text(options, "from")?,
        to: text(options, "to")?,
        source_id: text(options, "source_id")?,
        time_zone: text(options, "time_zone")?,
        raw: field(options, "raw") == Some(&Value::Bool(true)),
        limit: limit(options),
    })
}

/// `readSeriesView(database, quantity, options)`.
fn series_view(call: &mut Call<'_>) -> Outcome {
    let quantity: SeriesQuantity = input::decode(call.input.get("quantity").unwrap_or(&Value::Null))?;
    let options = series_options(call.input.get("options").unwrap_or(&Value::Null))?;
    let view = crate::view::series::read_series_view(call.store()?, quantity, &options)?;
    Ok(json::to_value(&view))
}

/// `resolveCalendarRange(database, options)`.
fn calendar_range(call: &mut Call<'_>) -> Outcome {
    let options = series_options(call.input.get("options").unwrap_or(&Value::Null))?;
    let request =
        CalendarRangeRequest { from: options.from, to: options.to, time_zone: options.time_zone, raw: options.raw };
    let range = calendar::resolve_calendar_range(call.store()?.connection(), &request)?;
    Ok(json::to_value(&range))
}

/// `resolveAsOfTime(database, value, explicitTimeZone)`.
fn as_of_time(call: &mut Call<'_>) -> Outcome {
    let value = input::string(&call.input, "value")?;
    let zone = input::string(&call.input, "time_zone")?;
    let resolved = calendar::resolve_as_of_time(
        call.store()?.connection(),
        value.as_deref(),
        zone.as_deref(),
        crate::model::time::now_utc,
    )?;
    Ok(json::to_value(&resolved))
}

/// `calculate(database, options)`.
fn calculate(call: &mut Call<'_>) -> Outcome {
    use crate::calculate::range::{CalculateOptions, Group, Operation as Calculation, RecordKind};
    let request = call.input.get("options").cloned().unwrap_or(Value::Null);
    let operation_text = text(&request, "operation")?.unwrap_or_else(|| "undefined".into());
    let operation = Calculation::parse(&operation_text).ok_or_else(|| {
        Error::other(format!("Unknown calculation operation {operation_text}; use one of {}", Calculation::names()))
    })?;
    let mut options =
        CalculateOptions::new(operation, text(&request, "quantity")?.unwrap_or_else(|| "undefined".into()));
    if let Some(kind) = text(&request, "record_kind")? {
        options.record_kind = RecordKind::parse(&kind).ok_or_else(|| {
            Error::other(format!("Unknown calculation record kind {kind}; use one of {}", RecordKind::names()))
        })?;
    }
    if let Some(period) = field(&request, "provider_period_type") {
        options.provider_period_type = Some(input::decode(period)?);
    }
    options.source_id = text(&request, "source_id")?;
    options.compare_source_id = text(&request, "compare_source_id")?;
    options.other_quantity = text(&request, "other_quantity")?;
    options.workout_session_id = text(&request, "workout_session_id")?;
    options.from = text(&request, "from")?;
    options.to = text(&request, "to")?;
    options.time_zone = text(&request, "time_zone")?;
    if let Some(group) = text(&request, "group")? {
        options.group = Group::parse(&group)
            .ok_or_else(|| Error::other(format!("Unknown calculation group {group}; use one of {}", Group::names())))?;
    }
    let receipt = crate::calculate::calculate_range(call.store()?, &options)?;
    Ok(json::to_value(&receipt))
}

/// A record a caller passes back to a renderer.
fn record(value: &Value) -> Result<CurrentRecord, Failure> {
    Ok(CurrentRecord::from_value(value)?)
}

/// `renderRecordList(records)`.
fn render_record_list(call: &mut Call<'_>) -> Outcome {
    let records = input::array(&call.input, "records")?.iter().map(record).collect::<Result<Vec<_>, _>>()?;
    Ok(Value::String(crate::view::records::render_record_list(&records)))
}

/// A link a caller passes back to a renderer. The worker exits after one
/// operation, so a relation name outside the known list is kept for its
/// lifetime.
fn link(value: &Value) -> Result<crate::view::records::RecordLink, Failure> {
    use crate::view::records::{LinkDirection, RecordLink};
    let direction = match value.get("direction").and_then(Value::as_str) {
        Some("incoming") => LinkDirection::Incoming,
        Some("outgoing") => LinkDirection::Outgoing,
        _ => return Err(Failure::Input("links[].direction must be incoming or outgoing".into())),
    };
    let relation = value.get("relation").map(js_string).unwrap_or_else(|| "undefined".into());
    Ok(RecordLink {
        direction,
        relation: Box::leak(relation.into_boxed_str()),
        record_id: value.get("record_id").map(js_string).unwrap_or_else(|| "undefined".into()),
        revision: value.get("revision").and_then(input::whole_number),
    })
}

/// `renderRecord(record, { revision, history, links })`.
fn render_record(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let shown = match field(&request, "revision") {
        Some(value) => Some(RecordRevision::from_value(value)?),
        None => None,
    };
    let history = match field(&request, "history") {
        Some(Value::Array(items)) => Some(items.iter().map(RecordRevision::from_value).collect::<Result<Vec<_>, _>>()?),
        Some(_) => return Err(Failure::Input("history must be an array".into())),
        None => None,
    };
    let links = match field(&request, "links") {
        Some(Value::Array(items)) => Some(items.iter().map(link).collect::<Result<Vec<_>, _>>()?),
        Some(_) => return Err(Failure::Input("links must be an array".into())),
        None => None,
    };
    let show = crate::view::records::RecordShow {
        file: String::new(),
        record: record(request.get("record").unwrap_or(&Value::Null))?,
        shown_revision: shown,
        history,
        links,
    };
    Ok(Value::String(crate::view::records::render_record(&show)))
}

fn number(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

fn owned(value: &Value, key: &str) -> String {
    value.get(key).map(js_string).unwrap_or_else(|| "undefined".into())
}

/// `renderSeriesView(view)`.
fn render_series_view(call: &mut Call<'_>) -> Outcome {
    let view = call.input.get("view").cloned().unwrap_or(Value::Null);
    let quantity_text = owned(&view, "quantity");
    let quantity = SeriesQuantity::parse(&quantity_text);
    let static_text = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
    let samples = match field(&view, "samples") {
        Some(Value::Array(items)) => Some(items.iter().map(record).collect::<Result<Vec<_>, _>>()?),
        Some(_) => return Err(Failure::Input("view.samples must be an array".into())),
        None => None,
    };
    let days = match field(&view, "days") {
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .map(|day| SeriesDay {
                    date: owned(day, "date"),
                    source_id: owned(day, "source_id"),
                    sample_count: day.get("sample_count").and_then(input::whole_number).unwrap_or(0) as usize,
                    minimum: number(day, "minimum"),
                    maximum: number(day, "maximum"),
                    mean: number(day, "mean"),
                    first_at: owned(day, "first_at"),
                    last_at: owned(day, "last_at"),
                })
                .collect(),
        ),
        Some(_) => return Err(Failure::Input("view.days must be an array".into())),
        None => None,
    };
    let rendered = SeriesView {
        quantity: quantity.map(SeriesQuantity::as_str).unwrap_or_else(|| static_text(quantity_text)),
        unit: static_text(owned(&view, "unit")),
        time_zone: text(&view, "time_zone")?,
        from: text(&view, "from")?,
        to: text(&view, "to")?,
        source_id: text(&view, "source_id")?,
        truncated: field(&view, "truncated") == Some(&Value::Bool(true)),
        samples,
        days,
    };
    Ok(Value::String(crate::view::series::render_series_view(&rendered)))
}

/// `renderDatabaseStats(stats)`.
fn render_stats(call: &mut Call<'_>) -> Outcome {
    use crate::store::stats::*;
    let stats = call.input.get("stats").cloned().unwrap_or(Value::Null);
    let list = |key: &str| stats.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    let optional = |value: &Value, key: &str| text(value, key).ok().flatten();
    let count = |value: &Value, key: &str| value.get(key).and_then(input::whole_number).unwrap_or(0);
    let current = stats.get("current_records").cloned().unwrap_or(Value::Null);
    let by_type = current
        .get("by_type")
        .and_then(Value::as_object)
        .map(|map| map.iter().map(|(key, value)| (key.clone(), input::whole_number(value).unwrap_or(0))).collect())
        .unwrap_or_default();
    let benchmarks = stats.get("benchmarks").cloned().unwrap_or(Value::Null);
    let predictions = stats.get("predictions").cloned().unwrap_or(Value::Null);
    let rendered = DatabaseStats {
        athleticstandard_version: owned(&stats, "athleticstandard_version"),
        database_id: owned(&stats, "database_id"),
        database_revision: count(&stats, "database_revision"),
        athlete: field(&stats, "athlete").cloned(),
        current_records: CurrentRecordCounts { total: count(&current, "total"), by_type },
        sources: list("sources")
            .iter()
            .map(|source| SourceStats {
                record_id: owned(source, "record_id"),
                source_type: owned(source, "source_type"),
                data_writer: owned(source, "data_writer"),
                data_route: owned(source, "data_route"),
                vendor: optional(source, "vendor"),
                sensor_type: optional(source, "sensor_type"),
                current_record_count: count(source, "current_record_count"),
                first_recorded_at: optional(source, "first_recorded_at"),
                last_recorded_at: optional(source, "last_recorded_at"),
            })
            .collect(),
        baselines: list("baselines")
            .iter()
            .map(|baseline| BaselineStats {
                source_id: owned(baseline, "source_id"),
                quantity: owned(baseline, "quantity"),
                unit: optional(baseline, "unit"),
                mean: number(baseline, "mean"),
                standard_deviation: number(baseline, "standard_deviation"),
                observation_count: count(baseline, "observation_count"),
                from: owned(baseline, "from"),
                to: owned(baseline, "to"),
                window_days: count(baseline, "window_days") as u32,
                method: owned(baseline, "method"),
            })
            .collect(),
        series: list("series")
            .iter()
            .map(|series| SeriesCoverageStats {
                source_id: owned(series, "source_id"),
                quantity: owned(series, "quantity"),
                unit: owned(series, "unit"),
                sample_count: count(series, "sample_count"),
                from: owned(series, "from"),
                to: owned(series, "to"),
            })
            .collect(),
        benchmarks: BenchmarkCounts { defined: count(&benchmarks, "defined"), results: count(&benchmarks, "results") },
        predictions: PredictionCounts {
            total: count(&predictions, "total"),
            graded: count(&predictions, "graded"),
            by_author: predictions
                .get("by_author")
                .and_then(Value::as_array)
                .map(|authors| {
                    authors
                        .iter()
                        .map(|author| AuthorCount {
                            author: owned(author, "author"),
                            predictions: count(author, "predictions"),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
    };
    Ok(Value::String(render_database_stats(&rendered)))
}

/// `renderCalculation(calculation)`.
fn render_calculation(call: &mut Call<'_>) -> Outcome {
    let calculation = call.input.get("calculation").cloned().unwrap_or(Value::Null);
    Ok(Value::String(crate::calculate::range::render_calculation_value(&calculation)))
}
