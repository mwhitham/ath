//! The API description of host API version 1, generated from the operation table and
//! the body shapes (D266). `host/tests/generated.rs` compares it with the committed
//! copy, and `ATH_WRITE_GENERATED=1 cargo test --test generated` rewrites it.

use athleticstandard::model::json_schema::Components;
use athleticstandard::model::record_id;
use athleticstandard::model::schema::SchemaRef;
use serde_json::{json, Map, Value};

use crate::operations::{routes, Body, Key, Operation, Who};
use crate::shapes;

/// Where the description is committed, relative to the repository.
pub const GENERATED_PATH: &str = "schema/host-api/1/openapi.json";

const PREFIX: &str = "#/components/schemas/";

const MEDIA_FILES: &[&str] = &["image/png", "image/jpeg", "image/webp", "application/pdf"];

/// The description as JSON text, ending with a newline.
pub fn generate() -> String {
    let mut text = athleticstandard::json::to_pretty(&description());
    text.push('\n');
    text
}

/// The description as a JSON value (OpenAPI 3.1).
pub fn description() -> Value {
    let named = shapes::named();
    let external = shapes::external();
    let mut components = Components::new(PREFIX);
    for (name, schema) in &named {
        components.name(schema, name);
    }
    for (name, schema, _) in &external {
        components.name(schema, name);
    }

    let mut paths = Map::new();
    for route in routes() {
        let operation = &route.operation;
        let entry = paths.entry(operation.path.to_string()).or_insert_with(|| Value::Object(Map::new()));
        entry
            .as_object_mut()
            .expect("a path item")
            .insert(operation.method.to_ascii_lowercase(), operation_object(operation, &named, &mut components));
    }

    let mut schemas = Map::new();
    for (name, schema) in &named {
        schemas.insert(name.to_string(), components.write(schema));
    }
    for (name, _, body) in &external {
        schemas.insert(name.to_string(), body.clone());
    }
    for (name, body) in components.shared() {
        schemas.insert(name, body);
    }

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Athletic Standard host API",
            "version": "1",
            "summary": "Read and write Athletic Standard records on an ath-host, within the athlete's grants.",
            "description": "Generated from the Rust definitions in `host/src/operations.rs` and `host/src/shapes.rs` (D266). Do not edit by hand. The guide for this API is `docs/hosting.md` section 7. Every error answer is JSON with a stable code (`Error`). Records and revisions use the published package schemas of the record format version the host reports in `/v1/capabilities`.",
            "license": { "name": "MIT", "identifier": "MIT" }
        },
        "jsonSchemaDialect": "https://json-schema.org/draft/2020-12/schema",
        "tags": [
            { "name": "discovery", "description": "Finding the sign-in provider, the host's features, and the signed-in person." },
            { "name": "athletes", "description": "Records the caller owns." },
            { "name": "records", "description": "The owner's reads and writes. Every write goes through the Athletic Standard library." },
            { "name": "grants", "description": "Coach access, which the athlete offers and can revoke (D262)." },
            { "name": "apps", "description": "The apps the owner uses with a record, which the owner can disconnect (D270)." },
            { "name": "coaching", "description": "What a coach can do under a grant." },
            { "name": "moves", "description": "Moving a record to another host, with one writer at every step (D267)." }
        ],
        "security": [{ "bearer": [] }],
        "paths": paths,
        "components": {
            "schemas": schemas,
            "parameters": {
                "IdempotencyKey": idempotency_key(false),
                "IdempotencyKeyRequired": idempotency_key(true),
            },
            "responses": error_responses(),
            "securitySchemes": {
                "bearer": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "An OAuth access token from the host's OpenID Connect provider (D260). The token must name the app it was issued to in `azp` or `client_id`. `/.well-known/oauth-protected-resource` names the provider."
                }
            }
        }
    })
}

fn idempotency_key(required: bool) -> Value {
    json!({
        "name": "Idempotency-Key",
        "in": "header",
        "required": required,
        "description": "1 to 200 characters. Send the same key when you repeat a request. A repeat with the same key, method, path, and body gets the first answer; the same key with another request gets `409 request_conflict` (D261).",
        "schema": { "type": "string", "minLength": 1, "maxLength": 200 }
    })
}

