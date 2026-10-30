use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use serde_json::{json, Value};

use ath_core::apple;
use ath_core::calc::{parse_skill, run_skill};
use ath_core::garmin::import_garmin;
use ath_core::personal::pull_garmin;
use ath_core::oura;
use ath_core::persist;
use ath_core::range_calc;
use ath_core::store::Store;
use ath_core::whoop;
use ath_core::strava::import_strava;

fn main() {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    let result = match command.as_str() {
        "show" => show(&collect(args)),
        "revise" => revise(&collect(args)),
        "calculate" => calculate(&collect(args)),
        "import-garmin" => import(&collect(args), import_garmin),
        "import-strava" => import(&collect(args), import_strava),
        "pull-garmin" => pull(&collect(args)),
        "persist" => persist_command(&collect(args)),
        "calculate-range" => calculate_range(&collect(args)),
        "import-whoop" => import_whoop_command(),
        "import-oura" => import_named(oura::import_oura),
        "import-apple" => import_apple_command(),
        _ => Err("commands: show, revise, calculate, import-garmin, import-strava, pull-garmin, persist".into()),
    };
    match result {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn collect(args: impl Iterator<Item = String>) -> Vec<String> {
    args.collect()
}

fn flag(args: &[String], name: &str) -> Result<String, String> {
    args.windows(2)
        .find(|pair| pair[0] == format!("--{name}"))
        .map(|pair| pair[1].clone())
        .ok_or_else(|| format!("missing --{name}"))
}

fn show(args: &[String]) -> Result<Value, String> {
    let store = Store::open(&PathBuf::from(flag(args, "database")?))?;
    let record = store.show(&flag(args, "record")?)?;
    Ok(json!({
        "record_id": record.record_id,
        "record_type": record.record_type,
        "revision": record.revision,
        "data": record.data,
    }))
}

fn revise(args: &[String]) -> Result<Value, String> {
    let mut store = Store::open(&PathBuf::from(flag(args, "database")?))?;
    let patch = serde_json::from_str::<Value>(&flag(args, "patch")?)
        .map_err(|error| error.to_string())?;
    let object = patch
        .as_object()
        .cloned()
        .ok_or("patch must be a JSON object")?;
    let record = store.revise(&flag(args, "record")?, &object)?;
    Ok(json!({
        "record_id": record.record_id,
        "revision": record.revision,
        "data": record.data,
    }))
}

fn calculate(args: &[String]) -> Result<Value, String> {
    let store = Store::open(&PathBuf::from(flag(args, "database")?))?;
    let record = store.show(&flag(args, "record")?)?;
    let skill_text = fs::read_to_string(flag(args, "skill")?).map_err(|error| error.to_string())?;
    let skill = parse_skill(&skill_text)?;
    let related = related_records(&store, &record)?;
    Ok(run_skill(&skill, &record, &related))
}

fn related_records(store: &Store, record: &ath_core::store::StoredRecord) -> Result<Vec<ath_core::store::StoredRecord>, String> {
    let Some(source_id) = record.data.get("source_id").and_then(|value| value.as_str()) else {
        return Ok(Vec::new());
    };
    if record.record_type == "point_measurement" {
        let quantity = record.data.get("quantity").and_then(|value| value.as_str()).unwrap_or("");
        return store.point_records(source_id, quantity);
    }
    if record.record_type == "sleep_session" || record.record_type == "workout_session" {
        return store.same_source_records(source_id, &record.record_type);
    }
    Ok(Vec::new())
}

fn import(
    args: &[String],
    run: fn(&mut Store, &std::path::Path, bool) -> Result<Value, String>,
) -> Result<Value, String> {
    let write = !args.iter().any(|arg| arg == "--preview");
    let mut store = Store::open(&PathBuf::from(flag(args, "database")?))?;
    run(&mut store, PathBuf::from(flag(args, "input")?).as_path(), write)
}

fn import_named(run: fn(&serde_json::Map<String, Value>, &str) -> Value) -> Result<Value, String> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).map_err(|error| error.to_string())?;
    let body: Value = serde_json::from_str(&input).map_err(|error| error.to_string())?;
    let files = body.get("files").and_then(Value::as_object).cloned().ok_or("import needs files")?;
    let source_id = body.get("source_id").and_then(Value::as_str).unwrap_or("");
    Ok(run(&files, source_id))
}

fn import_apple_command() -> Result<Value, String> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).map_err(|error| error.to_string())?;
    let body: Value = serde_json::from_str(&input).map_err(|error| error.to_string())?;
    let path = body.get("xml_path").and_then(Value::as_str).ok_or("import-apple needs xml_path")?;
    let entry = body.get("zip_entry").and_then(Value::as_str).filter(|entry| !entry.is_empty());
    apple::import_apple(path, entry)
}

fn import_whoop_command() -> Result<Value, String> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).map_err(|error| error.to_string())?;
    let body: Value = serde_json::from_str(&input).map_err(|error| error.to_string())?;
    let files = body.get("files").and_then(Value::as_object).ok_or("import-whoop needs files")?;
    let source_id = body.get("source_id").and_then(Value::as_str).unwrap_or("");
    Ok(whoop::import_whoop(files, source_id))
}

fn calculate_range(args: &[String]) -> Result<Value, String> {
    let conn = rusqlite::Connection::open(flag(args, "database")?).map_err(|error| error.to_string())?;
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).map_err(|error| error.to_string())?;
    let body: Value = serde_json::from_str(&input).map_err(|error| error.to_string())?;
    range_calc::calculate(&conn, &body)
}

fn persist_command(args: &[String]) -> Result<Value, String> {
    let mut conn = rusqlite::Connection::open(flag(args, "database")?).map_err(|error| error.to_string())?;
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).map_err(|error| error.to_string())?;
    let body: Value = serde_json::from_str(&input).map_err(|error| error.to_string())?;
    persist::apply(&mut conn, &body)
}

fn pull(args: &[String]) -> Result<Value, String> {
    let mut store = Store::open(&PathBuf::from(flag(args, "database")?))?;
    let base = flag(args, "base-url").unwrap_or_else(|_| "https://personal.connectapi.garmin.com".into());
    let key = flag(args, "key").or_else(|_| {
        env::var("ATH_CORE_GARMIN_KEY").map_err(|_| "missing Garmin personal API key".to_string())
    })?;
    let date = flag(args, "date")?;
    pull_garmin(&mut store, &base, &key, &date)
}
