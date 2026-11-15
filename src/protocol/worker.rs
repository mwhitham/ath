//! The worker loop: read one request, run one operation, write its events.

use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::error::Error;
use crate::json::{self, Map, Value};
use crate::store::{OpenOptions, Store};

use super::definition::{self, Kind, Operation, TargetUse};
use super::{EXIT_CANCELLED, EXIT_FAILED, EXIT_OK, EXIT_PROTOCOL, PROTOCOL_VERSION};

/// Bytes held before events are written to the pipe. A full pipe makes the
/// worker wait, so output never grows without limit.
const EVENT_BUFFER_BYTES: usize = 64 * 1024;

/// Why an operation did not return a result.
#[derive(Debug)]
pub enum Failure {
    /// A library error.
    Error(Error),
    /// A package that this version cannot represent (`PackageRepresentationError`).
    PackageRepresentation(Value),
    /// The request input does not have the shape the operation reads.
    Input(String),
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Failure::Error(error)
    }
}

impl From<crate::model::ValidationError> for Failure {
    fn from(error: crate::model::ValidationError) -> Self {
        Failure::Error(Error::Invalid(error))
    }
}

impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Failure::Error(Error::Io(error))
    }
}

impl From<rusqlite::Error> for Failure {
    fn from(error: rusqlite::Error) -> Self {
        Failure::Error(Error::Database(error))
    }
}

/// The result of a handler.
pub type Outcome = std::result::Result<Value, Failure>;

/// The database a request names.
#[derive(Debug, Clone)]
pub struct Target {
    pub path: PathBuf,
    pub allow_outdated_schema: bool,
    /// The database ID the client opened. A different database at the same
    /// path is refused.
    pub database_id: Option<String>,
}

impl Target {
    fn from_value(value: &Value) -> std::result::Result<Target, String> {
        let path = value
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .ok_or("target.path must be a non-empty string")?;
        let allow_outdated_schema = match value.get("allow_outdated_schema") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(flag)) => *flag,
            Some(_) => return Err("target.allow_outdated_schema must be a boolean".into()),
        };
        let database_id = match value.get("database_id") {
            None | Some(Value::Null) => None,
            Some(Value::String(id)) => Some(id.clone()),
            Some(_) => return Err("target.database_id must be a string".into()),
        };
        Ok(Target { path: PathBuf::from(path), allow_outdated_schema, database_id })
    }

    /// Open and check the database again, as a new open would.
    pub fn open(&self, read_only: bool) -> crate::Result<Store> {
        let store =
            Store::open(&self.path, OpenOptions { read_only, allow_outdated_schema: self.allow_outdated_schema })?;
        if let Some(expected) = &self.database_id {
            if &store.metadata().database_id != expected {
                return Err(Error::refused(format!(
                    "The database at {} is not the one this handle opened: database ID {} differs from {expected}",
                    store.path().display(),
                    store.metadata().database_id
                )));
            }
        }
        Ok(store)
    }
}

/// Shared event output. A cancellation from the input thread and events from
/// the operation never interleave within a line.
struct Events {
    id: String,
    out: Mutex<BufWriter<Box<dyn Write + Send>>>,
}

impl Events {
    fn line(&self, mut event: Map<String, Value>) -> String {
        event.insert("protocol".into(), Value::from(PROTOCOL_VERSION));
        event.insert("id".into(), Value::String(self.id.clone()));
        let mut line = json::to_string(&reorder(event));
        line.push('\n');
        line
    }

    fn write(&self, event: Map<String, Value>, flush: bool) {
        let line = self.line(event);
        let mut out = self.out.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // A client that went away cannot read anything more; the operation
        // still finishes or is cancelled by the input thread.
        let _ = out.write_all(line.as_bytes());
        if flush {
            let _ = out.flush();
        }
    }

