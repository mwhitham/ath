# PR verification record

Copy the relevant sections into the PR. Delete inapplicable rows with a short reason.
This records evidence; it does not replace the approved spec or decision history.

- Tested head SHA / base SHA:
- Builder/tool / run evidence (unknown is acceptable):
- Environment (OS, Rust toolchain, Node when the npm package is exercised):
- Approved public spec and acceptance criteria:

| Check | Exact command or action | Result / public-safe evidence |
| --- | --- | --- |
| Required local checks | `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; in `packages/npm`: `npm ci`, `npm run typecheck`, `npm test` | |
| Decisions / generated outputs | `ATH_BASE_SHA=<base> cargo test --locked --test decision_history -- --ignored working_tree_matches_recorded_decisions` / `cargo test --test generated` | |
| Changed behavior and refusal | Small synthetic regression, including nearby valid case | |
| Installed package, when affected | Full check archive smoke (`tools/archive-smoke.sh`) and npm smoke (`packages/npm/scripts/smoke.mjs`) | |
| CI | Links to checks for this head and current base. The required context is `checks passed`. Include the full check when a command, package, or install changes. | |
| Website, when affected | Actual site's build, narrow/wide layout, changed interaction | |

## You test this (Laptop or Phone)

Give at most five steps using synthetic data. State the expected result and what
failure looks like. For CLI changes, show the installed package. For visual changes,
include accessible screenshots. Add video only when it helps prove the interaction.
Label artifacts with the tested revision. An inaccessible link is not verified proof;
provide exact commands and sanitized text output if media cannot be viewed.

## Review disposition and learning

- Independent reviewer, reviewed head/base, findings or no findings:
- Fixed: finding link, reproducer, test, fix commit:
- Disagreed/deferred: evidence and maintainer decision, not silent dismissal:
- Repeated cause → smallest regression or instruction → evidence it catches the miss:
- Remaining limits / checks not run:

A new push or base change requires refreshed verification. No findings is not proof
that every acceptance criterion passed. A bot response is not maintainer approval.
