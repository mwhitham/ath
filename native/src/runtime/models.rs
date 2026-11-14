//! The live model list and the default model saved next to the athlete file (D78).
//!
//! The default is a model name, not a key. The key lives in the password
//! store. `athlete.ath.model` sits beside `athlete.ath.sqlite` and holds the
//! model ID and a newline.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::gateway::{closest_model_ids, CatalogModel, GatewayClient};
use super::keyring::Gateway;
use super::model_client::ModelError;
use crate::error::{Error, Result};

/// The live database suffix.
const FILE_SUFFIX: &str = ".ath.sqlite";

/// The portable package suffix.
const PORTABLE_FILE_SUFFIX: &str = ".ath.json";

/// The default-model file suffix.
pub const MODEL_SUFFIX: &str = ".ath.model";

/// The live list from one gateway. `ath models --json` prints this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Catalog {
    pub gateway: Gateway,
    pub models: Vec<CatalogModel>,
}

/// `ath models --default <name> --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SavedDefaultModel {
    /// The model ID saved.
    #[serde(rename = "default")]
    pub model: String,
    /// The database the default sits beside.
    pub file: PathBuf,
    pub gateway: Gateway,
}

/// The model one run will call, with the list it was checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModel {
    pub gateway: Gateway,
    pub model: String,
    pub models: Vec<CatalogModel>,
}

/// `athlete.ath.model` for `athlete.ath.sqlite`. A path without the live
/// database suffix maps to itself, which [`write_default_model`] refuses.
pub fn default_model_path(athlete_path: &Path) -> PathBuf {
    match athlete_path.to_str().and_then(|text| text.strip_suffix(FILE_SUFFIX)) {
        Some(stem) => PathBuf::from(format!("{stem}{MODEL_SUFFIX}")),
        None => athlete_path.to_path_buf(),
    }
}

/// The saved default model: the first line of the model file, trimmed.
pub fn read_default_model(athlete_path: &Path) -> Result<Option<String>> {
    let path = default_model_path(athlete_path);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path)?;
    let text = String::from_utf8_lossy(&bytes);
    let first = text.trim().split('\n').next().unwrap_or("").trim_end_matches('\r').trim().to_string();
    Ok((!first.is_empty()).then_some(first))
}

/// Save `model` as the default next to the athlete database.
///
/// The file is written beside it and renamed into place. A symbolic link at
/// the model path, the database itself, and a portable package are refused.
pub fn write_default_model(athlete_path: &Path, model: &str) -> Result<()> {
    let path = default_model_path(athlete_path);
    refuse_unsafe_model_path(athlete_path, &path)?;
    refuse_model_link(athlete_path, &path)?;
    write_model_file(athlete_path, &path, &format!("{model}\n"))
}

fn refuse_unsafe_model_path(athlete_path: &Path, model_path: &Path) -> Result<()> {
    let text = model_path.to_string_lossy();
    if model_path == athlete_path || text.ends_with(FILE_SUFFIX) || text.ends_with(PORTABLE_FILE_SUFFIX) {
        return Err(Error::refused(format!(
            "refusing to write the default model to {}; expected {MODEL_SUFFIX} next to a live {FILE_SUFFIX} database",
            model_path.display()
        )));
    }
    Ok(())
}

fn refuse_model_link(athlete_path: &Path, model_path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(model_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        return Err(Error::refused(format!(
            "refusing to write the default model to {}; it is a symbolic link",
            model_path.display()
        )));
    }
    let real = |path: &Path| match fs::canonicalize(path) {
        Ok(real) => Ok(Some(real)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    };
    if let (Some(athlete), Some(model)) = (real(athlete_path)?, real(model_path)?) {
        if athlete == model {
            return Err(Error::refused(format!(
                "refusing to write the default model to {}; it is the athlete database",
                model_path.display()
            )));
        }
    }
    Ok(())
}

fn temporary_name() -> String {
    let id = uuid::Uuid::now_v7().simple().to_string();
    format!(".model-{}-{}.tmp", std::process::id(), &id[id.len() - 12..])
}

fn write_model_file(athlete_path: &Path, model_path: &Path, contents: &str) -> Result<()> {
    let directory = model_path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let temporary = directory.join(temporary_name());
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644);
    }
    let written = options.open(&temporary).and_then(|mut file| file.write_all(contents.as_bytes()));
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    let replaced = replace_model_file(&temporary, athlete_path, model_path);
    if replaced.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    replaced
}

