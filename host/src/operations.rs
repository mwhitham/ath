//! Every operation of host API version 1, in one table (D266).
//!
//! Each entry names its method, path, caller, bodies, and handler. The router is built
//! from this table, and so is the API description in `schema/host-api/1/`. An
//! operation cannot be served without being described, or described without being
//! served.

use athleticstandard::model::schema::{nonnegative_int, positive_int, text, SchemaRef, Shape};
use athleticstandard::model::RecordType;
use axum::routing::{on, MethodFilter, MethodRouter};

use crate::api::{self, Shared};
use crate::moves;
use crate::shapes::{self, Permission};

/// Who may call an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    /// No token needed.
    Anyone,
    /// Any accepted token.
    SignedIn,
    /// The record's owner, through an app not disconnected from the record (D270).
    Owner,
    /// The owner, or a coach with any active grant through this app.
    OwnerOrCoach,
    /// A coach with an active grant through this app that includes the permission.
    Coach(Permission),
    /// Anyone holding the single-use activation code. The destination host calls it.
    CodeHolder,
}

/// A request or response body.
#[derive(Clone, Copy)]
pub enum Body {
    Empty,
    /// JSON with a named shape. `required` is false when an empty body is allowed.
    Json {
        schema: fn() -> SchemaRef,
        required: bool,
    },
    /// A zip of a complete package folder.
    Zip,
    /// An attachment file: PNG, JPEG, WebP, or PDF.
    File,
}

const fn json(schema: fn() -> SchemaRef) -> Body {
    Body::Json { schema, required: true }
}

/// How a repeated request is answered (D261).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Reads, which change nothing.
    None,
    /// An `Idempotency-Key` is required. The record keeps a receipt.
    Required,
    /// A key is optional. With one, a repeat gets the first answer.
    Optional,
    /// The answer carries a one-time code, which is never stored, so a repeat is not
    /// answered from memory. The operation says what a repeat does.
    NotRemembered,
}

pub struct Query {
    pub name: &'static str,
    pub required: bool,
    pub schema: fn() -> SchemaRef,
    pub description: &'static str,
}

pub struct Operation {
    pub method: &'static str,
    pub path: &'static str,
    pub id: &'static str,
    pub tag: &'static str,
    pub summary: &'static str,
    pub who: Who,
    pub query: &'static [Query],
    pub request: Body,
    pub status: u16,
    pub response: Body,
    pub key: Key,
}

pub struct Route {
    pub operation: Operation,
    pub handler: MethodRouter<Shared>,
}

/// One table entry. The method names both the description and the route, so they
/// cannot differ.
macro_rules! operation {
    ($method:ident $path:literal => $handler:path, $id:literal, $tag:literal, $summary:literal, {
        who: $who:expr, request: $request:expr, status: $status:literal, response: $response:expr, key: $key:expr
        $(, query: $query:expr)? $(,)?
    }) => {
        Route {
            operation: Operation {
                method: stringify!($method),
                path: $path,
                id: $id,
                tag: $tag,
                summary: $summary,
                who: $who,
                query: operation!(@query $($query)?),
                request: $request,
                status: $status,
                response: $response,
                key: $key,
            },
            handler: on(MethodFilter::$method, $handler),
        }
    };
    (@query $query:expr) => { $query };
    (@query) => { &[] };
}

fn record_type_shape() -> SchemaRef {
    RecordType::schema()
}

const SINCE: &[Query] = &[Query {
    name: "since",
    required: true,
    schema: nonnegative_int,
    description: "The database revision the device last verified.",
}];

const RECORD_QUERY: &[Query] = &[
    Query {
        name: "record_type",
        required: false,
        schema: record_type_shape,
        description: "Only records of this type.",
    },
    Query {
        name: "limit",
        required: false,
        schema: positive_int,
        description: "At most this many records. The default and maximum are the host's `page_size`.",
    },
];