    /// Write the cancellation as the last event and end the process while
    /// holding the output, so no later event follows it.
    fn cancel(&self, exit: &(dyn Fn(i32) + Sync)) {
        let mut event = Map::new();
        event.insert("type".into(), "error".into());
        event.insert("error".into(), protocol_error("cancelled", "the client cancelled the operation"));
        let line = self.line(event);
        let mut out = self.out.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = out.write_all(line.as_bytes());
        let _ = out.flush();
        exit(EXIT_CANCELLED);
    }
}

/// Put `protocol`, `id`, and `type` first so a person reading a trace sees them.
fn reorder(event: Map<String, Value>) -> Value {
    let mut out = Map::new();
    for key in ["protocol", "id", "type"] {
        if let Some(value) = event.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    for (key, value) in event {
        if !out.contains_key(&key) {
            out.insert(key, value);
        }
    }
    Value::Object(out)
}

/// One answer from the client to a callback.
#[derive(Debug)]
struct CallbackAnswer {
    callback_id: String,
    output: std::result::Result<Value, String>,
}

/// What an operation handler can use.
pub struct Call<'a> {
    pub input: Value,
    operation: &'static Operation,
    target: Option<Target>,
    store: Option<Store>,
    events: &'a Events,
    callbacks: Callbacks<'a>,
}

/// The client functions an operation can call, separate from the store so a
/// handler can hold both.
pub struct Callbacks<'a> {
    operation: &'static Operation,
    events: &'a Events,
    answers: Option<&'a Receiver<CallbackAnswer>>,
    next_callback: std::cell::Cell<u64>,
}

impl Callbacks<'_> {
    /// Ask the client to run the named callback and wait for its answer. The
    /// inner error is the message of a function that threw or rejected.
    pub fn call(&self, name: &str, input: Value) -> crate::Result<std::result::Result<Value, String>> {
        if !self.operation.callbacks.iter().any(|callback| callback.name == name) {
            return Err(Error::other(format!("{} has no callback named {name}", self.operation.name)));
        }
        let answers = self.answers.ok_or_else(|| Error::other("this operation cannot call back"))?;
        self.next_callback.set(self.next_callback.get() + 1);
        let callback_id = self.next_callback.get().to_string();
        let mut event = Map::new();
        event.insert("type".into(), "callback".into());
        event.insert("callback_id".into(), Value::String(callback_id.clone()));
        event.insert("name".into(), Value::String(name.to_string()));
        event.insert("input".into(), input);
        self.events.write(event, true);
        loop {
            let answer = answers.recv().map_err(|_| Error::other("the client stopped answering callbacks"))?;
            if answer.callback_id == callback_id {
                return Ok(answer.output);
            }
        }
    }
}

impl Call<'_> {
    /// The database for this request, opened on first use with the access
    /// the operation declares.
    pub fn store(&mut self) -> crate::Result<&mut Store> {
        if self.store.is_none() {
            let target = self
                .target
                .as_ref()
                .ok_or_else(|| Error::other(format!("{} needs a database target", self.operation.name)))?;
            let read_only = match self.operation.target {
                TargetUse::Read => true,
                TargetUse::Write => false,
                TargetUse::None => {
                    return Err(Error::other(format!("{} does not use a database target", self.operation.name)))
                }
            };
            self.store = Some(target.open(read_only)?);
        }
        Ok(self.store.as_mut().expect("opened above"))
    }

    /// The request's target, for operations that check it before opening.
    pub fn target(&self) -> Option<&Target> {
        self.target.as_ref()
    }

    /// Send one element of a streamed list.
    pub fn item(&self, item: Value) {
        let mut event = Map::new();
        event.insert("type".into(), "item".into());
        event.insert("item".into(), item);
        self.events.write(event, false);
    }

    /// Send a progress event.
    pub fn progress(&self, progress: Value) {
        let mut event = Map::new();
        event.insert("type".into(), "progress".into());
        event.insert("progress".into(), progress);
        self.events.write(event, true);
    }

    /// The database and the callbacks together, for operations that read
    /// while they call the client.
    pub fn store_and_callbacks(&mut self) -> crate::Result<(&mut Store, &Callbacks<'_>)> {
        self.store()?;
        Ok((self.store.as_mut().expect("opened above"), &self.callbacks))
    }
}

