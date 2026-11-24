//! Test servers for the host: a sign-in key server, a stand-in S3 bucket, and the host
//! itself, each on a free local port. The signing key is a fixed, test-only Ed25519
//! key built from a seed below; it signs nothing outside these tests.

#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ath_host::config::{Config, Limits, S3Config, SignInConfig, StorageConfig};
use axum::body::Bytes;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde_json::{json, Value};

/// The test key's 32-byte seed and public key.
const SEED: &str = "4828f8020207fb11233d01b011c5937bede2b268a8603620b80bf97e050503d0";
const PUBLIC_X: &str = "itrbr0O5BuqdMUzuGWjVxNLAdQjldIGhKq8Rxhhr3_A";
pub const ISSUER: &str = "https://sign-in.test";
pub const AUDIENCE: &str = "https://host.test";
pub const ATHLETE_APP: &str = "athlete-app";
pub const COACH_APP: &str = "coach-app";

pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap())
}

fn serve(router: Router) -> SocketAddr {
    runtime().block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        address
    })
}

/// A key server publishing the test key.
pub fn key_server() -> String {
    let jwks = json!({ "keys": [{ "kty": "OKP", "crv": "Ed25519", "x": PUBLIC_X, "kid": "test-key", "alg": "EdDSA", "use": "sig" }] });
    let address = serve(Router::new().route("/jwks.json", get(move || async move { Json(jwks.clone()) })));
    format!("http://{address}/jwks.json")
}

fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

/// An access token for `subject`, issued to `client`.
pub fn token(subject: &str, client: &str) -> String {
    token_with(
        json!({ "iss": ISSUER, "sub": subject, "aud": AUDIENCE, "azp": client, "name": subject, "exp": far_future() }),
    )
}

pub fn token_with(claims: Value) -> String {
    let mut der = hex_bytes("302e020100300506032b657004220420");
    der.extend(hex_bytes(SEED));
    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = Some("test-key".into());
    encode(&header, &claims, &EncodingKey::from_ed_der(&der)).unwrap()
}

pub fn far_future() -> u64 {
    (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()) + 3600
}