const WORKOUT_QUERY: &[Query] = &[
    Query {
        name: "limit",
        required: false,
        schema: positive_int,
        description: "At most this many workouts. The default and maximum are the host's `page_size`.",
    },
    Query {
        name: "after",
        required: false,
        schema: text,
        description: "A `next` value from an earlier page. Treat it as opaque.",
    },
];

/// Every operation, in the order the description lists them.
pub fn routes() -> Vec<Route> {
    use Body::{Empty, File, Zip};
    use Who::*;
    vec![
        // ---- Discovery and account
        operation!(GET "/.well-known/oauth-protected-resource" => api::protected_resource,
            "getProtectedResource", "discovery", "Protected-resource metadata (RFC 9728), naming the sign-in provider.",
            { who: Anyone, request: Empty, status: 200, response: json(shapes::ProtectedResource::schema), key: Key::None }),
        operation!(GET "/v1/capabilities" => api::capabilities,
            "getCapabilities", "discovery", "The API version, record format version, storage, and enrollment.",
            { who: Anyone, request: Empty, status: 200, response: json(shapes::Capabilities::schema), key: Key::None }),
        operation!(GET "/v1/me" => api::me,
            "getMe", "discovery", "The signed-in person, the app, and the records they own that this app may see.",
            { who: SignedIn, request: Empty, status: 200, response: json(shapes::Me::schema), key: Key::None }),
        // ---- Records the caller owns
        operation!(POST "/v1/athletes" => api::create_athlete,
            "createAthlete", "athletes", "Create an empty record. Without open enrollment, an unused invitation is required. With `incoming`, the record is staged for a move.",
            { who: SignedIn, request: Body::Json { schema: shapes::NewAthleteRequest::schema, required: false }, status: 201, response: json(shapes::CreatedAthlete::schema), key: Key::Optional }),
        operation!(GET "/v1/athletes" => api::list_athletes,
            "listAthletes", "athletes", "The caller's records, except retired ones and ones this app was disconnected from.",
            { who: SignedIn, request: Empty, status: 200, response: json(shapes::AthleteList::schema), key: Key::None }),
        operation!(GET "/v1/athletes/{id}" => api::athlete_summary,
            "getAthlete", "athletes", "The caller's role and the record's state. The owner also gets the database revision.",
            { who: OwnerOrCoach, request: Empty, status: 200, response: json(shapes::AthleteSummary::schema), key: Key::None }),
        operation!(PUT "/v1/athletes/{id}/package" => api::put_package,
            "restorePackage", "records", "Restore a complete package into an empty record: the first transfer of an existing record.",
            { who: Owner, request: Zip, status: 200, response: json(shapes::DatabaseRevision::schema), key: Key::Optional }),
        operation!(GET "/v1/athletes/{id}/export" => api::export,
            "exportPackage", "records", "The complete package as a zip. `x-manifest-digest` is the SHA-256 of its manifest.",
            { who: Owner, request: Empty, status: 200, response: Zip, key: Key::None }),
        operation!(GET "/v1/athletes/{id}/changes" => api::changes,
            "listChanges", "records", "The record changes since a database revision, in order (D263). When `complete` is false, download the export.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::ChangeList::schema), key: Key::None, query: SINCE }),
        operation!(GET "/v1/athletes/{id}/records" => api::list_records,
            "listRecords", "records", "Current records, oldest first.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::RecordList::schema), key: Key::None, query: RECORD_QUERY }),
        operation!(POST "/v1/athletes/{id}/records" => api::create_record,
            "createRecord", "records", "Create a record, with any uploaded attachments. New attachment records come first in the result.",
            { who: Owner, request: json(shapes::CreateRecordRequest::schema), status: 201, response: json(shapes::WriteResult::schema), key: Key::Required }),
        operation!(GET "/v1/athletes/{id}/records/{record_id}" => api::get_record,
            "getRecord", "records", "One record and its complete history.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::RecordWithHistory::schema), key: Key::None }),
        operation!(POST "/v1/athletes/{id}/records/{record_id}/corrections" => api::correct_record,
            "correctRecord", "records", "Correct a record. An out-of-date `expected_revision` gets `409 stale`.",
            { who: Owner, request: json(shapes::CorrectionRequest::schema), status: 200, response: json(shapes::WriteResult::schema), key: Key::Required }),
        operation!(POST "/v1/athletes/{id}/records/{record_id}/deletion" => api::delete_record,
            "deleteRecord", "records", "Delete a record. Its history stays.",
            { who: Owner, request: json(shapes::DeletionRequest::schema), status: 200, response: json(shapes::WriteResult::schema), key: Key::Required }),
        operation!(POST "/v1/athletes/{id}/records/{record_id}/erasure" => api::erase,
            "eraseRecord", "records", "Erase a record and its history for privacy (D107). Without a key, a repeat fails because the record is gone.",
            { who: Owner, request: json(shapes::ErasureRequest::schema), status: 200, response: json(shapes::DatabaseRevision::schema), key: Key::Optional }),
        operation!(POST "/v1/athletes/{id}/uploads" => api::upload,
            "uploadAttachment", "records", "Upload an attachment file. Name its `upload_id` when creating the record, within one hour.",
            { who: Owner, request: File, status: 201, response: json(shapes::Upload::schema), key: Key::Optional }),
        operation!(GET "/v1/athletes/{id}/attachments/{record_id}" => api::download_attachment,
            "downloadAttachment", "records", "An attachment's file, with its media type.",
            { who: Owner, request: Empty, status: 200, response: File, key: Key::None }),
        // ---- Grants (D262)
        operation!(POST "/v1/athletes/{id}/grant-offers" => api::create_grant_offer,
            "createGrantOffer", "grants", "Offer a coach access through one app. The code is shown once. A repeat makes a second offer.",
            { who: Owner, request: json(shapes::GrantOfferRequest::schema), status: 201, response: json(shapes::GrantOffer::schema), key: Key::NotRemembered }),
        operation!(GET "/v1/athletes/{id}/grant-offers/pending" => api::pending_offers,
            "listPendingOffers", "grants", "Offers nobody has accepted that have not expired. Codes are never shown again.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::PendingOffers::schema), key: Key::None }),
        operation!(DELETE "/v1/athletes/{id}/grant-offers/{offer_id}" => api::cancel_offer,
            "cancelGrantOffer", "grants", "Cancel a pending offer.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::CancelledOffer::schema), key: Key::Optional }),
        operation!(GET "/v1/athletes/{id}/grants" => api::list_grants,
            "listGrants", "grants", "Every grant for the record, revoked ones included.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::GrantList::schema), key: Key::None }),
        operation!(DELETE "/v1/athletes/{id}/grants/{grant_id}" => api::revoke_grant,
            "revokeGrant", "grants", "Revoke a grant. The coach's next request fails, and a coach write already waiting fails too.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::RevokedGrant::schema), key: Key::Optional }),
        operation!(POST "/v1/grant-offers/accept" => api::accept_grant_offer,
            "acceptGrantOffer", "grants", "Accept an offer, through the app it names.",
            { who: SignedIn, request: json(shapes::AcceptOfferRequest::schema), status: 200, response: json(shapes::AcceptedGrant::schema), key: Key::Optional }),
        // ---- Apps (D270)
        operation!(GET "/v1/athletes/{id}/apps" => api::list_apps,
            "listApps", "apps", "The apps the owner has used with this record.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::AppList::schema), key: Key::None }),
        operation!(DELETE "/v1/athletes/{id}/apps/{client_id}" => api::disconnect_app,
            "disconnectApp", "apps", "Disconnect an app from this record. Its next request gets `403 app_disconnected`. The last connected app cannot be disconnected.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::DisconnectedApp::schema), key: Key::Optional }),
        operation!(PUT "/v1/athletes/{id}/apps/{client_id}" => api::connect_app,
            "connectApp", "apps", "Connect a disconnected app again, from an app that is still connected.",
            { who: Owner, request: Empty, status: 200, response: json(shapes::ConnectedApp::schema), key: Key::Optional }),
        // ---- Coaches
        operation!(GET "/v1/roster" => api::roster,
            "getRoster", "coaching", "Each record that granted the caller access through this app.",
            { who: SignedIn, request: Empty, status: 200, response: json(shapes::Roster::schema), key: Key::None }),
        operation!(GET "/v1/athletes/{id}/workouts" => api::workout_summaries,
            "listWorkouts", "coaching", "Workout summaries, newest start first: exactly the fields the read grant allows.",
            { who: Coach(Permission::ReadWorkoutSummaries), request: Empty, status: 200, response: json(shapes::WorkoutPage::schema), key: Key::None, query: WORKOUT_QUERY }),
        operation!(POST "/v1/athletes/{id}/notes" => api::add_note,
            "addCoachNote", "coaching", "Add a coach note. The host writes the coach author. Evidence needs the read grant too.",
            { who: Coach(Permission::AddCoachNotes), request: json(shapes::NoteRequest::schema), status: 201, response: json(shapes::WriteResult::schema), key: Key::Required }),
        operation!(POST "/v1/athletes/{id}/programs" => api::add_program,
            "addProgram", "coaching", "Add a training plan and its planned sessions. The result lists the plan, then its sessions.",
            { who: Coach(Permission::AddProgramming), request: json(shapes::ProgramRequest::schema), status: 201, response: json(shapes::WriteResult::schema), key: Key::Required }),
        // ---- Moves (D267)
        operation!(POST "/v1/athletes/{id}/move/freeze" => moves::freeze,
            "freezeForMove", "moves", "On the source: stop writes and fix the final package. A repeat returns the same transfer.",
            { who: Owner, request: json(shapes::FreezeRequest::schema), status: 200, response: json(shapes::FrozenRecord::schema), key: Key::NotRemembered }),
        operation!(POST "/v1/athletes/{id}/move/abort" => moves::abort,
            "abortMove", "moves", "On the source, before retiring: reopen the record for writes.",
            { who: Owner, request: json(shapes::TransferRequest::schema), status: 200, response: json(shapes::ReopenedRecord::schema), key: Key::NotRemembered }),
        operation!(POST "/v1/athletes/{id}/move/retire" => moves::retire,
            "retireForMove", "moves", "On the source: retire the writer and issue a single-use activation code. A repeat issues a new code; only the newest works.",
            { who: Owner, request: json(shapes::TransferRequest::schema), status: 200, response: json(shapes::RetiredRecord::schema), key: Key::NotRemembered }),
        operation!(POST "/v1/athletes/{id}/move/activate" => moves::activate,
            "activateMove", "moves", "On the destination: ask the source to confirm the code, then accept writes.",
            { who: Owner, request: json(shapes::ActivateRequest::schema), status: 200, response: json(shapes::ActivatedRecord::schema), key: Key::NotRemembered }),
        operation!(GET "/v1/athletes/{id}/move/recovery-copy" => moves::recovery_copy,
            "getRecoveryCopy", "moves", "On the source, after retiring: the retired record's package as a zip.",
            { who: Owner, request: Empty, status: 200, response: Zip, key: Key::None }),
        operation!(POST "/v1/transfers/{transfer_id}/verify" => moves::verify,
            "verifyTransfer", "moves", "Called by the destination host. The code works once, for the destination and digest it names.",
            { who: CodeHolder, request: json(shapes::VerifyRequest::schema), status: 200, response: json(shapes::VerifiedTransfer::schema), key: Key::NotRemembered }),
    ]
}