/// Run one protocol worker on this process's standard streams. Returns the
/// exit status.
pub fn serve() -> i32 {
    let stdin: Box<dyn Read + Send> = Box::new(std::io::stdin());
    let stdout: Box<dyn Write + Send> = Box::new(std::io::stdout());
    serve_streams(stdin, stdout, &|status| std::process::exit(status))
}

/// Run one worker on the given streams. `exit` ends the process at once; it
/// is called only when the client cancels.
pub fn serve_streams(
    input: Box<dyn Read + Send>,
    output: Box<dyn Write + Send>,
    exit: &'static (dyn Fn(i32) + Sync),
) -> i32 {
    let mut reader = BufReader::new(input);
    let mut line = String::new();
    let read = reader.read_line(&mut line);
    let request = match read {
        Ok(0) => Err(("protocol".to_string(), "the client sent no request".to_string(), String::new())),
        Ok(_) => parse_request(&line),
        Err(error) => Err(("protocol".to_string(), format!("the request could not be read: {error}"), String::new())),
    };
    let request = match request {
        Ok(request) => request,
        Err((code, message, id)) => {
            let events = Events { id, out: Mutex::new(BufWriter::with_capacity(EVENT_BUFFER_BYTES, output)) };
            write_error(&events, protocol_error(&code, &message));
            flush(&events);
            return EXIT_PROTOCOL;
        }
    };
    let events = Arc::new(Events {
        id: request.id.clone(),
        out: Mutex::new(BufWriter::with_capacity(EVENT_BUFFER_BYTES, output)),
    });
    let finished = Arc::new(AtomicBool::new(false));

    let operation = request.operation;
    let answers = if operation.kind == Kind::Async {
        let (sender, receiver) = mpsc::channel();
        let events = Arc::clone(&events);
        let finished = Arc::clone(&finished);
        std::thread::spawn(move || watch_input(reader, sender, &events, &finished, exit));
        Some(receiver)
    } else {
        None
    };

    let mut call = Call {
        input: request.input,
        operation,
        target: request.target,
        store: None,
        events: &events,
        callbacks: Callbacks {
            operation,
            events: &events,
            answers: answers.as_ref(),
            next_callback: std::cell::Cell::new(0),
        },
    };
    let outcome = (operation.run)(&mut call);
    let store = call.store.take();
    let outcome = finish_store(store, outcome);
    let status = match outcome {
        Ok((output, database_id)) => {
            let mut event = Map::new();
            event.insert("type".into(), "result".into());
            event.insert("output".into(), output);
            if let Some(database_id) = database_id {
                event.insert("database_id".into(), Value::String(database_id));
            }
            events.write(event, false);
            EXIT_OK
        }
        Err(failure) => {
            write_error(&events, failure_json(&failure));
            EXIT_FAILED
        }
    };
    finished.store(true, Ordering::SeqCst);
    flush(&events);
    status
}

/// Close the database after the operation. Report the database ID only when
/// the operation replaced it, as a restore into a new database does.
///
/// Each call runs in its own worker process, so replies can arrive in any
/// order. A read that started before a restore must not report the old ID
/// after the restore reported the new one, or the client would send the old
/// ID with every later request and have each one refused.
fn finish_store(store: Option<Store>, outcome: Outcome) -> std::result::Result<(Value, Option<String>), Failure> {
    let Some(mut store) = store else {
        return outcome.map(|output| (output, None));
    };
    let opened_with = store.metadata().database_id.clone();
    let database_id = if store.is_read_only() {
        None
    } else {
        store.reload_metadata().ok().map(|()| store.metadata().database_id.clone()).filter(|id| *id != opened_with)
    };
    let closed = store.close();
    let output = outcome?;
    closed?;
    Ok((output, database_id))
}

