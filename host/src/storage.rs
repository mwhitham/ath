//! Where athletes' databases live and how the host reads and writes them (D256, D257,
//! D268).
//!
//! **Directory mode.** Each athlete has a private folder in `data/athletes/` with an
//! ordinary database and its attachments. One host process owns the data folder; a
//! lock per athlete orders that athlete's writes.
//!
//! **Remote mode.** Each athlete has a database on a libSQL server (or, for tests, a
//! local file standing in for one) and attachments in an S3-compatible bucket. The
//! host keeps a working copy in `data/working/`. Every operation runs on the working
//! copy through the library, unchanged. A write's row changes are then committed to
//! the server in one transaction that first checks the database revision and access
//! epoch. If the check fails, the copy is discarded and the request runs again.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use athleticstandard::store::replica::{
    schema_scripts, Changeset, ReplayStatement, SqlValue, TableRows, TableShape, NOT_COPIED,
};
use athleticstandard::store::{CreateOptions, OpenOptions, Store};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::config::{Config, Provision, StorageConfig};
use crate::db::{first_integer, integer, text, Db};
use crate::error::{ApiError, ApiResult};
use crate::objects::ObjectStore;

/// How many times a write runs again after the server moved on.
const ATTEMPTS: usize = 3;

pub struct Storage {
    data: PathBuf,
    mode: Mode,
    locks: std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

enum Mode {
    Directory,
    Remote(Box<Remote>),
}

struct Remote {
    servers: Servers,
    objects: ObjectStore,
    shapes: Vec<TableShape>,
    idle: Duration,
    connections: Mutex<HashMap<String, Db>>,
    last_used: std::sync::Mutex<HashMap<String, Instant>>,
}

enum Servers {
    Libsql { url_template: String, token: String, provision: Provision },
    Local { directory: PathBuf },
}

impl Storage {
    pub fn new(config: &Config) -> Result<Storage, String> {
        let data = config.data_directory.clone();
        std::fs::create_dir_all(data.join("staging"))
            .map_err(|error| format!("cannot create the data folder: {error}"))?;
        let mode = match &config.storage {
            StorageConfig::Directory => {
                std::fs::create_dir_all(data.join("athletes")).map_err(|error| error.to_string())?;
                Mode::Directory
            }
            StorageConfig::Libsql { url_template, token_env, provision, attachments, .. } => {
                Mode::Remote(Box::new(Remote::new(
                    &data,
                    Servers::Libsql {
                        url_template: url_template.clone(),
                        token: Config::secret(token_env)?,
                        provision: provision.clone(),
                    },
                    ObjectStore::new(attachments)?,
                    config.limits.working_copy_idle_seconds,
                )?))
            }
            StorageConfig::LocalServer { directory, attachments } => {
                std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
                Mode::Remote(Box::new(Remote::new(
                    &data,
                    Servers::Local { directory: directory.clone() },
                    ObjectStore::new(attachments)?,
                    config.limits.working_copy_idle_seconds,
                )?))
            }
        };
        Ok(Storage { data, mode, locks: std::sync::Mutex::new(HashMap::new()) })
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.mode, Mode::Remote(_))
    }

    pub fn staging(&self) -> PathBuf {
        self.data.join("staging")
    }

    /// The athlete's folder: the record in directory mode, the working copy in remote
    /// mode.
    fn folder(&self, resource_id: &str) -> PathBuf {
        match self.mode {
            Mode::Directory => self.data.join("athletes").join(resource_id),
            Mode::Remote(_) => self.data.join("working").join(resource_id),
        }
    }

    fn database(&self, resource_id: &str) -> PathBuf {
        self.folder(resource_id).join("athlete.ath.sqlite")
    }

    async fn lock(&self, resource_id: &str) -> OwnedMutexGuard<()> {
        let lock = self.locks.lock().expect("lock map").entry(resource_id.to_string()).or_default().clone();
        lock.lock_owned().await
    }

