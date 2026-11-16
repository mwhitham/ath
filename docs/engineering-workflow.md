# Public engineering workflow

Refine the public spec in ChatGPT, then give its GitHub link to a GrokBot worker.
GrokBots build public code and website changes. Codex independently reviews the PR.
The worker fixes reproduced findings, and the maintainer decides whether to merge.
Codex builds private work in a separate private environment. No manager agent is needed.

## Daily use

1. Write a public-safe [engineering spec](../.github/ISSUE_TEMPLATE/engineering-spec.md).
   Link the approved build-history spec and decisions. Mark unresolved choices.
2. Give the worker the issue link and this instruction:
   "Read AGENTS.md and .agents/skills/ath-engineering/SKILL.md. Implement the approved
   scope on a branch. Open a PR with acceptance evidence. Do not merge or publish."
3. The worker fills the PR template and [verification record](templates/verification.md).
   Each substantial claim names the tested head/base. Routine docs changes need no video.
4. Independent Codex review checks the spec and diff. The worker reproduces findings
   and records fixes, disagreements, and deferrals. Refresh review after code/base changes.
5. After repeated misses, add one regression or narrow instruction tied to the
   finding. Update current build progress; use the existing decision log for new
   decisions. The maintainer merges only after checks and review are complete.

The skill lives under `.agents/skills`, outside the package's runtime `skills/`.
If the worker does not discover repository skills, explicitly give it the path above.
No GrokBot API or automatic issue trigger is assumed. For a separate public website
repository, use the same handoff with its own guidance and checks; this PR does not
grant access to that repository or deploy it.

## Independent Codex review setup

Use the built-in GitHub integration. It does not require an API key in ATH Actions.
In [Code Review settings](https://chatgpt.com/settings/code-review), connect/select
only the public repository needed for this workflow. Enable code review and choose
automatic review for **All PRs**, so worker-authored PRs are covered. Choose the
available review trigger for new PRs and updates. For a manual request or re-review,
comment `@codex review` on the PR and confirm an actual review response arrives.
An eyes reaction alone is not a completed review.

The [official setup guide](https://learn.chatgpt.com/docs/third-party/github) requires
push/admin permission and documents repository `AGENTS.md` review rules. It currently
describes GitHub findings as P0/P1 only. Keep acceptance checks and human review for
issues below that threshold. See also the
[custom-rule guidance](https://developers.openai.com/blog/custom-code-review-rules-for-codex).

Verified on 2026-11-08, before the 2.1.1 rebuild: this repository is public; the
connected account has admin permission; auto-merge is disabled. The rules then
required one approval, code-owner review, dismissal of stale approvals, approval
of the last push, and seven checks: `verify (22)`, `verify (24)`, three
`ath-core-binary` jobs, `package-smoke`, and `mapping-path`. `CODEOWNERS` still
covers every path.

D225 and D226 replaced those seven checks. Pull requests now run the quick check
in `.github/workflows/ci.yml`. Linux runs formatting, Clippy, every Rust test, the
decision-history check, and the npm tests on Node 24. macOS and Windows run Clippy
and every Rust test. Windows also runs the Credential Manager test. The one
required status check is `checks passed`, and it passes only when those three
jobs pass. The full check, including release builds and the archive and npm
install checks, runs on a version tag and by hand
(`.github/workflows/release-cross-platform.yml`). It is not one of the required
pull-request contexts. This merge does not change the ruleset. Leave the approval
rules and `checks passed` in place.

The existing Codex integration completed a review of this implementation on
[`e0d872d` in PR #42](https://github.com/mwhitham/ath/pull/42#issuecomment-6726172498)
and reported no major issues. No new reviewer service or credential was needed.
Its summary labels the trigger "Manual request". The connected tools do not expose
the review settings, so **All PRs and update triggers remain unverified**. Check those
settings for coverage of GrokBot PRs; use the manual trigger if needed. No repository
ruleset was changed. Inspect the full check before merging a command, package,
or install change. Making more contexts required is a separate maintainer
settings choice.

If review is unavailable, paste the public PR and approved public spec into a fresh
Codex chat: "Review this PR without editing. Read applicable AGENTS.md. Verify each
acceptance criterion against the diff. Report actionable findings with severity,
path, reproduction, reviewed head/base, and checks you could not run. Do not merge."
Record the result in the PR. Missing access, quota, or a missing bot response is an
unperformed review, never a pass. The builder's self-check is not independent review.

## Public and private separation

- Use separate workers, checkouts, tool connections, and repository permissions.
  Public workers must have no private-repository token, mount, checkout, or connector.
  Grant access to selected public repositories, not all repositories in the account.
- Specs, comments, screenshots, test data, and logs in public work must be public-safe
  from the start. Do not transfer private code or chats for a worker to "sanitize".
  Describe only an approved public interface when public and private work interact.
- Real health exports stay outside Git in encrypted storage under the existing rule.
  Public CI uses synthetic fixtures only. A fixture hash check is not a secret scanner
  and does not prove the absence of private prose or credentials.
- New CI checks here use read-only permissions and no secrets. No cross-repository
  dispatch, privileged review job, auto-fix bot, merge command, or deployment is added.
  Owner review of rules, workflows, and fixture allowlists remains necessary.

## Checks and learning

`ATH_BASE_SHA=origin/main cargo test --locked --test decision_history -- --ignored working_tree_matches_recorded_decisions`
compares recorded decisions with the chosen base. Stage new decision files before
the local check. It rejects rewritten, removed, duplicated, or non-continuous new
D-numbers. It permits edits to unmerged proposals and does not freeze specs or
progress. Fetch the current base before checking. CI supplies the pull-request base
commit, so a missing base fails instead of skipping the check.

`cargo test --test generated` checks JSON Schema, the npm protocol module, the
command reference, and the demo database (D219).
`ATH_WRITE_GENERATED=1 cargo test --test generated` rewrites them. Inspect generated
changes before committing them. It does not rewrite a published historical schema,
`spec/sql/`, or `tests/fixtures/`. The public-data check is `tests/public_data.rs`.
Archive and npm install checks stay in the full check.

The [five-PR audit](engineering-audit.md) explains why these checks and instructions
were chosen. For the next five confirmed GrokBot PRs, record tool/run evidence, review
rounds caused by code/spec defects, separate proof-only revisions, CI escapes, and
repeated causes. Compare only like tasks. Remove rules that add noise. The audited
Cursor PRs are a repository baseline, not a measurement of GrokBot performance.