fn flush(events: &Events) {
    let mut out = events.out.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = out.flush();
}

fn write_error(events: &Events, error: Value) {
    let mut event = Map::new();
    event.insert("type".into(), "error".into());
    event.insert("error".into(), error);
    events.write(event, true);
}

struct Request {
    id: String,
    operation: &'static Operation,
    input: Value,
    target: Option<Target>,
}

type RequestError = (String, String, String);

/// Read the request line. The version is checked before anything else.
fn parse_request(line: &str) -> std::result::Result<Request, RequestError> {
    let value = json::parse(line)
        .map_err(|error| ("protocol".to_string(), format!("the request is not JSON: {error}"), String::new()))?;
    let id = value.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
    let fail = |code: &str, message: String| (code.to_string(), message, id.clone());
    match value.get("protocol").and_then(Value::as_u64) {
        Some(version) if version == u64::from(PROTOCOL_VERSION) => {}
        Some(version) => {
            return Err(fail(
                "protocol_version",
                format!(
                    "this ath speaks client protocol {PROTOCOL_VERSION}, not {version}; install matching athleticstandard and ath versions"
                ),
            ))
        }
        None => return Err(fail("protocol_version", "the request does not name a protocol version".into())),
    }
    if id.is_empty() {
        return Err(fail("protocol", "the request has no id".into()));
    }
    let name = value.get("op").and_then(Value::as_str).unwrap_or_default();
    let operation = definition::find(name).ok_or_else(|| fail("protocol", format!("unknown operation {name:?}")))?;
    let input = value.get("input").cloned().unwrap_or(Value::Null);
    let target = match value.get("target") {
        None | Some(Value::Null) => None,
        Some(target) => Some(Target::from_value(target).map_err(|message| fail("protocol", message))?),
    };
    if operation.target != TargetUse::None && target.is_none() {
        return Err(fail("protocol", format!("{name} needs a database target")));
    }
    Ok(Request { id, operation, input, target })
}

/// Read callback answers and cancellation until the operation finishes.
fn watch_input(
    mut reader: BufReader<Box<dyn Read + Send>>,
    sender: Sender<CallbackAnswer>,
    events: &Events,
    finished: &AtomicBool,
    exit: &(dyn Fn(i32) + Sync),
) {
    let mut line = String::new();
    loop {
        line.clear();
        let cancel = match reader.read_line(&mut line) {
            Ok(0) | Err(_) => true,
            Ok(_) => match json::parse(&line) {
                Ok(message) => match message.get("type").and_then(Value::as_str) {
                    Some("cancel") => true,
                    Some("callback_result") | Some("callback_error") => {
                        let callback_id =
                            message.get("callback_id").and_then(Value::as_str).unwrap_or_default().to_string();
                        let output = if message["type"] == "callback_result" {
                            Ok(message.get("output").cloned().unwrap_or(Value::Null))
                        } else {
                            Err(message
                                .get("error")
                                .and_then(|error| error.get("message"))
                                .and_then(Value::as_str)
                                .unwrap_or("the callback failed")
                                .to_string())
                        };
                        let _ = sender.send(CallbackAnswer { callback_id, output });
                        false
                    }
                    _ => false,
                },
                Err(_) => false,
            },
        };
        if cancel {
            if !finished.load(Ordering::SeqCst) {
                events.cancel(exit);
            }
            return;
        }
    }
}

fn protocol_error(code: &str, message: &str) -> Value {
    serde_json::json!({ "code": code, "message": message })
}

/// The JSON form of a failure for the client.
pub fn failure_json(failure: &Failure) -> Value {
    match failure {
        Failure::Error(error) => error_json(error),
        Failure::PackageRepresentation(details) => {
            let mut out = Map::new();
            out.insert("code".into(), "package_representation".into());
            if let Value::Object(details) = details {
                for (key, value) in details {
                    out.insert(key.clone(), value.clone());
                }
            }
            Value::Object(out)
        }
        Failure::Input(message) => protocol_error("input", message),
    }
}