    /// Create a new, empty record for a new athlete.
    pub async fn create(&self, resource_id: &str) -> ApiResult<u64> {
        let _guard = self.lock(resource_id).await;
        match &self.mode {
            Mode::Directory => {
                let folder = self.folder(resource_id);
                std::fs::create_dir_all(&folder)
                    .map_err(|error| ApiError::internal("creating a record folder", error))?;
                let path = self.database(resource_id);
                blocking(move || Store::create(&path, CreateOptions::default())?.database_revision()).await
            }
            Mode::Remote(remote) => {
                let server = remote.provision(resource_id).await?;
                let database_id = athleticstandard::ids::new_record_id();
                let created_at = crate::directory::now();
                for script in schema_scripts(&database_id, &created_at) {
                    server
                        .execute_batch(&script)
                        .await
                        .map_err(|error| ApiError::internal("creating a database", error))?;
                }
                remote.current(self, resource_id).await
            }
        }
    }

    /// Run a read on the athlete's record.
    pub async fn read<T: Send + 'static>(
        &self,
        resource_id: &str,
        read: impl FnOnce(&Store) -> athleticstandard::Result<T> + Send + 'static,
    ) -> ApiResult<T> {
        let _guard = self.lock(resource_id).await;
        if let Mode::Remote(remote) = &self.mode {
            remote.current(self, resource_id).await?;
        }
        let path = self.database(resource_id);
        blocking(move || {
            let store = Store::open(&path, OpenOptions::default())?;
            read(&store)
        })
        .await
    }

    /// Run a write. `guard_epoch` is the access epoch read before the grant check, for
    /// a write made under a grant (D262). The write is given it so the library can
    /// check it in directory mode; in remote mode the server check uses it.
    pub async fn write<T: Send + 'static>(
        &self,
        resource_id: &str,
        guard_epoch: Option<u64>,
        write: impl Fn(&mut Store, Option<u64>) -> athleticstandard::Result<T> + Send + Sync + 'static,
    ) -> ApiResult<T> {
        let write = Arc::new(write);
        match &self.mode {
            Mode::Directory => {
                let _guard = self.lock(resource_id).await;
                let path = self.database(resource_id);
                let value = blocking(move || {
                    let mut store = Store::open(&path, OpenOptions::default())?;
                    write(&mut store, guard_epoch)
                })
                .await?;
                Ok(value)
            }
            Mode::Remote(remote) => {
                for attempt in 1..=ATTEMPTS {
                    let _guard = self.lock(resource_id).await;
                    remote.current(self, resource_id).await?;
                    let (_, epoch) = remote.revision_and_epoch(resource_id).await?;
                    if let Some(expected) = guard_epoch {
                        if expected != epoch {
                            return Err(ApiError::stale(
                                "Access to this record changed while the request was waiting; check the grant and try again",
                            ));
                        }
                    }
                    let path = self.database(resource_id);
                    let attachments = path.parent().expect("record folder").join("attachments");
                    let before = file_names(&attachments);
                    let write = write.clone();
                    let captured = blocking(move || {
                        let mut store = Store::open(&path, OpenOptions::default())?;
                        let captured = store.capture(|store| write(store, None))?;
                        let statements = store.replay_statements(&captured.changeset)?;
                        Ok((captured, statements))
                    })
                    .await;
                    let (captured, statements) = match captured {
                        Ok(result) => result,
                        Err(error) => {
                            // The library rolled the operation back; the copy is intact.
                            return Err(error);
                        }
                    };
                    if captured.changeset.is_empty() {
                        return Ok(captured.value);
                    }
                    let after = file_names(&attachments);
                    let outcome = async {
                        for name in after.difference(&before) {
                            let bytes = std::fs::read(attachments.join(name))
                                .map_err(|error| ApiError::internal("reading a new attachment", error))?;
                            remote.objects.put(&ObjectStore::key(resource_id, name), bytes).await?;
                        }
                        remote
                            .commit(
                                resource_id,
                                captured.base_revision,
                                captured.new_revision,
                                guard_epoch.unwrap_or(epoch),
                                &captured.changeset,
                                &statements,
                            )
                            .await?;
                        for name in before.difference(&after) {
                            remote.objects.delete(&ObjectStore::key(resource_id, name)).await?;
                        }
                        Ok::<(), ApiError>(())
                    }
                    .await;
                    match outcome {
                        Ok(()) => return Ok(captured.value),
                        Err(error) => {
                            // The copy is ahead of the server. Discard it.
                            remote.discard(self, resource_id);
                            if error.code == "stale" && attempt < ATTEMPTS && guard_epoch.is_none() {
                                continue;
                            }
                            return Err(error);
                        }
                    }
                }
                Err(ApiError::stale("The record kept changing; try again"))
            }
        }
    }

    /// The access epoch: in the record in directory mode, on the server in remote mode.
    pub async fn access_epoch(&self, resource_id: &str) -> ApiResult<u64> {
        match &self.mode {
            Mode::Directory => self.read(resource_id, |store| store.current_access_epoch()).await,
            Mode::Remote(remote) => Ok(remote.revision_and_epoch(resource_id).await?.1),
        }
    }

    /// Raise the access epoch after a grant is revoked (D262).
    pub async fn raise_access_epoch(&self, resource_id: &str) -> ApiResult<u64> {
        let _guard = self.lock(resource_id).await;
        match &self.mode {
            Mode::Directory => {
                let path = self.database(resource_id);
                blocking(move || Store::open(&path, OpenOptions::default())?.raise_access_epoch()).await
            }
            Mode::Remote(remote) => {
                let server = remote.server(resource_id).await?;
                server
                    .execute(
                        "UPDATE internal_access_state SET access_epoch = access_epoch + 1 WHERE singleton_id = 1",
                        Vec::new(),
                    )
                    .await
                    .map_err(|error| ApiError::internal("raising the access epoch", error))?;
                Ok(remote.revision_and_epoch(resource_id).await?.1)
            }
        }
    }

    /// Make sure every attachment file the record names is in the local folder, so
    /// export and download work. In directory mode they always are.
    pub async fn ensure_attachment_files(&self, resource_id: &str) -> ApiResult<()> {
        let Mode::Remote(remote) = &self.mode else {
            return Ok(());
        };
        let path = self.database(resource_id);
        let names = self
            .read(resource_id, move |store| {
                let records = store.list_records(&athleticstandard::store::read::ListOptions {
                    record_type: Some(athleticstandard::model::RecordType::Attachment),
                    include_deleted: true,
                    limit: 10_000,
                })?;
                Ok(records
                    .into_iter()
                    .filter_map(|record| match &record.data {
                        athleticstandard::model::RecordData::Attachment(data) => {
                            Some(athleticstandard::files::attachments::stored_attachment_name(&record.record_id, data))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>())
            })
            .await?;
        let folder = path.parent().expect("record folder").join("attachments");
        for name in names {
            let target = folder.join(&name);
            if target.exists() {
                continue;
            }
            let bytes = remote.objects.get(&ObjectStore::key(resource_id, &name)).await?.ok_or_else(|| {
                ApiError::internal("reading an attachment", format!("missing object for {resource_id}"))
            })?;
            std::fs::create_dir_all(&folder)
                .map_err(|error| ApiError::internal("creating an attachment folder", error))?;
            std::fs::write(&target, bytes).map_err(|error| ApiError::internal("writing an attachment", error))?;
        }
        Ok(())
    }

    /// The bytes of one attachment file, after the permission check.
    pub async fn attachment_bytes(&self, resource_id: &str, stored_name: &str) -> ApiResult<Vec<u8>> {
        self.ensure_attachment_files(resource_id).await?;
        let path = self.folder(resource_id).join("attachments").join(stored_name);
        std::fs::read(&path).map_err(|_| ApiError::not_found())
    }

    /// Replace one athlete's record with the package in `package` (D264). Directory
    /// mode only. The old folder is kept beside it, renamed, until the operator removes
    /// it.
    pub async fn replace_from_package(&self, resource_id: &str, package: &Path) -> ApiResult<()> {
        if self.is_remote() {
            return Err(ApiError::refused(
                "In remote mode, restore the athlete's database with the provider's restore; see docs/hosting.md",
            ));
        }
        let _guard = self.lock(resource_id).await;
        let folder = self.folder(resource_id);
        let staged = self.data.join("athletes").join(format!("{resource_id}.restoring"));
        let _ = std::fs::remove_dir_all(&staged);
        std::fs::create_dir_all(&staged).map_err(|error| ApiError::internal("restoring a record", error))?;
        let database = staged.join("athlete.ath.sqlite");
        let package = package.to_path_buf();
        blocking(move || {
            let mut store = Store::create(&database, CreateOptions::default())?;
            let preview = athleticstandard::package::preview_package_import(&store, &package)?;
            athleticstandard::package::apply_package_import(&mut store, &preview, &[])?;
            Ok(())
        })
        .await?;
        let replaced = self
            .data
            .join("athletes")
            .join(format!("{resource_id}.replaced-{}", chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ")));
        if folder.exists() {
            std::fs::rename(&folder, &replaced).map_err(|error| ApiError::internal("restoring a record", error))?;
        }
        std::fs::rename(&staged, &folder).map_err(|error| ApiError::internal("restoring a record", error))?;
        Ok(())
    }

    /// Delete idle working copies (D268). Directory mode keeps every record.
    pub fn sweep_idle(&self) {
        if let Mode::Remote(remote) = &self.mode {
            let idle: Vec<String> = remote
                .last_used
                .lock()
                .expect("last-used map")
                .iter()
                .filter(|(_, used)| used.elapsed() > remote.idle)
                .map(|(id, _)| id.clone())
                .collect();
            for id in idle {
                if let Ok(lock) =
                    self.locks.lock().expect("lock map").entry(id.clone()).or_default().clone().try_lock_owned()
                {
                    remote.discard(self, &id);
                    drop(lock);
                }
            }
        }
    }

    /// Remove every working copy, as the host does when it starts (D268).
    pub fn clear_working_copies(&self) {
        if self.is_remote() {
            let _ = std::fs::remove_dir_all(self.data.join("working"));
        }
    }
}

impl Remote {
    fn new(data: &Path, servers: Servers, objects: ObjectStore, idle_seconds: u64) -> Result<Remote, String> {
        let working = data.join("working");
        std::fs::create_dir_all(&working).map_err(|error| error.to_string())?;
        let template = data.join("staging").join("schema-template.ath.sqlite");
        let _ = std::fs::remove_file(&template);
        let store = Store::create(&template, CreateOptions::default()).map_err(|error| error.to_string())?;
        let shapes = store.table_shapes().map_err(|error| error.to_string())?;
        drop(store);
        let _ = std::fs::remove_file(&template);
        Ok(Remote {
            servers,
            objects,
            shapes,
            idle: Duration::from_secs(idle_seconds),
            connections: Mutex::new(HashMap::new()),
            last_used: std::sync::Mutex::new(HashMap::new()),
        })
    }

    fn database_name(resource_id: &str) -> String {
        resource_id.replace('_', "-")
    }

    async fn server(&self, resource_id: &str) -> ApiResult<Db> {
        if let Some(db) = self.connections.lock().await.get(resource_id) {
            return Ok(db.clone());
        }
        let db = match &self.servers {
            Servers::Local { directory } => {
                let path = directory.join(format!("{}.sqlite", Remote::database_name(resource_id)));
                if !path.exists() {
                    return Err(ApiError::not_found());
                }
                Db::open_local(&path).map_err(|error| ApiError::internal("opening a server database", error))?
            }
            Servers::Libsql { url_template, token, .. } => {
                let url = url_template.replace("{database}", &Remote::database_name(resource_id));
                Db::open_remote(&url, token)
                    .await
                    .map_err(|error| ApiError::internal("opening a server database", error))?
            }
        };
        self.connections.lock().await.insert(resource_id.to_string(), db.clone());
        Ok(db)
    }

    /// Create the athlete's database on the server.
    async fn provision(&self, resource_id: &str) -> ApiResult<Db> {
        let name = Remote::database_name(resource_id);
        match &self.servers {
            Servers::Local { directory } => {
                let path = directory.join(format!("{name}.sqlite"));
                Db::open_local(&path).map_err(|error| ApiError::internal("creating a server database", error))?;
            }
            Servers::Libsql { provision, .. } => {
                let provision = provision.clone();
                let name = name.clone();
                tokio::task::spawn_blocking(move || provision_database(&provision, &name))
                    .await
                    .map_err(|error| ApiError::internal("creating a server database", error))?
                    .map_err(|error| ApiError::internal("creating a server database", error))?;
            }
        }
        self.server(resource_id).await
    }

    async fn revision_and_epoch(&self, resource_id: &str) -> ApiResult<(u64, u64)> {
        let server = self.server(resource_id).await?;
        let rows = server
            .query(
                "SELECT (SELECT current_revision FROM database_state WHERE singleton_id = 1),
                        (SELECT access_epoch FROM internal_access_state WHERE singleton_id = 1)",
                Vec::new(),
            )
            .await
            .map_err(|error| ApiError::internal("reading the server revision", error))?;
        let row = rows.first().ok_or_else(|| ApiError::internal("reading the server revision", "no row"))?;
        match (row.first().and_then(integer), row.get(1).and_then(integer)) {
            (Some(revision), Some(epoch)) => Ok((revision as u64, epoch as u64)),
            _ => Err(ApiError::internal("reading the server revision", "the database is not initialized")),
        }
    }

    /// Bring the working copy to the server's revision and return that revision.
    async fn current(&self, storage: &Storage, resource_id: &str) -> ApiResult<u64> {
        self.last_used.lock().expect("last-used map").insert(resource_id.to_string(), Instant::now());
        let (revision, _) = self.revision_and_epoch(resource_id).await?;
        let path = storage.database(resource_id);
        if path.exists() {
            let copy_path = path.clone();
            let copy_revision =
                blocking(move || Store::open(&copy_path, OpenOptions::default())?.database_revision()).await;
            match copy_revision {
                Ok(copy) if copy == revision => return Ok(revision),
                Ok(copy) if copy < revision => {
                    if let Some(entries) = self.log_after(resource_id, copy, revision).await? {
                        let copy_path = path.clone();
                        let caught_up = blocking(move || {
                            let mut store = Store::open(&copy_path, OpenOptions::default())?;
                            for (base, _, changeset) in &entries {
                                let epoch = store.access_epoch()?;
                                store.replay(*base, epoch, changeset)?;
                            }
                            store.database_revision()
                        })
                        .await;
                        if caught_up.as_ref().is_ok_and(|reached| *reached == revision) {
                            return Ok(revision);
                        }
                    }
                }
                _ => {}
            }
        }
        self.rebuild(storage, resource_id).await?;
        Ok(revision)
    }

    async fn log_after(
        &self,
        resource_id: &str,
        revision: u64,
        current: u64,
    ) -> ApiResult<Option<Vec<(u64, u64, Changeset)>>> {
        let server = self.server(resource_id).await?;
        let rows = server
            .query(
                "SELECT base_revision, database_revision, changeset FROM internal_replication_log
                 WHERE database_revision > ?1 ORDER BY database_revision",
                vec![SqlValue::Integer(revision as i64)],
            )
            .await
            .map_err(|error| ApiError::internal("reading the replication log", error))?;
        let mut at = revision;
        let mut entries = Vec::new();
        for row in rows {
            let (Some(base), Some(new), Some(json)) =
                (row.first().and_then(integer), row.get(1).and_then(integer), row.get(2).and_then(text))
            else {
                return Ok(None);
            };
            if base as u64 != at {
                return Ok(None);
            }
            entries.push((base as u64, new as u64, Changeset::from_json(json)?));
            at = new as u64;
        }
        Ok((at == current).then_some(entries))
    }

    async fn rebuild(&self, storage: &Storage, resource_id: &str) -> ApiResult<()> {
        let server = self.server(resource_id).await?;
        let mut tables = Vec::new();
        for shape in &self.shapes {
            if NOT_COPIED.contains(&shape.name.as_str()) {
                continue;
            }
            let columns = shape.columns.iter().map(|column| format!("\"{column}\"")).collect::<Vec<_>>().join(", ");
            let order = shape.primary_key.iter().map(|column| format!("\"{column}\"")).collect::<Vec<_>>().join(", ");
            let rows = server
                .query(&format!("SELECT {columns} FROM \"{}\" ORDER BY {order}", shape.name), Vec::new())
                .await
                .map_err(|error| ApiError::internal("copying a server database", error))?;
            tables.push(TableRows { table: shape.name.clone(), columns: shape.columns.clone(), rows });
        }
        let folder = storage.folder(resource_id);
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).map_err(|error| ApiError::internal("creating a working copy", error))?;
        let path = storage.database(resource_id);
        blocking(move || Store::build_working_copy(&path, &tables).map(|_| ())).await
    }

    fn discard(&self, storage: &Storage, resource_id: &str) {
        let _ = std::fs::remove_dir_all(storage.folder(resource_id));
        self.last_used.lock().expect("last-used map").remove(resource_id);
    }

    /// Commit a changeset to the server in one transaction (D268).
    async fn commit(
        &self,
        resource_id: &str,
        base_revision: u64,
        new_revision: u64,
        access_epoch: u64,
        changeset: &Changeset,
        statements: &[ReplayStatement],
    ) -> ApiResult<()> {
        let server = self.server(resource_id).await?;
        let failed = |error: crate::db::DbError| ApiError::internal("committing to the server", error);
        let mut transaction = server.transaction().await.map_err(failed)?;
        let rows = transaction
            .query(
                "SELECT (SELECT current_revision FROM database_state WHERE singleton_id = 1),
                        (SELECT access_epoch FROM internal_access_state WHERE singleton_id = 1)",
                Vec::new(),
            )
            .await
            .map_err(failed)?;
        let row = rows.first().cloned().unwrap_or_default();
        let (revision, epoch) = (row.first().and_then(integer), row.get(1).and_then(integer));
        if revision != Some(base_revision as i64) || epoch != Some(access_epoch as i64) {
            return Err(ApiError::stale("The record changed on the server; the request will run again"));
        }
        transaction
            .execute(
                "INSERT INTO internal_replication_log (database_revision, base_revision, changeset, committed_at) VALUES (?1, ?2, ?3, ?4)",
                vec![
                    SqlValue::Integer(new_revision as i64),
                    SqlValue::Integer(base_revision as i64),
                    SqlValue::Text(changeset.to_json()),
                    SqlValue::Text(crate::directory::now()),
                ],
            )
            .await
            .map_err(failed)?;
        for statement in statements {
            let changed = transaction.execute(&statement.sql, statement.params.clone()).await.map_err(failed)?;
            if !statement.accepts(changed) {
                return Err(ApiError::stale(format!("A change to {} did not apply on the server", statement.table)));
            }
        }
        let ended = first_integer(
            &transaction
                .query("SELECT current_revision FROM database_state WHERE singleton_id = 1", Vec::new())
                .await
                .map_err(failed)?,
        );
        if ended != Some(new_revision as i64) {
            return Err(ApiError::internal("committing to the server", "the changeset ended at another revision"));
        }
        transaction.commit().await.map_err(failed)
    }
}

/// Create one database on the server. Runs on a blocking thread.
fn provision_database(provision: &Provision, name: &str) -> Result<(), String> {
    match provision {
        Provision::Existing => Ok(()),
        Provision::Turso { organization, group, platform_token_env } => {
            let token = Config::secret(platform_token_env)?;
            let response = ureq::post(&format!("https://api.turso.tech/v1/organizations/{organization}/databases"))
                .set("Authorization", &format!("Bearer {token}"))
                .timeout(Duration::from_secs(60))
                .send_json(serde_json::json!({ "name": name, "group": group }));
            match response {
                Ok(_) | Err(ureq::Error::Status(409, _)) => Ok(()),
                Err(error) => Err(format!("Turso did not create the database: {error}")),
            }
        }
        Provision::SqldAdmin { admin_url } => {
            let response = ureq::post(&format!("{}/v1/namespaces/{name}/create", admin_url.trim_end_matches('/')))
                .timeout(Duration::from_secs(60))
                .send_json(serde_json::json!({}));
            match response {
                Ok(_) | Err(ureq::Error::Status(409, _)) => Ok(()),
                Err(error) => Err(format!("the libSQL server did not create the namespace: {error}")),
            }
        }
    }
}

fn file_names(folder: &Path) -> BTreeSet<String> {
    std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_file())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> athleticstandard::Result<T> + Send + 'static,
) -> ApiResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ApiError::internal("a worker thread", error))?
        .map_err(ApiError::from)
}
