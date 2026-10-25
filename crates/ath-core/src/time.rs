use chrono::{Datelike, Timelike};

pub fn format_datetime(value: chrono::DateTime<chrono::Utc>) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        value.year(),
        value.month(),
        value.day(),
        value.hour(),
        value.minute(),
        value.second(),
        value.timestamp_subsec_millis()
    )
}

pub fn format_millis(millis: i64) -> String {
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|value| format_datetime(value.with_timezone(&chrono::Utc)))
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".into())
}
