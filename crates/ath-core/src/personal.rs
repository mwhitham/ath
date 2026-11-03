//! Garmin personal connection. The key is passed in and is not written to the database.

use serde_json::{json, Map, Value};

use crate::garmin::workout_from_json;
use crate::store::{timestamp_receipt, utc_now, Store};

const TOKEN_PATH: &str = "/personalapitoken-service/personal-api/personal-api-key/token";
const ACTIVITIES_PATH: &str = "/activitylist-service/activities/search/activities?start=0&limit=20";

pub fn pull_garmin(
    store: &mut Store,
    base_url: &str,
    api_key: &str,
    date: &str,
) -> Result<Value, String> {
    let token = obtain_token(base_url, api_key)?;
    let activities = get_json(base_url, ACTIVITIES_PATH, &token)?;
    let hrv_path = format!("/hrv-service/hrv/{date}");
    let hrv = get_json(base_url, &hrv_path, &token).ok();
    let source_id = store.ensure_source("garmin", "garmin_personal_api", "garmin")?;
    let received_at = utc_now();
    let mut record_ids = Vec::new();
    let mut understood = Vec::new();
    let mut preserved = Vec::new();
    let activity_list = match &activities {
        Value::Array(items) => items.clone(),
        other => vec![other.clone()],
    };
    for activity in activity_list {
        let Some(object) = activity.as_object() else {
            continue;
        };
        if object.get("startTimeGMT").is_none() && object.get("start_at").is_none() {
            preserved.extend(object.keys().cloned());
            continue;
        }
        let (data, understood_fields, preserved_fields) =
            workout_from_json(&source_id, object, &received_at)?;
        understood.extend(understood_fields);
        preserved.extend(preserved_fields);
        let key = object
            .get("activityId")
            .map(|value| value.to_string())
            .unwrap_or_else(|| data.get("start_at").and_then(Value::as_str).unwrap_or("activity").to_string());
        let id = store.insert_record(
            "workout_session",
            &data,
            Some(("garmin_personal/v1", &key)),
            "provider_import",
        )?;
        record_ids.push(id);
    }
    if let Some(hrv) = hrv {
        if let Some(object) = hrv.as_object() {
            if let Some(last_night) = object.get("LastNightAvg").and_then(Value::as_f64) {
                let mut data = Map::new();
                data.insert("source_id".into(), json!(source_id));
                data.insert("received_at".into(), json!(received_at));
                data.insert("received_at_basis".into(), json!("import_time"));
                data.insert("summary_type".into(), json!("LastNightAvg"));
                data.insert("recorded_at".into(), json!(format!("{date}T00:00:00.000Z")));
                data.insert("input_timestamp".into(), timestamp_receipt(&format!("{date}T00:00:00.000Z")));
                data.insert(
                    "value".into(),
                    json!({
                        "standard_value": last_night,
                        "standard_unit": "ms",
                        "input_value": last_night,
                        "input_unit": "ms"
                    }),
                );
                let mut supplied = Map::new();
                for (key, value) in object {
                    if key != "LastNightAvg" {
                        supplied.insert(key.clone(), value.clone());
                        preserved.push(key.clone());
                    }
                }
                if !supplied.is_empty() {
                    data.insert("supplied_fields".into(), Value::Object(supplied));
                }
                data.insert("field_basis".into(), crate::garmin::provider_basis(false));
                understood.push("LastNightAvg".into());
                let id = store.insert_record(
                    "provider_summary",
                    &data,
                    Some(("garmin_personal/v1", &format!("LastNightAvg:{date}"))),
                    "provider_import",
                )?;
                record_ids.push(id);
            } else {
                preserved.extend(object.keys().cloned());
            }
        }
    }
    understood.sort();
    understood.dedup();
    preserved.sort();
    preserved.dedup();
    Ok(json!({
        "source": "garmin_personal",
        "record_ids": record_ids,
        "understood": understood,
        "preserved": preserved,
    }))
}

fn obtain_token(base_url: &str, api_key: &str) -> Result<String, String> {
    let response = ureq::post(&format!("{}{TOKEN_PATH}", base_url.trim_end_matches('/')))
        .set("Authorization", &format!("ApiKey {api_key}"))
        .set("Accept", "*/*")
        .call()
        .map_err(|error| format!("Garmin token request failed: {error}"))?;
    let token = response.into_string().map_err(|error| error.to_string())?;
    let token = token.trim();
    if token.split('.').count() < 3 {
        return Err("Garmin token response was not a JWT".into());
    }
    Ok(token.to_string())
}

fn get_json(base_url: &str, path: &str, token: &str) -> Result<Value, String> {
    let response = ureq::get(&format!("{}{path}", base_url.trim_end_matches('/')))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/json")
        .call()
        .map_err(|error| format!("Garmin request {path} failed: {error}"))?;
    let body = response.into_string().map_err(|error| error.to_string())?;
    serde_json::from_str(&body)
        .map_err(|error| format!("Garmin response for {path} was not JSON: {error}"))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::obtain_token;

    #[test]
    fn reads_a_jwt_from_the_token_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 2048];
            let _ = stream.read(&mut buffer);
            let body = "aaa.bbb.ccc";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        let token = obtain_token(&format!("http://{address}"), "test-key").unwrap();
        assert_eq!(token, "aaa.bbb.ccc");
    }
}
