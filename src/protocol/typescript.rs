//! Generate the npm client's protocol module from the operation definitions.
//!
//! The generated file holds the protocol version, the request and response
//! types of every operation, the client's dispatch table, and the native
//! constants the JavaScript exports repeat. `tests/generated.rs`
//! compares the committed file with this output; setting
//! `ATH_WRITE_GENERATED=1` rewrites it.

use std::fmt::Write as _;

use super::definition::{Kind, OPERATIONS};
use super::{MODE_ARGUMENT, PROTOCOL_VERSION};
use crate::json::{self, Value};

/// The path of the generated file, relative to the repository root.
pub const GENERATED_PATH: &str = "packages/npm/src/generated/protocol.ts";

/// Native values the JavaScript exports repeat, in published order.
pub fn constants() -> Vec<(&'static str, Value)> {
    use crate::calculate::range::{Operation, RecordKind};
    vec![
        ("ATHLETIC_STANDARD_VERSION", Value::from(crate::model::package::ATHLETIC_STANDARD_VERSION)),
        ("SQL_SCHEMA_VERSION", Value::from(crate::store::SQL_SCHEMA_VERSION)),
        ("SQLITE_APPLICATION_ID", Value::from(crate::store::SQLITE_APPLICATION_ID)),
        ("SQLITE_APPLICATION_MARKER", Value::from(crate::store::SQLITE_APPLICATION_MARKER)),
        ("MAX_ATTACHMENT_BYTES", Value::from(crate::input_policy::MAX_ATTACHMENT_BYTES)),
        (
            "CALCULATION_OPERATIONS",
            Value::Array(Operation::ALL.iter().map(|item| Value::from(item.as_str())).collect()),
        ),
        (
            "CALCULATION_RECORD_KINDS",
            Value::Array(RecordKind::ALL.iter().map(|item| Value::from(item.as_str())).collect()),
        ),
    ]
}

/// The complete text of the generated TypeScript module.
pub fn generate() -> String {
    let mut out = String::new();
    out.push_str(
        "// Generated from src/protocol by tests/generated.rs.\n\
         // Do not edit. Run `ATH_WRITE_GENERATED=1 cargo test --test generated`.\n\n",
    );
    out.push_str("import type * as Model from \"../schema.js\";\n");
    out.push_str("import type * as Api from \"../types.js\";\n\n");
    out.push_str(
        "/** A prepared attachment file sent with the write that records it. */\n\
         export interface AttachmentFile {\n  record_id: string;\n  created_at: string;\n  changed_by: unknown;\n  data: unknown;\n  bytes_base64: string;\n}\n\n",
    );
    out.push_str("/** The client protocol version this package speaks. */\n");
    let _ = writeln!(out, "export const PROTOCOL_VERSION = {PROTOCOL_VERSION} as const;\n");
    out.push_str("/** The sole argument that starts a protocol worker. */\n");
    let _ =
        writeln!(out, "export const WORKER_ARGUMENT = {} as const;\n", json::to_string(&Value::from(MODE_ARGUMENT)));

    out.push_str("/** Values the native library defines and the JavaScript exports repeat. */\n");
    out.push_str("export const NATIVE_CONSTANTS = {\n");
    for (name, value) in constants() {
        let _ = writeln!(out, "  {name}: {},", json::to_string(&value));
    }
    out.push_str("} as const;\n\n");

    out.push_str("/** The request and response types of every operation. */\n");
    out.push_str("export interface OperationTypes {\n");
    for operation in OPERATIONS.iter() {
        let _ = writeln!(out, "  {}: {{", json::to_string(&Value::from(operation.name)));
        let _ = writeln!(out, "    input: {};", indent(operation.input));
        let _ = writeln!(out, "    output: {};", indent(operation.output));
        let _ = writeln!(out, "    item: {};", operation.items.map(indent).unwrap_or_else(|| "never".into()));
        if operation.callbacks.is_empty() {
            out.push_str("    callbacks: Record<never, never>;\n");
        } else {
            out.push_str("    callbacks: {\n");
            for callback in operation.callbacks {
                let _ = writeln!(
                    out,
                    "      {}: (input: {}) => Promise<{}>;",
                    callback.name,
                    indent(callback.input),
                    indent(callback.output)
                );
            }
            out.push_str("    };\n");
        }
        out.push_str("  };\n");
    }
    out.push_str("}\n\n");

    out.push_str("/** Every operation name. */\n");
    out.push_str("export type OperationName = keyof OperationTypes;\n\n");
    out.push_str("/** Operations the client runs with a synchronous child process. */\n");
    out.push_str("export type SyncOperationName = {\n");
    out.push_str("  [Name in OperationName]: (typeof OPERATIONS)[Name][\"kind\"] extends \"sync\" ? Name : never;\n");
    out.push_str("}[OperationName];\n\n");
    out.push_str("/** Operations the client runs with a promise. */\n");
    out.push_str("export type AsyncOperationName = Exclude<OperationName, SyncOperationName>;\n\n");

    out.push_str("/** How the client dispatches each operation. */\n");
    out.push_str("export const OPERATIONS = {\n");
    for operation in OPERATIONS.iter() {
        let callbacks: Vec<String> =
            operation.callbacks.iter().map(|callback| json::to_string(&Value::from(callback.name))).collect();
        let _ = writeln!(
            out,
            "  {}: {{ kind: {}, target: {}, streams: {}, callbacks: [{}] }},",
            json::to_string(&Value::from(operation.name)),
            json::to_string(&Value::from(operation.kind.as_str())),
            json::to_string(&Value::from(operation.target.as_str())),
            operation.items.is_some(),
            callbacks.join(", ")
        );
    }
    out.push_str("} as const;\n");
    debug_assert!(OPERATIONS.iter().all(|operation| operation.kind == Kind::Async
        || (operation.target == super::TargetUse::None
            && operation.callbacks.is_empty()
            && operation.items.is_none())));
    out
}

/// Indent continuation lines of a multi-line type so the output reads well.
fn indent(text: &str) -> String {
    text.trim().lines().collect::<Vec<_>>().join("\n    ")
}
