# iOS verification app (planned)

Small SwiftUI app for the Rust foundation proof (CLI → iPhone correction → CLI).

- **Owner:** Grokbot verification work coordinates scenarios; Rust builders own this package layout.
- **Depends on:** Swift package generated from `crates/ath-uniffi`.
- **Gate:** must run in the **iPhone Simulator** on macOS with Xcode installed.

Not buildable in Linux CI. See `build-history/plans/rust-adoption.md`.
