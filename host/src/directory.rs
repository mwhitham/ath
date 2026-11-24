//! The host's directory: signed-in people, the athlete records they own, grant offers,
//! grants, and invitations (D256, D260, D262). It is a separate database from every
//! athlete's record. Grants are history-free host state; the record keeps only
//! authorship (D255).

use athleticstandard::store::replica::SqlValue;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::{first_integer, text, Db, DbError, DbResult, Row};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS host_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS principals (
  principal_id TEXT PRIMARY KEY,
  issuer TEXT NOT NULL,
  subject TEXT NOT NULL,
  display_name TEXT,
  coach_id TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  UNIQUE (issuer, subject)
) STRICT;
CREATE TABLE IF NOT EXISTS athletes (
  resource_id TEXT PRIMARY KEY,
  owner_principal_id TEXT NOT NULL REFERENCES principals (principal_id),
  display_name TEXT,
  state TEXT NOT NULL CHECK (state IN ('active', 'frozen', 'retired', 'staged')),
  created_at TEXT NOT NULL
) STRICT;
CREATE INDEX IF NOT EXISTS athletes_owner_idx ON athletes (owner_principal_id);
CREATE TABLE IF NOT EXISTS invitations (
  code_hash TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('athlete', 'grant')),
  resource_id TEXT REFERENCES athletes (resource_id),
  permissions TEXT,
  client_id TEXT,
  coach_label TEXT,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_by TEXT,
  used_at TEXT
) STRICT;
CREATE TABLE IF NOT EXISTS grants (
  grant_id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL REFERENCES athletes (resource_id),
  coach_principal_id TEXT NOT NULL REFERENCES principals (principal_id),
  client_id TEXT NOT NULL,
  permissions TEXT NOT NULL,
  coach_label TEXT,
  created_at TEXT NOT NULL,
  revoked_at TEXT
) STRICT;
CREATE INDEX IF NOT EXISTS grants_coach_idx ON grants (coach_principal_id, client_id);
CREATE INDEX IF NOT EXISTS grants_resource_idx ON grants (resource_id);
CREATE TABLE IF NOT EXISTS host_requests (
  principal_id TEXT NOT NULL,
  client_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  operation TEXT NOT NULL,
  request_digest TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (principal_id, client_id, request_id)
) STRICT;
CREATE TABLE IF NOT EXISTS erasure_intents (
  intent_id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL,
  record_id TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  requested_at TEXT NOT NULL,
  completed_at TEXT
) STRICT;
CREATE INDEX IF NOT EXISTS erasure_intents_resource_idx ON erasure_intents (resource_id);
CREATE TABLE IF NOT EXISTS transfers (
  transfer_id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL,
  direction TEXT NOT NULL CHECK (direction IN ('outgoing', 'incoming')),
  destination TEXT,
  manifest_digest TEXT,
  activation_hash TEXT,
  state TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
) STRICT;
";

