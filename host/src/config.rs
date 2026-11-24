//! The host's configuration: a JSON file. Secrets are never in the file. The file
//! names environment variables that hold them.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Address and port to listen on, such as `0.0.0.0:8080`.
    pub listen: String,
    /// The address clients use, such as `https://records.example.org`. It is the
    /// resource identifier in the protected-resource metadata (RFC 9728).
    pub public_url: String,
    /// The host's own folder: its directory database, staging files, and, for the
    /// directory backend, every athlete's database.
    pub data_directory: PathBuf,
    pub storage: StorageConfig,
    pub sign_in: SignInConfig,
    /// Text shown to an athlete when they connect: who runs this host and its storage
    /// (D264).
    pub operator_disclosure: String,
    /// When true, any signed-in person can create their own record. When false, a
    /// person needs an invitation from `ath-host invite`.
    #[serde(default)]
    pub open_enrollment: bool,
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StorageConfig {
    /// Native SQLite files in `data_directory/athletes/` (D257).
    Directory,
    /// One database per athlete on a server that speaks the libSQL protocol, with
    /// attachments in an S3-compatible bucket (D257, D259, D268).
    Libsql {
        /// The database address for one athlete, with `{database}` where the database
        /// name goes, such as `libsql://{database}-myorg.turso.io`.
        url_template: String,
        /// The host's directory database.
        directory_url: String,
        /// Environment variable holding the token for every database.
        token_env: String,
        provision: Provision,
        attachments: S3Config,
    },
    /// Test stand-in for a server: each athlete's "server" database is a local file
    /// in this folder, committed to with the same guarded transaction (D268).
    LocalServer { directory: PathBuf, attachments: S3Config },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Provision {
    /// The operator creates each database before use.
    Existing,
    /// Turso's Platform API creates each database.
    Turso { organization: String, group: String, platform_token_env: String },
    /// A self-run libSQL server with namespaces creates each database through its
    /// admin address.
    SqldAdmin { admin_url: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct S3Config {
    /// Such as `https://<account>.r2.cloudflarestorage.com` or `https://s3.amazonaws.com`.
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key_env: String,
    pub secret_key_env: String,
    /// Path-style addresses (`endpoint/bucket/key`). Most S3-compatible services
    /// accept them.
    #[serde(default = "yes")]
    pub path_style: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignInConfig {
    /// The OpenID Connect issuer, exactly as it appears in tokens (D260).
    pub issuer: String,
    /// The audience tokens for this API must name.
    pub audience: String,
    /// Where to read signing keys. When absent, the issuer's discovery document says.
    #[serde(default)]
    pub jwks_uri: Option<String>,
    /// Client IDs this host accepts. When empty, any client the provider issued a
    /// token to is accepted; grants still name the client.
    #[serde(default)]
    pub allowed_clients: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Largest request body, such as an uploaded package or attachment.
    #[serde(default = "Limits::default_body_bytes")]
    pub body_bytes: usize,
    /// Largest number of records in one list page.
    #[serde(default = "Limits::default_page_size")]
    pub page_size: u32,
    /// How long an idle working copy is kept, in seconds (D268).
    #[serde(default = "Limits::default_working_copy_idle_seconds")]
    pub working_copy_idle_seconds: u64,
}

impl Limits {
    fn default_body_bytes() -> usize {
        64 * 1024 * 1024
    }
    fn default_page_size() -> u32 {
        200
    }
    fn default_working_copy_idle_seconds() -> u64 {
        15 * 60
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            body_bytes: Limits::default_body_bytes(),
            page_size: Limits::default_page_size(),
            working_copy_idle_seconds: Limits::default_working_copy_idle_seconds(),
        }
    }
}

impl Config {
    pub fn read(path: &Path) -> Result<Config, String> {
        let text = std::fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let mut config: Config = serde_json::from_str(&text)
            .map_err(|error| format!("{} is not a valid configuration: {error}", path.display()))?;
        if config.data_directory.is_relative() {
            config.data_directory = path.parent().unwrap_or(Path::new(".")).join(&config.data_directory);
        }
        Ok(config)
    }

    pub fn secret(name: &str) -> Result<String, String> {
        std::env::var(name).map_err(|_| format!("the environment variable {name} is not set"))
    }
}
