//! Backups and restores of one athlete at a time (D264).
//!
//! A backup is each athlete's complete package as a zip, checked by restoring it into
//! an empty database before it is kept. A restore replaces one athlete's record from
//! such a package, then applies every erasure recorded since, and raises the access
//! epoch so a write that started before the restore cannot commit after it.

use std::path::{Path, PathBuf};

use athleticstandard::store::{CreateOptions, Store};

use crate::api::{erase_again, export_zip, AppState};
use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackedUp {
    pub athlete_id: String,
    pub file: PathBuf,
    pub manifest_digest: String,
}

/// Back up every athlete that is not retired into `output/<athlete>/<time>-<digest>.zip`.
pub async fn backup_all(state: &AppState, output: &Path) -> ApiResult<Vec<BackedUp>> {
    let athletes = state.directory.all_athletes().await.map_err(|error| ApiError::internal("directory", error))?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let mut done = Vec::new();
    for athlete in athletes.iter().filter(|athlete| athlete.state == "active" || athlete.state == "frozen") {
        let (bytes, digest) = export_zip(state, &athlete.resource_id).await?;
        verify_package(state, &bytes).await?;
        let folder = output.join(&athlete.resource_id);
        std::fs::create_dir_all(&folder).map_err(|error| ApiError::internal("writing a backup", error))?;
        let file = folder.join(format!("{stamp}-{}.zip", &digest[..12]));
        let partial = file.with_extension("zip.partial");
        std::fs::write(&partial, &bytes).map_err(|error| ApiError::internal("writing a backup", error))?;
        std::fs::rename(&partial, &file).map_err(|error| ApiError::internal("writing a backup", error))?;
        done.push(BackedUp { athlete_id: athlete.resource_id.clone(), file, manifest_digest: digest });
    }
    Ok(done)
}

/// Restore the package into an empty database in the staging folder and check it.
async fn verify_package(state: &AppState, bytes: &[u8]) -> ApiResult<()> {
    let folder = state.storage.staging().join("verify").join(uuid::Uuid::new_v4().simple().to_string());
    let package = folder.join("package");
    crate::package_zip::unpack(bytes, &package).map_err(|error| ApiError::internal("checking a backup", error))?;
    let database = folder.join("record").join("athlete.ath.sqlite");
    std::fs::create_dir_all(database.parent().expect("record folder"))
        .map_err(|error| ApiError::internal("checking a backup", error))?;
    let result = tokio::task::spawn_blocking(move || -> athleticstandard::Result<()> {
        let mut store = Store::create(&database, CreateOptions::default())?;
        let preview = athleticstandard::package::preview_package_import(&store, &package)?;
        athleticstandard::package::apply_package_import(&mut store, &preview, &[])?;
        let report = athleticstandard::store::check::check_database(&store)?;
        if !report.valid {
            return Err(athleticstandard::Error::other("the restored backup did not pass the database check"));
        }
        Ok(())
    })
    .await
    .map_err(|error| ApiError::internal("checking a backup", error))?;
    let _ = std::fs::remove_dir_all(&folder);
    result.map_err(|error| ApiError::internal("checking a backup", error))
}

/// Replace one athlete's record with a backup, then reapply later erasures and raise
/// the access epoch. Directory mode only: in remote mode, use the provider's restore
/// for the athlete's database (see docs/hosting.md).
pub async fn restore_athlete(state: &AppState, athlete_id: &str, zip: &[u8]) -> ApiResult<u64> {
    state
        .directory
        .athlete(athlete_id)
        .await
        .map_err(|error| ApiError::internal("directory", error))?
        .ok_or_else(ApiError::not_found)?;
    let folder = state.storage.staging().join("restore").join(uuid::Uuid::new_v4().simple().to_string());
    crate::package_zip::unpack(zip, &folder)
        .map_err(|error| ApiError::invalid(format!("The backup is not a readable zip: {error}")))?;
    let result = state.storage.replace_from_package(athlete_id, &folder).await;
    let _ = std::fs::remove_dir_all(&folder);
    result?;
    for (intent, record_id, reason) in
        state.directory.erasure_intents(athlete_id).await.map_err(|error| ApiError::internal("directory", error))?
    {
        erase_again(state, athlete_id, &record_id, &reason).await?;
        state
            .directory
            .complete_erasure_intent(&intent)
            .await
            .map_err(|error| ApiError::internal("directory", error))?;
    }
    state.storage.raise_access_epoch(athlete_id).await?;
    state.storage.read(athlete_id, |store| store.database_revision()).await
}

/// Finish erasures that were requested but not completed, as the host does when it
/// starts (D264).
pub async fn finish_erasures(state: &AppState) -> ApiResult<usize> {
    let unfinished =
        state.directory.unfinished_erasure_intents().await.map_err(|error| ApiError::internal("directory", error))?;
    for (intent, athlete_id, record_id, reason) in &unfinished {
        erase_again(state, athlete_id, record_id, reason).await?;
        state
            .directory
            .complete_erasure_intent(intent)
            .await
            .map_err(|error| ApiError::internal("directory", error))?;
    }
    Ok(unfinished.len())
}