/// The three grants (D262).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ReadWorkoutSummaries,
    AddCoachNotes,
    AddProgramming,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Principal {
    pub principal_id: String,
    pub issuer: String,
    pub subject: String,
    pub display_name: Option<String>,
    /// The opaque ID written into coach authorship (D255).
    pub coach_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Athlete {
    pub resource_id: String,
    pub owner_principal_id: String,
    pub display_name: Option<String>,
    pub state: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Grant {
    pub grant_id: String,
    pub resource_id: String,
    pub coach_principal_id: String,
    pub client_id: String,
    pub permissions: Vec<Permission>,
    pub coach_label: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

impl Grant {
    pub fn allows(&self, permission: Permission) -> bool {
        self.revoked_at.is_none() && self.permissions.contains(&permission)
    }
}

/// An offer an athlete made: a code a coach accepts with a named app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Offer {
    pub code: String,
    pub expires_at: String,
}

#[derive(Clone)]
pub struct Directory {
    db: Db,
}

/// True for an athlete ID this host issues: `ath_` and 32 lowercase hexadecimal
/// digits. Every request's ID is checked before it reaches storage, where it becomes
/// part of a file path or a database address.
pub fn valid_resource_id(id: &str) -> bool {
    id.len() == 36
        && id.starts_with("ath_")
        && id[4..].bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn later(seconds: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::seconds(seconds)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn hash_code(code: &str) -> String {
    hex(&Sha256::digest(code.as_bytes()))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A code with 244 random bits, safe to show once and store only as a hash.
pub fn new_code() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

fn t(value: &str) -> SqlValue {
    SqlValue::Text(value.to_string())
}

fn opt(value: Option<&str>) -> SqlValue {
    value.map(t).unwrap_or(SqlValue::Null)
}

fn get_text(row: &Row, index: usize) -> String {
    row.get(index).and_then(text).unwrap_or_default().to_string()
}

fn get_opt(row: &Row, index: usize) -> Option<String> {
    row.get(index).and_then(text).map(str::to_string)
}

fn permissions_text(permissions: &[Permission]) -> String {
    serde_json::to_string(permissions).expect("permissions serialize")
}

fn parse_permissions(text: &str) -> Vec<Permission> {
    serde_json::from_str(text).unwrap_or_default()
}

fn athlete(row: &Row) -> Athlete {
    Athlete {
        resource_id: get_text(row, 0),
        owner_principal_id: get_text(row, 1),
        display_name: get_opt(row, 2),
        state: get_text(row, 3),
        created_at: get_text(row, 4),
    }
}

const ATHLETE_COLUMNS: &str = "resource_id, owner_principal_id, display_name, state, created_at";
const GRANT_COLUMNS: &str =
    "grant_id, resource_id, coach_principal_id, client_id, permissions, coach_label, created_at, revoked_at";

fn grant(row: &Row) -> Grant {
    Grant {
        grant_id: get_text(row, 0),
        resource_id: get_text(row, 1),
        coach_principal_id: get_text(row, 2),
        client_id: get_text(row, 3),
        permissions: parse_permissions(&get_text(row, 4)),
        coach_label: get_opt(row, 5),
        created_at: get_text(row, 6),
        revoked_at: get_opt(row, 7),
    }
}

impl Directory {
    pub async fn open(db: Db) -> DbResult<Directory> {
        db.execute_batch(SCHEMA).await?;
        Ok(Directory { db })
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    /// The principal for a verified issuer and subject, created on first sight. The ID
    /// is derived from the issuer and subject, never from an email address (D260).
    pub async fn principal(&self, issuer: &str, subject: &str, display_name: Option<&str>) -> DbResult<Principal> {
        let principal_id = format!("p_{}", &hex(&Sha256::digest(format!("{issuer}\n{subject}").as_bytes()))[..32]);
        self.db
            .execute(
                "INSERT INTO principals (principal_id, issuer, subject, display_name, coach_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (principal_id) DO UPDATE SET display_name = coalesce(excluded.display_name, principals.display_name)",
                vec![t(&principal_id), t(issuer), t(subject), opt(display_name), t(&uuid::Uuid::new_v4().to_string()), t(&now())],
            )
            .await?;
        let rows = self
            .db
            .query(
                "SELECT principal_id, issuer, subject, display_name, coach_id FROM principals WHERE principal_id = ?1",
                vec![t(&principal_id)],
            )
            .await?;
        let row = rows.first().ok_or_else(|| DbError("principal was not stored".into()))?;
        Ok(Principal {
            principal_id: get_text(row, 0),
            issuer: get_text(row, 1),
            subject: get_text(row, 2),
            display_name: get_opt(row, 3),
            coach_id: get_text(row, 4),
        })
    }

    pub async fn create_athlete(&self, owner_principal_id: &str, display_name: Option<&str>) -> DbResult<Athlete> {
        self.create_athlete_in(owner_principal_id, display_name, "active").await
    }

    /// A new athlete record in `state`: `active`, or `staged` for a record arriving
    /// from another host, which accepts only its package until it is activated (D267).
    pub async fn create_athlete_in(
        &self,
        owner_principal_id: &str,
        display_name: Option<&str>,
        state: &str,
    ) -> DbResult<Athlete> {
        let resource_id = format!("ath_{}", uuid::Uuid::new_v4().simple());
        self.db
            .execute(
                &format!("INSERT INTO athletes ({ATHLETE_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5)"),
                vec![t(&resource_id), t(owner_principal_id), opt(display_name), t(state), t(&now())],
            )
            .await?;
        self.athlete(&resource_id).await?.ok_or_else(|| DbError("athlete was not stored".into()))
    }

    pub async fn athlete(&self, resource_id: &str) -> DbResult<Option<Athlete>> {
        if !valid_resource_id(resource_id) {
            return Ok(None);
        }
        let rows = self
            .db
            .query(&format!("SELECT {ATHLETE_COLUMNS} FROM athletes WHERE resource_id = ?1"), vec![t(resource_id)])
            .await?;
        Ok(rows.first().map(athlete))
    }

    pub async fn athletes_owned_by(&self, principal_id: &str) -> DbResult<Vec<Athlete>> {
        let rows = self
            .db
            .query(
                &format!("SELECT {ATHLETE_COLUMNS} FROM athletes WHERE owner_principal_id = ?1 AND state <> 'retired' ORDER BY created_at, resource_id"),
                vec![t(principal_id)],
            )
            .await?;
        Ok(rows.iter().map(athlete).collect())
    }

    pub async fn all_athletes(&self) -> DbResult<Vec<Athlete>> {
        let rows = self
            .db
            .query(
                &format!("SELECT {ATHLETE_COLUMNS} FROM athletes WHERE state <> 'retired' ORDER BY resource_id"),
                Vec::new(),
            )
            .await?;
        Ok(rows.iter().map(athlete).collect())
    }

    pub async fn set_athlete_state(&self, resource_id: &str, from: &str, to: &str) -> DbResult<bool> {
        let changed = self
            .db
            .execute(
                "UPDATE athletes SET state = ?3 WHERE resource_id = ?1 AND state = ?2",
                vec![t(resource_id), t(from), t(to)],
            )
            .await?;
        Ok(changed == 1)
    }

    /// An invitation that lets one person create their own record, used once.
    pub async fn create_athlete_invitation(&self, created_by: &str, valid_seconds: i64) -> DbResult<Offer> {
        let code = new_code();
        let expires_at = later(valid_seconds);
        self.db
            .execute(
                "INSERT INTO invitations (code_hash, kind, created_by, created_at, expires_at) VALUES (?1, 'athlete', ?2, ?3, ?4)",
                vec![t(&hash_code(&code)), t(created_by), t(&now()), t(&expires_at)],
            )
            .await?;
        Ok(Offer { code, expires_at })
    }

    /// Use an athlete invitation. False when it is unknown, used, or expired.
    pub async fn use_athlete_invitation(&self, code: &str, principal_id: &str) -> DbResult<bool> {
        let changed = self
            .db
            .execute(
                "UPDATE invitations SET used_by = ?2, used_at = ?3
                 WHERE code_hash = ?1 AND kind = 'athlete' AND used_at IS NULL AND expires_at > ?3",
                vec![t(&hash_code(code)), t(principal_id), t(&now())],
            )
            .await?;
        Ok(changed == 1)
    }

    /// A grant offer for one athlete's record: the permissions, the app it must be
    /// accepted through, and a label the athlete gave the coach.
    pub async fn create_grant_offer(
        &self,
        resource_id: &str,
        permissions: &[Permission],
        client_id: &str,
        coach_label: Option<&str>,
        created_by: &str,
        valid_seconds: i64,
    ) -> DbResult<Offer> {
        let code = new_code();
        let expires_at = later(valid_seconds);
        self.db
            .execute(
                "INSERT INTO invitations (code_hash, kind, resource_id, permissions, client_id, coach_label, created_by, created_at, expires_at)
                 VALUES (?1, 'grant', ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                vec![
                    t(&hash_code(&code)),
                    t(resource_id),
                    t(&permissions_text(permissions)),
                    t(client_id),
                    opt(coach_label),
                    t(created_by),
                    t(&now()),
                    t(&expires_at),
                ],
            )
            .await?;
        Ok(Offer { code, expires_at })
    }

    /// Accept a grant offer as `coach` through `client_id`. `None` when the offer is
    /// unknown, used, expired, made for another app, or made by the coach themselves.
    pub async fn accept_grant_offer(&self, code: &str, coach: &Principal, client_id: &str) -> DbResult<Option<Grant>> {
        let mut transaction = self.db.transaction().await?;
        let rows = transaction
            .query(
                "SELECT resource_id, permissions, client_id, coach_label, created_by FROM invitations
                 WHERE code_hash = ?1 AND kind = 'grant' AND used_at IS NULL AND expires_at > ?2",
                vec![t(&hash_code(code)), t(&now())],
            )
            .await?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        if get_text(row, 2) != client_id || get_text(row, 4) == coach.principal_id {
            return Ok(None);
        }
        let grant_id = format!("grant_{}", uuid::Uuid::new_v4().simple());
        transaction
            .execute(
                "UPDATE invitations SET used_by = ?2, used_at = ?3 WHERE code_hash = ?1",
                vec![t(&hash_code(code)), t(&coach.principal_id), t(&now())],
            )
            .await?;
        transaction
            .execute(
                &format!("INSERT INTO grants ({GRANT_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)"),
                vec![
                    t(&grant_id),
                    t(&get_text(row, 0)),
                    t(&coach.principal_id),
                    t(client_id),
                    t(&get_text(row, 1)),
                    opt(get_opt(row, 3).as_deref()),
                    t(&now()),
                ],
            )
            .await?;
        transaction.commit().await?;
        self.grant(&grant_id).await
    }

    pub async fn grant(&self, grant_id: &str) -> DbResult<Option<Grant>> {
        let rows = self
            .db
            .query(&format!("SELECT {GRANT_COLUMNS} FROM grants WHERE grant_id = ?1"), vec![t(grant_id)])
            .await?;
        Ok(rows.first().map(grant))
    }

    pub async fn grants_for_athlete(&self, resource_id: &str) -> DbResult<Vec<Grant>> {
        let rows = self
            .db
            .query(
                &format!("SELECT {GRANT_COLUMNS} FROM grants WHERE resource_id = ?1 ORDER BY created_at, grant_id"),
                vec![t(resource_id)],
            )
            .await?;
        Ok(rows.iter().map(grant).collect())
    }

    /// The coach's current grant for this athlete through this app.
    pub async fn active_grant(
        &self,
        resource_id: &str,
        coach_principal_id: &str,
        client_id: &str,
    ) -> DbResult<Option<Grant>> {
        let rows = self
            .db
            .query(
                &format!(
                    "SELECT {GRANT_COLUMNS} FROM grants
                     WHERE resource_id = ?1 AND coach_principal_id = ?2 AND client_id = ?3 AND revoked_at IS NULL
                     ORDER BY created_at DESC LIMIT 1"
                ),
                vec![t(resource_id), t(coach_principal_id), t(client_id)],
            )
            .await?;
        Ok(rows.first().map(grant))
    }

    /// Athletes who granted this coach access through this app, with the grant.
    pub async fn roster(&self, coach_principal_id: &str, client_id: &str) -> DbResult<Vec<(Athlete, Grant)>> {
        let rows = self
            .db
            .query(
                &format!(
                    "SELECT {}, {} FROM grants g JOIN athletes a ON a.resource_id = g.resource_id
                     WHERE g.coach_principal_id = ?1 AND g.client_id = ?2 AND g.revoked_at IS NULL AND a.state <> 'retired'
                     ORDER BY a.resource_id",
                    ATHLETE_COLUMNS.split(", ").map(|c| format!("a.{c}")).collect::<Vec<_>>().join(", "),
                    GRANT_COLUMNS.split(", ").map(|c| format!("g.{c}")).collect::<Vec<_>>().join(", "),
                ),
                vec![t(coach_principal_id), t(client_id)],
            )
            .await?;
        Ok(rows.iter().map(|row| (athlete(&row[..5].to_vec()), grant(&row[5..].to_vec()))).collect())
    }

    /// Mark a grant revoked. The caller raises the access epoch afterwards (D262).
    pub async fn revoke(&self, resource_id: &str, grant_id: &str) -> DbResult<bool> {
        let changed = self
            .db
            .execute(
                "UPDATE grants SET revoked_at = ?3 WHERE resource_id = ?1 AND grant_id = ?2 AND revoked_at IS NULL",
                vec![t(resource_id), t(grant_id), t(&now())],
            )
            .await?;
        Ok(changed == 1)
    }

    pub async fn setting(&self, key: &str) -> DbResult<Option<String>> {
        let rows = self.db.query("SELECT value FROM host_settings WHERE key = ?1", vec![t(key)]).await?;
        Ok(rows.first().and_then(|row| get_opt(row, 0)))
    }

    /// Store a setting once. False when it was already set.
    pub async fn set_setting_once(&self, key: &str, value: &str) -> DbResult<bool> {
        let changed = self
            .db
            .execute("INSERT OR IGNORE INTO host_settings (key, value) VALUES (?1, ?2)", vec![t(key), t(value)])
            .await?;
        Ok(changed == 1)
    }

    pub async fn count(&self, table: &str) -> DbResult<i64> {
        let rows = self.db.query(&format!("SELECT count(*) FROM {table}"), Vec::new()).await?;
        Ok(first_integer(&rows).unwrap_or(0))
    }

    // ---- Moves between hosts (D267) ------------------------------------------

    pub async fn record_transfer(
        &self,
        transfer_id: &str,
        resource_id: &str,
        direction: &str,
        destination: Option<&str>,
        state: &str,
    ) -> DbResult<()> {
        self.db
            .execute(
                "INSERT INTO transfers (transfer_id, resource_id, direction, destination, state, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                vec![t(transfer_id), t(resource_id), t(direction), opt(destination), t(state), t(&now())],
            )
            .await?;
        Ok(())
    }

    pub async fn transfer(&self, transfer_id: &str) -> DbResult<Option<Transfer>> {
        let rows = self
            .db
            .query(
                "SELECT transfer_id, resource_id, direction, destination, manifest_digest, activation_hash, state FROM transfers WHERE transfer_id = ?1",
                vec![t(transfer_id)],
            )
            .await?;
        Ok(rows.first().map(|row| Transfer {
            transfer_id: get_text(row, 0),
            resource_id: get_text(row, 1),
            direction: get_text(row, 2),
            destination: get_opt(row, 3),
            manifest_digest: get_opt(row, 4),
            activation_hash: get_opt(row, 5),
            state: get_text(row, 6),
        }))
    }

    pub async fn update_transfer(
        &self,
        transfer_id: &str,
        from_state: &str,
        to_state: &str,
        manifest_digest: Option<&str>,
        activation_hash: Option<&str>,
    ) -> DbResult<bool> {
        let changed = self
            .db
            .execute(
                "UPDATE transfers SET state = ?3, manifest_digest = coalesce(?4, manifest_digest),
                   activation_hash = coalesce(?5, activation_hash), updated_at = ?6
                 WHERE transfer_id = ?1 AND state = ?2",
                vec![t(transfer_id), t(from_state), t(to_state), opt(manifest_digest), opt(activation_hash), t(&now())],
            )
            .await?;
        Ok(changed == 1)
    }

    pub async fn open_transfer_for(&self, resource_id: &str) -> DbResult<Option<Transfer>> {
        let rows = self
            .db
            .query(
                "SELECT transfer_id FROM transfers WHERE resource_id = ?1 AND state NOT IN ('aborted', 'activated', 'retired') ORDER BY created_at DESC LIMIT 1",
                vec![t(resource_id)],
            )
            .await?;
        match rows.first().and_then(|row| get_opt(row, 0)) {
            Some(id) => self.transfer(&id).await,
            None => Ok(None),
        }
    }
}

impl Directory {
    /// Record the single-use activation for a retired transfer, stored as a hash.
    pub async fn set_activation(&self, transfer_id: &str, activation: &str) -> DbResult<bool> {
        let changed = self
            .db
            .execute(
                "UPDATE transfers SET activation_hash = ?2, updated_at = ?3 WHERE transfer_id = ?1 AND state = 'retired'",
                vec![t(transfer_id), t(&hash_code(activation)), t(&now())],
            )
            .await?;
        Ok(changed == 1)
    }

    /// Use an activation once. True only when the code, the destination, and the
    /// package digest all match a retired transfer that was not activated before.
    pub async fn use_activation(
        &self,
        transfer_id: &str,
        activation: &str,
        destination: &str,
        manifest_digest: &str,
    ) -> DbResult<bool> {
        let changed = self
            .db
            .execute(
                "UPDATE transfers SET state = 'activated', activation_hash = NULL, updated_at = ?5
                 WHERE transfer_id = ?1 AND state = 'retired' AND activation_hash = ?2
                   AND destination = ?3 AND manifest_digest = ?4",
                vec![t(transfer_id), t(&hash_code(activation)), t(destination), t(manifest_digest), t(&now())],
            )
            .await?;
        Ok(changed == 1)
    }

    /// Note an erasure before it is applied, so it is applied again after a failure
    /// or a restore from an older backup (D264).
    pub async fn record_erasure_intent(
        &self,
        resource_id: &str,
        record_id: &str,
        reason_code: &str,
    ) -> DbResult<String> {
        let intent_id = format!("erase_{}", uuid::Uuid::new_v4().simple());
        self.db
            .execute(
                "INSERT INTO erasure_intents (intent_id, resource_id, record_id, reason_code, requested_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                vec![t(&intent_id), t(resource_id), t(record_id), t(reason_code), t(&now())],
            )
            .await?;
        Ok(intent_id)
    }

    pub async fn complete_erasure_intent(&self, intent_id: &str) -> DbResult<()> {
        self.db
            .execute(
                "UPDATE erasure_intents SET completed_at = ?2 WHERE intent_id = ?1 AND completed_at IS NULL",
                vec![t(intent_id), t(&now())],
            )
            .await?;
        Ok(())
    }

    /// Every erasure ever requested for one athlete, oldest first: (intent, record, reason).
    pub async fn erasure_intents(&self, resource_id: &str) -> DbResult<Vec<(String, String, String)>> {
        let rows = self
            .db
            .query(
                "SELECT intent_id, record_id, reason_code FROM erasure_intents WHERE resource_id = ?1 ORDER BY requested_at, intent_id",
                vec![t(resource_id)],
            )
            .await?;
        Ok(rows.iter().map(|row| (get_text(row, 0), get_text(row, 1), get_text(row, 2))).collect())
    }

    /// Erasures that were requested and never completed: (intent, athlete, record, reason).
    pub async fn unfinished_erasure_intents(&self) -> DbResult<Vec<(String, String, String, String)>> {
        let rows = self
            .db
            .query(
                "SELECT intent_id, resource_id, record_id, reason_code FROM erasure_intents WHERE completed_at IS NULL ORDER BY requested_at",
                Vec::new(),
            )
            .await?;
        Ok(rows.iter().map(|row| (get_text(row, 0), get_text(row, 1), get_text(row, 2), get_text(row, 3))).collect())
    }
}

impl Directory {
    /// A request the host answered before, for operations that change the directory
    /// rather than a record: (operation, digest, result).
    pub async fn find_request(
        &self,
        principal_id: &str,
        client_id: &str,
        request_id: &str,
    ) -> DbResult<Option<(String, String, String)>> {
        let rows = self
            .db
            .query(
                "SELECT operation, request_digest, result_json FROM host_requests WHERE principal_id = ?1 AND client_id = ?2 AND request_id = ?3",
                vec![t(principal_id), t(client_id), t(request_id)],
            )
            .await?;
        Ok(rows.first().map(|row| (get_text(row, 0), get_text(row, 1), get_text(row, 2))))
    }

    pub async fn store_request(
        &self,
        principal_id: &str,
        client_id: &str,
        request_id: &str,
        operation: &str,
        request_digest: &str,
        result_json: &str,
    ) -> DbResult<()> {
        self.db
            .execute(
                "INSERT OR IGNORE INTO host_requests (principal_id, client_id, request_id, operation, request_digest, result_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                vec![t(principal_id), t(client_id), t(request_id), t(operation), t(request_digest), t(result_json), t(&now())],
            )
            .await?;
        Ok(())
    }

    /// Grant offers for one athlete that nobody accepted yet. The offer ID is the hash
    /// of its code; the code itself is shown only once.
    pub async fn pending_offers(&self, resource_id: &str) -> DbResult<Vec<serde_json::Value>> {
        let rows = self
            .db
            .query(
                "SELECT code_hash, permissions, client_id, coach_label, created_at, expires_at FROM invitations
                 WHERE kind = 'grant' AND resource_id = ?1 AND used_at IS NULL AND expires_at > ?2 ORDER BY created_at",
                vec![t(resource_id), t(&now())],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "offer_id": get_text(row, 0),
                    "permissions": parse_permissions(&get_text(row, 1)),
                    "client_id": get_text(row, 2),
                    "coach_label": get_opt(row, 3),
                    "created_at": get_text(row, 4),
                    "expires_at": get_text(row, 5),
                })
            })
            .collect())
    }

    /// Cancel a pending offer. False when it is unknown, used, or for another athlete.
    pub async fn cancel_offer(&self, resource_id: &str, offer_id: &str) -> DbResult<bool> {
        let changed = self
            .db
            .execute(
                "UPDATE invitations SET used_by = 'cancelled', used_at = ?3
                 WHERE code_hash = ?2 AND resource_id = ?1 AND kind = 'grant' AND used_at IS NULL",
                vec![t(resource_id), t(offer_id), t(&now())],
            )
            .await?;
        Ok(changed == 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Transfer {
    pub transfer_id: String,
    pub resource_id: String,
    pub direction: String,
    pub destination: Option<String>,
    pub manifest_digest: Option<String>,
    pub activation_hash: Option<String>,
    pub state: String,
}
