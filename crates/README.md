# Rust workspace

| Crate | Role |
|---|---|
| `ath-core` | Domain and storage (embeds `spec/sql/migrations/`) |
| `ath-node` | napi-rs bindings for `athleticstandard` |
| `ath-uniffi` | Scaffold for Swift/Kotlin UniFFI (generation wired in Foundation stage) |

Build:

```sh
cargo check --workspace
cargo test --workspace
```

Requires Rust **1.88** (see `rust-toolchain.toml`).
