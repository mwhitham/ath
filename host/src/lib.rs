//! `ath-host`: an optional program that serves many athletes' Athletic Standard records
//! over HTTPS (build-history/v2.1.3/spec.md). Every record operation goes through the
//! Athletic Standard library. The host adds sign-in, grants, receipts, and storage.

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod directory;
pub mod error;
pub mod moves;
pub mod objects;
pub mod package_zip;
pub mod storage;

use std::sync::Arc;

use crate::api::AppState;
use crate::config::{Config, StorageConfig};
use crate::db::Db;
use crate::directory::Directory;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Open the directory the configuration names.
pub async fn open_directory(config: &Config) -> Result<Directory, String> {
    std::fs::create_dir_all(&config.data_directory)
        .map_err(|error| format!("cannot create the data folder: {error}"))?;
    let db = match &config.storage {
        StorageConfig::Libsql { directory_url, token_env, .. } => {
            Db::open_remote(directory_url, &Config::secret(token_env)?).await.map_err(|error| error.to_string())?
        }
        StorageConfig::Directory => {
            Db::open_local(&config.data_directory.join("host.sqlite")).map_err(|error| error.to_string())?
        }
        // Host copies that share athletes' databases share the directory too.
        StorageConfig::LocalServer { directory, .. } => {
            std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
            Db::open_local(&directory.join("host-directory.sqlite")).map_err(|error| error.to_string())?
        }
    };
    Directory::open(db).await.map_err(|error| error.to_string())
}

/// Everything a running host needs.
pub async fn state(config: Config) -> Result<Arc<AppState>, String> {
    let directory = open_directory(&config).await?;
    directory.set_setting_once("receipt_key", &directory::new_code()).await.map_err(|error| error.to_string())?;
    let receipt_key = directory
        .setting("receipt_key")
        .await
        .map_err(|error| error.to_string())?
        .ok_or("the receipt key is missing")?;
    let storage = storage::Storage::new(&config)?;
    storage.clear_working_copies();
    let verifier = auth::Verifier::new(config.sign_in.clone());
    Ok(Arc::new(AppState { config, directory, storage, verifier, receipt_key }))
}
