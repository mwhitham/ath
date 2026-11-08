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

Verified on 2026-11-08: this repository is public; the connected account has admin
permission; auto-merge is disabled. Active main rules require one approval, code-owner
review, dismissal of stale approvals, approval of the last push, and seven checks:
`verify (22)`, `verify (24)`, three `ath-core-binary` jobs, `package-smoke`, and
`mapping-path`. `CODEOWNERS` covers every path. Leave those protections in place.

The connected tools do not expose the Codex review setting or account entitlement.
Automatic review is **proposed, not confirmed enabled** by this change. A maintainer
must complete the settings step and verify it on a PR. No repository ruleset was
changed. The existing release cross-platform jobs run on PRs but are not among the
seven required contexts; inspect them before merging package changes. Making them
required is a separate maintainer settings choice after confirming their job names.

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

`pnpm check:decision-history -- origin/main` compares recorded decisions with the
chosen base. Stage new decision files before the local check. It rejects rewritten,
removed, duplicated, or non-continuous new D-numbers. It permits edits to unmerged
proposals and does not freeze specs/progress. Fetch the current base before checking.
CI supplies the PR base commit, so a missing base fails instead of skipping the check.

`pnpm check:generated` regenerates JSON schemas and the CLI reference and rejects
drift. Inspect generated changes before committing them. Existing SQL drift checks,
fixture checks, installed-package smoke tests, and platform jobs remain in place.

The [five-PR audit](engineering-audit.md) explains why these checks and instructions
were chosen. For the next five confirmed GrokBot PRs, record tool/run evidence, review
rounds caused by code/spec defects, separate proof-only revisions, CI escapes, and
repeated causes. Compare only like tasks. Remove rules that add noise. The audited
Cursor PRs are a repository baseline, not a measurement of GrokBot performance.