/// The JSON form of a library error.
pub fn error_json(error: &Error) -> Value {
    let mut out = Map::new();
    out.insert("code".into(), error.code().into());
    out.insert("message".into(), error.to_string().into());
    match error {
        Error::Invalid(validation) => {
            out.insert("issues".into(), Value::Array(validation.issues.iter().map(|issue| issue.to_json()).collect()));
        }
        Error::MigrationRequired(preview) => {
            out.insert("preview".into(), json::to_value(preview.as_ref()));
        }
        _ => {}
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> (i32, Vec<Value>) {
        let output = Arc::new(Mutex::new(Vec::new()));
        struct Shared(Arc<Mutex<Vec<u8>>>);
        impl Write for Shared {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let status = serve_streams(
            Box::new(std::io::Cursor::new(input.as_bytes().to_vec())),
            Box::new(Shared(Arc::clone(&output))),
            &|status| panic!("exit {status}"),
        );
        let bytes = output.lock().unwrap().clone();
        let events = String::from_utf8(bytes).unwrap().lines().map(|line| json::parse(line).unwrap()).collect();
        (status, events)
    }

    #[test]
    fn another_protocol_version_is_refused_before_anything_runs() {
        let (status, events) = run("{\"protocol\":2,\"id\":\"1\",\"op\":\"database.metadata\",\"target\":{\"path\":\"/nonexistent/x.ath.sqlite\"}}\n");
        assert_eq!(status, EXIT_PROTOCOL);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "error");
        assert_eq!(events[0]["error"]["code"], "protocol_version");
    }

    #[test]
    fn a_missing_version_is_refused() {
        let (status, events) = run("{\"id\":\"1\",\"op\":\"ids.uuid_v7\"}\n");
        assert_eq!(status, EXIT_PROTOCOL);
        assert_eq!(events[0]["error"]["code"], "protocol_version");
    }

    /// A read reports no database ID, so a read that finishes after a
    /// restore cannot put the old ID back on the client's handle.
    #[test]
    fn a_read_reports_no_database_id() {
        let directory = crate::files::make_temporary_directory("ath-worker-read-id-").unwrap();
        let path = directory.join("athlete.ath.sqlite");
        Store::create(&path, crate::store::CreateOptions::default()).unwrap().close().unwrap();
        let mut request = Map::new();
        request.insert("protocol".into(), Value::from(PROTOCOL_VERSION));
        request.insert("id".into(), "4".into());
        request.insert("op".into(), "database.metadata".into());
        let mut target = Map::new();
        target.insert("path".into(), Value::String(path.display().to_string()));
        request.insert("target".into(), Value::Object(target));
        request.insert("input".into(), Value::Object(Map::new()));
        let (status, events) = run(&format!("{}\n", json::to_string(&Value::Object(request))));
        assert_eq!(status, EXIT_OK, "{events:?}");
        let result = events.iter().find(|event| event["type"] == "result").expect("a result");
        assert!(result["output"]["database_id"].is_string(), "{result:?}");
        assert!(result.get("database_id").is_none(), "{result:?}");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn an_unknown_operation_is_refused() {
        let (status, events) = run("{\"protocol\":1,\"id\":\"9\",\"op\":\"sql.execute\",\"input\":{}}\n");
        assert_eq!(status, EXIT_PROTOCOL);
        assert_eq!(events[0]["id"], "9");
        assert_eq!(events[0]["error"]["code"], "protocol");
    }

    #[test]
    fn a_sync_operation_returns_one_result() {
        let (status, events) = run("{\"protocol\":1,\"id\":\"3\",\"op\":\"ids.uuid_v7\",\"input\":{\"now\":0}}\n");
        assert_eq!(status, EXIT_OK, "{events:?}");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "result");
        let id = events[0]["output"].as_str().unwrap();
        assert!(id.starts_with("00000000-0000-7"), "{id}");
    }
}
