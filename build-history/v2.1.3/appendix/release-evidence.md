# Version 2.1.3 build evidence

This is the evidence from building 2.1.3 (spec section 9 and build step 10). It was
gathered on 2026-11-23 on an Apple silicon Mac (arm64) running macOS 26.6.2, with Rust
1.100.0, Node 25.6.1, and Swift 6.4. All data was synthetic. No live provider account
was used.

## 1. Checks

| Check | Result |
|---|---|
| `cargo fmt --check` | Passes. |
| `cargo clippy --all-targets -- -D warnings` | Passes, for the library, the CLI, and `ath-host`. |
| `cargo test` | 778 passed, 0 failed, 15 ignored. The six `cli_terminal` tests need a real terminal; they pass outside the build sandbox. |
| `npm ci`, `npm run typecheck`, `npm test` | 101 passed, 1 skipped (a Windows-only test). Includes the host client run against a real `ath-host`. |
| `examples/host-mcp`: `npm test` | 7 passed, against a real `ath-host`. |
| `examples/host-swift`: `swift test` | 4 passed on macOS, against a real `ath-host`. |

## 2. What the tests cover

| Area | Where | What it shows |
|---|---|---|
| Version 2.1.3, migration 007, coach authors | `tests/record_2_1_3.rs`, `tests/migration.rs` | A coach note survives export and restore byte for byte. Coach authors are refused on measured records and athlete reports. The change log matches each committed revision. |
| Recording and replaying writes (D268, D269) | `tests/replica.rs` | Create, correct, delete, erase, and a whole package restore replay onto another copy with the same rows and the same export. A replay from an old revision or access epoch is refused. A server's log is contiguous, and an erasure clears it. The schema scripts build the same schema as a new file. |
| The host API, both modes | `host/tests/host_api.rs` | A report with a photo is saved and opened on another device. Retries return the first answer. A coach sees and adds only what was granted. A revocation blocks a waiting write. Two host copies share one record. Forged IDs look like records that are not shared. |
| Moves and backups (D264, D267) | `host/tests/moves.rs` | A move has one writer at every step, and an activation code works once, only for the destination and digest it names. A record with a photo moves from directory mode to remote mode and back, intact. A restore from an older backup keeps later erasures. An unfinished erasure is finished on start. |
| Disconnecting one app (D270) | `host/tests/host_api.rs`, `packages/npm/test/host.test.ts` | One athlete uses two apps with one record. After the athlete disconnects one, its next read or write is refused, its list hides the record, and it cannot connect itself again. The last connected app cannot be disconnected. The access epoch rises, so a waiting owner write fails. The other app connects it again. |
| Failures | `host/tests/faults.rs` | A failed attachment upload and a commit that fails partway leave the server unchanged, and a retry writes once. A restart clears working copies and leftover staging. An expired upload cannot be used. |
| Clients | `packages/npm/test/host.test.ts`, `examples/host-mcp/test`, `examples/host-swift/Tests` | The npm client, the starter, the MCP tools, and the Swift client run the owner and coach flows against the host. |

## 3. Timings at 100,000 records

`host/tests/scale.rs`, release build. The workload is 100,000 invented athlete reports,
not the project's 100,000- and 3-million-record performance workloads, which need the
2.1.0 generator. The package was 2.7 MB. The remote mode used a local file in place of
the server, so these times include no network.

| Operation | Directory mode | Remote mode |
|---|---:|---:|
| First transfer (upload and restore the package) | 6.3 s | 20.1 s |
| Export | 1.8 s | 1.4 s |
| Refresh with nothing changed | 5 ms | 2 ms |
| One write | 7 ms | 8 ms |
| Refresh after one note | 3 ms | 3 ms |
| Backup with a trial restore | 10.2 s | 9.6 s |
| One-athlete restore | 6.3 s | not offered |
| A second host copy builds its working copy | — | 7.3 s |

A small write for a second athlete during a 3,000-record restore for the first took
3.8 ms in directory mode and 17.8 ms in remote mode, and finished before the restore.

In remote mode the first transfer records every row, replays it on the working copy,
and commits it to the server. Insertions are sent in scripts of up to 500 statements,
so 100,000 records take about a thousand round trips, not hundreds of thousands. A
real server adds one network round trip per script.

## 4. What passed, by storage option

| Option | Status |
|---|---|
| Native directory | Passed every test above. |
| Remote mode with a local file in place of the server | Passed every test above. |
| A self-run libSQL server | Not run. `host_api.rs` has an ignored qualification test for it. Running the downloaded server binary was not allowed in this build environment. |
| Turso Cloud, libSQL-engine databases | Not run. Needs an account; the same qualification test applies. |
| Bunny Database | Not run; in preview (D258). |
| S3-compatible buckets | Tested with a stand-in bucket that ignores signatures. No real bucket was used. |

## 5. Not verified

- Sign-in through WorkOS AuthKit or Keycloak. Tokens in the tests are signed by a fixed
  test-only Ed25519 key.
- The Swift client on iOS. Only macOS ran; Xcode was not set up for iOS builds here.
- The container image. Docker was not running, so `host/Dockerfile` was not built.
- The Litestream recipe. No Litestream binary was run.

## 6. Not built in 2.1.3

- Per-athlete encryption keys on the libSQL backend (D264).
- Routing between several machines in directory mode (spec 6.1).
- A generated description of the API in `schema/host-api/1/` (D266).
- Applying a refresh change by change on a device (D263). The host lists the changes;
  a device refreshes by downloading the full package and checking its digest.
- Attachments through a staging key with signed download links and an orphan cleanup
  job (D259). Attachments go to their final key before the commit; a failed commit can
  leave an unused object.
- The limits in spec 6.10 beyond the request body size and upload expiry.
