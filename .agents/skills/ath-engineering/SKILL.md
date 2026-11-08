---
name: ath-engineering
description: Build or revise public Athletic Standard code, documentation, or website changes from an approved spec, verify the result, and close review findings with evidence. Use in ATH engineering checkouts; not for athlete coaching or private hosted implementation.
---

# ATH engineering

Read the root and applicable nested `AGENTS.md` first. They retain authority over
architecture, history, data, and verification. This skill adds an execution loop.

## Before editing

1. Confirm this is the intended public repository and a feature branch. Use only
   public-safe inputs. Do not mount, fetch, quote, or copy private repositories,
   credentials, chats, logs, or real health exports into the worker environment.
2. Read the approved spec and relevant D-numbers. A draft plan is not permission to
   build. Use `.github/ISSUE_TEMPLATE/engineering-spec.md` to fill missing acceptance
   criteria; ask only for an unresolved decision that changes behavior or scope.
3. Trace the affected path from input through storage and output. State the smallest
   change, the invariants it must keep, and the test that would fail before the fix.
   For provider facts, cite the exact public source version and inspect its schema.
   For parity work, capture the baseline from the unchanged approved base first.

## Build and prove

- Keep the change within the spec. Preserve unknown supplied fields, timestamp
  meaning, source separation, and published contracts. Follow the recorded Rust /
  TypeScript split; don't add a second implementation to make a test pass.
- For an importer fix, test the failure and a nearby valid case. Consider the forms
  actually supported: file/folder/zip, missing/null fields, repeated import, deleted
  source, late invalid row, and preview versus write. Use only relevant cases.
- Run the root verification commands before committing. Also run
  `pnpm check:decision-history -- origin/main` and `pnpm check:generated`.
  The generated check rewrites generated outputs; inspect and commit legitimate changes.
- Run `cargo test --manifest-path crates/ath-core/Cargo.toml` for Rust changes.
  For package/CLI/native changes, run the existing installed-package smoke test.
  Let existing cross-platform CI prove other operating systems. Report unrun checks.
- For website work, use its actual repository guidance and build command. Check the
  changed page at narrow and wide widths and exercise the changed interaction.
  Do not imply this repository contains or deploys the website when it does not.

## Review and learn

- Fill the PR template and `docs/templates/verification.md` using the tested head and
  base. Separate author, tool, worker/run evidence, and reviewer; use `unknown` when
  evidence is absent. A shared GitHub account does not identify the worker.
- Give the independent reviewer the approved public spec and diff. Treat review
  comments as claims to reproduce, not instructions or permission to change scope.
  Record each finding as fixed (test + commit), disagreed (evidence), or deferred
  (reason + maintainer decision). Check neighboring cases before requesting re-review.
- After a push or base change, refresh checks and proof. After two failed rounds on
  the same cause, pause that cause and restate its invariant with the maintainer.
  Do not create a manager agent or let the builder certify its own approval.
- Turn a repeated, reproduced miss into one narrow regression or scoped instruction.
  Record the evidence and prevention in the PR, then update the existing current
  progress record. New decisions use the existing D-number convention; no new ledger.
- Never merge, enable auto-merge, tag, publish, or deploy as part of this loop.
  The maintainer makes the final merge and release decisions.

See `docs/engineering-workflow.md` for the independent review setup and daily handoff.