/// Rename into place. `rename` replaces the name and does not follow a link.
fn replace_model_file(temporary: &Path, athlete_path: &Path, model_path: &Path) -> Result<()> {
    refuse_model_link(athlete_path, model_path)?;
    match fs::rename(temporary, model_path) {
        Ok(()) => Ok(()),
        Err(error) if matches!(error.kind(), io::ErrorKind::AlreadyExists | io::ErrorKind::PermissionDenied) => {
            refuse_model_link(athlete_path, model_path)?;
            fs::remove_file(model_path)?;
            fs::rename(temporary, model_path)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

/// Fetch the live list from a connected gateway.
pub fn load_catalog(client: &GatewayClient) -> std::result::Result<Catalog, ModelError> {
    Ok(Catalog { gateway: client.gateway(), models: client.list_reasoning_models()? })
}

/// The exact ID on the list, or the ID that matches without case.
pub fn require_on_list(id: &str, models: &[CatalogModel]) -> std::result::Result<String, ModelError> {
    if let Some(exact) = models.iter().find(|model| model.id == id) {
        return Ok(exact.id.clone());
    }
    let lowered = id.to_lowercase();
    if let Some(found) = models.iter().find(|model| model.id.to_lowercase() == lowered) {
        return Ok(found.id.clone());
    }
    Err(ModelError::UnknownModel { id: id.to_string(), closest: closest_model_ids(id, models, 5) })
}

/// The model this run will call: `--model`, then the saved default. Neither
/// is a refusal. Nothing is guessed, and an earlier backtest winner is not
/// reused (D76, D78).
pub fn choose_model(athlete_path: &Path, requested: Option<&str>, models: &[CatalogModel]) -> Result<String> {
    let requested = requested.map(str::trim).filter(|value| !value.is_empty());
    let chosen = match requested {
        Some(value) => value.to_string(),
        None => read_default_model(athlete_path)?.ok_or(ModelError::NoModel)?,
    };
    Ok(require_on_list(&chosen, models)?)
}

/// Load the live list, then choose the model. A missing key is reported
/// before a missing model choice, as in 2.1.0.
pub fn resolve_model(client: &GatewayClient, athlete_path: &Path, requested: Option<&str>) -> Result<ResolvedModel> {
    let catalog = load_catalog(client)?;
    let model = choose_model(athlete_path, requested, &catalog.models)?;
    Ok(ResolvedModel { gateway: catalog.gateway, model, models: catalog.models })
}

/// `ath models --default <name>`: check the name against the list, then save it.
pub fn save_default_model(athlete_path: &Path, requested: &str, catalog: &Catalog) -> Result<SavedDefaultModel> {
    let model = require_on_list(requested, &catalog.models)?;
    write_default_model(athlete_path, &model)?;
    Ok(SavedDefaultModel { model, file: athlete_path.to_path_buf(), gateway: catalog.gateway })
}

/// The line `ath models --default <name>` prints.
pub fn saved_default_message(saved: &SavedDefaultModel) -> String {
    format!("saved {} as the default model next to the athlete file.", saved.model)
}

/// The human `ath models` list.
pub fn render_model_list(models: &[CatalogModel], gateway: Gateway) -> String {
    let mut lines = vec![
        format!("{} live text models that can reason, from {gateway}.", models.len()),
        "Open weights are included. Image, audio, and embedding models are out.".to_string(),
        String::new(),
    ];
    for model in models {
        match &model.name {
            Some(name) if name != &model.id => lines.push(format!("  {}  ({name})", model.id)),
            _ => lines.push(format!("  {}", model.id)),
        }
    }
    if models.is_empty() {
        lines.push("  (none — the catalog marked no reasoning text models)".to_string());
    }
    lines.push(String::new());
    lines.push("Pick one:  ath predict fran --model <name>".to_string());
    lines.push("Save one:  ath models --default <name>".to_string());
    lines.join("\n")
}