const ERRORS: &[(&str, &str)] = &[
    ("400", "`invalid`: the request is malformed, or a body does not match its shape."),
    ("401", "`sign_in_required`: no token, or a token the host refuses. The answer has a `WWW-Authenticate` header naming the protected-resource metadata."),
    ("403", "`permission_denied`: the route or permission is not the caller's. `app_disconnected`: the owner disconnected this app from the record."),
    ("404", "`not_found`: no such resource, or nothing the caller may see. An unknown record and one that is not shared get the same answer."),
    ("409", "`stale`, `request_conflict`, or `host_moving`."),
    ("413", "`limit_reached`: the body is larger than the host allows."),
    ("422", "`refused`: the library refused the change."),
    ("500", "`internal`: the host could not finish. Details stay in the host's log."),
];

fn error_responses() -> Value {
    let mut responses = Map::new();
    for (status, description) in ERRORS {
        responses.insert(
            format!("Error{status}"),
            json!({
                "description": description,
                "content": { "application/json": { "schema": { "$ref": format!("{PREFIX}Error") } } }
            }),
        );
    }
    Value::Object(responses)
}

fn shape_name(schema: SchemaRef, named: &[(&'static str, SchemaRef)]) -> &'static str {
    named
        .iter()
        .find(|(_, known)| std::ptr::eq(*known, schema))
        .map(|(name, _)| *name)
        .expect("every request and response body has a name in shapes::named()")
}

fn operation_object(operation: &Operation, named: &[(&'static str, SchemaRef)], components: &mut Components) -> Value {
    let mut out = Map::new();
    out.insert("operationId".into(), operation.id.into());
    out.insert("tags".into(), json!([operation.tag]));
    out.insert("summary".into(), operation.summary.into());
    out.insert("description".into(), who(operation.who).into());
    if operation.who == Who::Anyone || operation.who == Who::CodeHolder {
        out.insert("security".into(), json!([]));
    }

    let mut parameters = Vec::new();
    for name in path_parameters(operation.path) {
        parameters.push(json!({
            "name": name,
            "in": "path",
            "required": true,
            "schema": path_schema(name, components),
        }));
    }
    for query in operation.query {
        parameters.push(json!({
            "name": query.name,
            "in": "query",
            "required": query.required,
            "description": query.description,
            "schema": components.use_of((query.schema)()),
        }));
    }
    match operation.key {
        Key::Required => parameters.push(json!({ "$ref": "#/components/parameters/IdempotencyKeyRequired" })),
        Key::Optional => parameters.push(json!({ "$ref": "#/components/parameters/IdempotencyKey" })),
        Key::None | Key::NotRemembered => {}
    }
    if operation.key == Key::NotRemembered {
        out.insert(
            "x-athleticstandard-repeat".into(),
            "Not answered from memory: the answer carries a one-time code that the host stores only as a hash. The summary says what a repeat does.".into(),
        );
    }
    if !parameters.is_empty() {
        out.insert("parameters".into(), Value::Array(parameters));
    }

    let request = operation.request;
    match request {
        Body::Empty => {}
        Body::Json { schema, required } => {
            out.insert(
                "requestBody".into(),
                json!({
                    "required": required,
                    "content": { "application/json": { "schema": { "$ref": format!("{PREFIX}{}", shape_name(schema(), named)) } } }
                }),
            );
        }
        Body::Zip => {
            out.insert(
                "requestBody".into(),
                json!({
                    "required": true,
                    "description": "A zip of a complete package folder: the files `ath export` writes, at the root of the zip.",
                    "content": { "application/zip": {} }
                }),
            );
        }
        Body::File => {
            let mut content = Map::new();
            for media in MEDIA_FILES {
                content.insert(media.to_string(), json!({}));
            }
            out.insert(
                "requestBody".into(),
                json!({ "required": true, "description": "The file's bytes. Other file types get `422 refused`.", "content": content }),
            );
        }
    }

    let mut responses = Map::new();
    responses.insert(operation.status.to_string(), success(operation, named));
    for status in error_statuses(operation) {
        responses.insert(status.to_string(), json!({ "$ref": format!("#/components/responses/Error{status}") }));
    }
    out.insert("responses".into(), Value::Object(responses));
    Value::Object(out)
}

fn success(operation: &Operation, named: &[(&'static str, SchemaRef)]) -> Value {
    match operation.response {
        Body::Json { schema, .. } => json!({
            "description": "Success.",
            "content": { "application/json": { "schema": { "$ref": format!("{PREFIX}{}", shape_name(schema(), named)) } } }
        }),
        Body::Zip => json!({
            "description": "The complete package as a zip.",
            "headers": {
                "x-manifest-digest": {
                    "description": "The SHA-256 of the package's `export-manifest.json`, as 64 lowercase hexadecimal digits.",
                    "schema": { "type": "string" }
                }
            },
            "content": { "application/zip": {} }
        }),
        Body::File => {
            let mut content = Map::new();
            for media in MEDIA_FILES {
                content.insert(media.to_string(), json!({}));
            }
            json!({ "description": "The file's bytes, with its media type.", "content": content })
        }
        Body::Empty => json!({ "description": "Success." }),
    }
}

/// The error statuses an operation can answer with.
fn error_statuses(operation: &Operation) -> Vec<u16> {
    let writes = operation.method != "GET";
    let has_body = !matches!(operation.request, Body::Empty);
    let mut statuses = Vec::new();
    if has_body || !operation.query.is_empty() || operation.key == Key::Required {
        statuses.push(400);
    }
    if !matches!(operation.who, Who::Anyone | Who::CodeHolder) {
        statuses.push(401);
    }
    if operation.path.contains("{id}") || operation.id == "createAthlete" {
        statuses.push(403);
    }
    if operation.path.contains('{') || operation.id == "acceptGrantOffer" {
        statuses.push(404);
    }
    if (writes && operation.who != Who::CodeHolder) || operation.id == "listChanges" {
        statuses.push(409);
    }
    if has_body {
        statuses.push(413);
    }
    if writes && operation.who != Who::CodeHolder && operation.id != "acceptGrantOffer" {
        statuses.push(422);
    }
    if operation.who != Who::Anyone {
        statuses.push(500);
    }
    statuses
}

fn who(who: Who) -> String {
    match who {
        Who::Anyone => "Who may call: anyone. No token is needed.".to_string(),
        Who::SignedIn => "Who may call: any signed-in person.".to_string(),
        Who::Owner => {
            "Who may call: the record's owner, through an app not disconnected from the record (D270).".to_string()
        }
        Who::OwnerOrCoach => "Who may call: the owner, or a coach with an active grant through this app.".to_string(),
        Who::Coach(permission) => format!(
            "Who may call: a coach with an active grant through this app that includes `{}` (D262).",
            permission.as_str()
        ),
        Who::CodeHolder => {
            "Who may call: anyone holding the activation code; the destination host calls it.".to_string()
        }
    }
}

fn path_parameters(path: &str) -> Vec<&str> {
    path.split('/').filter_map(|segment| segment.strip_prefix('{').and_then(|rest| rest.strip_suffix('}'))).collect()
}

fn path_schema(name: &str, components: &mut Components) -> Value {
    match name {
        "id" => components.use_of(shapes::athlete_id()),
        "record_id" => components.use_of(record_id()),
        "client_id" => {
            json!({ "type": "string", "minLength": 1, "description": "The app's client ID, percent-encoded." })
        }
        _ => json!({ "type": "string", "minLength": 1 }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operation_has_a_unique_id_and_route() {
        let routes = routes();
        let mut ids = std::collections::BTreeSet::new();
        let mut routes_seen = std::collections::BTreeSet::new();
        for route in &routes {
            assert!(ids.insert(route.operation.id), "{} is listed twice", route.operation.id);
            assert!(
                routes_seen.insert((route.operation.method, route.operation.path)),
                "{} {} is listed twice",
                route.operation.method,
                route.operation.path
            );
        }
    }

    #[test]
    fn every_reference_points_at_a_definition() {
        let description = description();
        let schemas = description["components"]["schemas"].as_object().unwrap();
        let text = description.to_string();
        for (index, _) in text.match_indices(PREFIX) {
            let rest = &text[index + PREFIX.len()..];
            let name: String = rest.chars().take_while(|c| *c != '"').collect();
            assert!(schemas.contains_key(&name), "{name} is referenced but not defined");
        }
    }
}
