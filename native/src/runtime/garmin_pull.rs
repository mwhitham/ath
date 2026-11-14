//! The Garmin personal connection: `ath pull garmin --date <day>`.
//!
//! The pull exchanges the person's API key for a token, then reads the latest
//! activities and the day's HRV summary. The key is sent only in the token
//! request's `Authorization` header. It is never written to the database, and
//! a message or result that would contain it is replaced.
//!
//! The responses are read before the database is touched. The import then
//! runs as one preview and one write, so no lock is held while the network
//! answers.
//!
//! The base URL is a parameter, so tests use a local server and never the
//! network. These are the rules of `crates/ath-core/src/personal.rs` in 2.1.0.

use std::time::Duration;

use crate::error::{Error, Result};
use crate::import::{self, ImportOptions, PersonalDay};
use crate::json::Value;
use crate::store::Store;

/// Garmin's personal API root.
pub const PERSONAL_API_BASE: &str = "https://personal.connectapi.garmin.com";

const TOKEN_PATH: &str = "/personalapitoken-service/personal-api/personal-api-key/token";
const ACTIVITIES_PATH: &str = "/activitylist-service/activities/search/activities?start=0&limit=20";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// The message shown in place of any text that contains the key.
pub const KEY_HIDDEN_MESSAGE: &str = "Garmin personal connection failed";

/// Read one day from the personal connection: the activity list, and the
/// day's HRV summary when Garmin returns one.
pub fn fetch_day(base_url: &str, api_key: &str, date: &str) -> Result<PersonalDay> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .timeout_write(READ_TIMEOUT)
        .build();
    let hide = |message: String| {
        if message.contains(api_key) {
            Error::other(KEY_HIDDEN_MESSAGE)
        } else {
            Error::other(message)
        }
    };
    let token = obtain_token(&agent, base_url, api_key).map_err(hide)?;
    let activities = get_json(&agent, base_url, ACTIVITIES_PATH, &token).map_err(hide)?;
    let hrv = get_json(&agent, base_url, &format!("/hrv-service/hrv/{date}"), &token).ok();
    Ok(PersonalDay { date: date.to_string(), activities, hrv })
}

fn obtain_token(agent: &ureq::Agent, base_url: &str, api_key: &str) -> std::result::Result<String, String> {
    let response = agent
        .post(&format!("{}{TOKEN_PATH}", base_url.trim_end_matches('/')))
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

fn get_json(agent: &ureq::Agent, base_url: &str, path: &str, token: &str) -> std::result::Result<Value, String> {
    let response = agent
        .get(&format!("{}{path}", base_url.trim_end_matches('/')))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/json")
        .call()
        .map_err(|error| format!("Garmin request {path} failed: {error}"))?;
    let body = response.into_string().map_err(|error| error.to_string())?;
    serde_json::from_str(&body).map_err(|error| format!("Garmin response for {path} was not JSON: {error}"))
}

/// Pull one day and write it. Returns what 2.1.0 printed with `--json`:
/// `source`, `record_ids`, `understood`, and `preserved`.
///
/// The pull has no separate confirmation, as in 2.1.0: it previews and
/// writes in one call. A result that contains the key is refused.
pub fn pull_day(
    store: &mut Store,
    base_url: &str,
    api_key: &str,
    date: &str,
    options: &ImportOptions,
) -> Result<Value> {
    let day = fetch_day(base_url, api_key, date)?;
    let preview = import::preview_pull(store, day, options)?;
    preview.apply(store)?;
    let result = preview.report_value(true)?;
    if !api_key.is_empty() && crate::json::to_string(&result).contains(api_key) {
        return Err(Error::refused("refusing to print a result that contains the Garmin key"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    #[test]
    fn reads_a_jwt_from_the_token_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 2048];
            let _ = stream.read(&mut buffer);
            let body = "aaa.bbb.ccc";
            let response =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = stream.write_all(response.as_bytes());
        });
        let agent = ureq::AgentBuilder::new().build();
        assert_eq!(obtain_token(&agent, &format!("http://{address}"), "test-key").unwrap(), "aaa.bbb.ccc");
    }
}