/// A stand-in S3 bucket: path-style PUT, GET, and DELETE, keeping objects in memory.
/// It ignores signatures; the host's signing is covered by the library it uses.
#[derive(Clone, Default)]
pub struct Bucket {
    pub objects: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

pub fn bucket_server() -> (String, Bucket) {
    let bucket = Bucket::default();
    async fn put(State(bucket): State<Bucket>, UrlPath(key): UrlPath<String>, body: Bytes) -> StatusCode {
        bucket.objects.lock().unwrap().insert(key, body.to_vec());
        StatusCode::OK
    }
    async fn get_object(State(bucket): State<Bucket>, UrlPath(key): UrlPath<String>) -> Result<Vec<u8>, StatusCode> {
        bucket.objects.lock().unwrap().get(&key).cloned().ok_or(StatusCode::NOT_FOUND)
    }
    async fn remove(State(bucket): State<Bucket>, UrlPath(key): UrlPath<String>) -> StatusCode {
        bucket.objects.lock().unwrap().remove(&key);
        StatusCode::NO_CONTENT
    }
    let router =
        Router::new().route("/test-bucket/{*key}", get(get_object).put(put).delete(remove)).with_state(bucket.clone());
    let address = serve(router);
    (format!("http://{address}"), bucket)
}

pub fn temporary(prefix: &str) -> PathBuf {
    // The library refuses paths through a symbolic link, and the system's temporary
    // folder is often one, so use its real path.
    let root = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let path = root.join(format!("{prefix}{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

pub enum Mode<'a> {
    Directory,
    /// A real libSQL server, from the `ATH_QUALIFY_*` environment variables, with the
    /// stand-in bucket for attachments.
    Libsql {
        bucket: &'a str,
    },
    /// A local stand-in server folder shared by every host copy, and a bucket address.
    LocalServer {
        server: &'a Path,
        bucket: &'a str,
    },
}

pub fn config(data: &Path, mode: Mode<'_>, jwks: &str) -> Config {
    std::env::set_var("ATH_TEST_S3_KEY", "test");
    std::env::set_var("ATH_TEST_S3_SECRET", "test-secret");
    let test_bucket = |bucket: &str| S3Config {
        endpoint: bucket.to_string(),
        bucket: "test-bucket".into(),
        region: "test".into(),
        access_key_env: "ATH_TEST_S3_KEY".into(),
        secret_key_env: "ATH_TEST_S3_SECRET".into(),
        path_style: true,
    };
    let storage = match mode {
        Mode::Directory => StorageConfig::Directory,
        Mode::Libsql { bucket } => {
            let variable = |name: &str| {
                std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run the qualification suite"))
            };
            let provision = match variable("ATH_QUALIFY_PROVISION").as_str() {
                "existing" => ath_host::config::Provision::Existing,
                other if other.starts_with("sqld-admin:") => {
                    ath_host::config::Provision::SqldAdmin { admin_url: other["sqld-admin:".len()..].to_string() }
                }
                other if other.starts_with("turso:") => {
                    let mut parts = other["turso:".len()..].splitn(2, ':');
                    ath_host::config::Provision::Turso {
                        organization: parts.next().unwrap_or_default().to_string(),
                        group: parts.next().unwrap_or("default").to_string(),
                        platform_token_env: "ATH_QUALIFY_TURSO_PLATFORM_TOKEN".into(),
                    }
                }
                other => panic!("unknown ATH_QUALIFY_PROVISION: {other}"),
            };
            StorageConfig::Libsql {
                url_template: variable("ATH_QUALIFY_LIBSQL_URL_TEMPLATE"),
                directory_url: variable("ATH_QUALIFY_LIBSQL_DIRECTORY_URL"),
                token_env: "ATH_QUALIFY_LIBSQL_TOKEN".into(),
                provision,
                attachments: test_bucket(bucket),
            }
        }
        Mode::LocalServer { server, bucket } => {
            StorageConfig::LocalServer { directory: server.to_path_buf(), attachments: test_bucket(bucket) }
        }
    };
    Config {
        listen: "127.0.0.1:0".into(),
        public_url: AUDIENCE.into(),
        data_directory: data.to_path_buf(),
        storage,
        sign_in: SignInConfig {
            issuer: ISSUER.into(),
            audience: AUDIENCE.into(),
            jwks_uri: Some(jwks.into()),
            allowed_clients: Vec::new(),
        },
        operator_disclosure: "Test host run by the test suite, storing records on this machine.".into(),
        open_enrollment: true,
        limits: Limits::default(),
    }
}

pub struct Host {
    pub url: String,
    pub state: ath_host::api::Shared,
}

pub fn host(config: Config) -> Host {
    let state = runtime().block_on(ath_host::state(config)).unwrap();
    let address = serve(ath_host::api::router(state.clone()));
    Host { url: format!("http://{address}"), state }
}

/// A response's status and JSON body (or raw bytes).
pub struct Reply {
    pub status: u16,
    pub json: Value,
    pub bytes: Vec<u8>,
    pub headers: HashMap<String, String>,
}

pub fn call(method: &str, url: &str, token: &str, body: Option<Value>, key: Option<&str>) -> Reply {
    let mut request = ureq::request(method, url).set("Authorization", &format!("Bearer {token}"));
    if let Some(key) = key {
        request = request.set("Idempotency-Key", key);
    }
    let response = match body {
        Some(body) => request.send_json(body),
        None => request.call(),
    };
    reply(response)
}

pub fn send_bytes(method: &str, url: &str, token: &str, bytes: &[u8]) -> Reply {
    reply(ureq::request(method, url).set("Authorization", &format!("Bearer {token}")).send_bytes(bytes))
}

fn reply(response: Result<ureq::Response, ureq::Error>) -> Reply {
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(error) => panic!("request failed: {error}"),
    };
    let status = response.status();
    let headers = response
        .headers_names()
        .into_iter()
        .filter_map(|name| response.header(&name).map(|v| (name, v.to_string())))
        .collect();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes).unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Reply { status, json, bytes, headers }
}

pub fn demo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/demo-athlete")
}

/// The demo record's complete package as a zip.
pub fn demo_package_zip() -> Vec<u8> {
    let folder = temporary("ath-host-demo-package-");
    let copy = folder.join("record");
    copy_folder(&demo_root(), &copy);
    let store = athleticstandard::store::Store::open(copy.join("athlete.ath.sqlite"), Default::default()).unwrap();
    let package = folder.join("package");
    athleticstandard::package::export_package(&store, &package, &Default::default()).unwrap();
    ath_host::package_zip::pack(&package).unwrap()
}

pub fn demo_photo() -> Vec<u8> {
    std::fs::read(demo_root().join("attachments/01890f3e-e7d3-7cc0-88c8-00000000000e.png")).unwrap()
}

pub fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_folder(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// An athlete report as an owner would log it.
pub fn report(note: &str) -> Value {
    json!({
        "report_type": "note",
        "reported_at": "2026-11-21T08:00:00.000Z",
        "input_reported_at": { "timestamp_missing_reason": "not_applicable", "utc_offset_missing_reason": "not_applicable" },
        "reported_via": "text",
        "note": note,
    })
}

/// Restore an exported zip into a new local database, as a second device would, and
/// return the store.
pub fn open_on_another_device(zip: &[u8]) -> (PathBuf, athleticstandard::store::Store) {
    let folder = temporary("ath-host-device-");
    let package = folder.join("package");
    ath_host::package_zip::unpack(zip, &package).unwrap();
    let path = folder.join("device").join("athlete.ath.sqlite");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut store = athleticstandard::store::Store::create(&path, Default::default()).unwrap();
    let preview = athleticstandard::package::preview_package_import(&store, &package).unwrap();
    athleticstandard::package::apply_package_import(&mut store, &preview, &[]).unwrap();
    (path, store)
}
