# Five-PR engineering audit

Snapshot: 2026-11-08. This is an operational audit of public PR evidence, not a new
architecture decision or a replacement history. Selection covers a release build,
a mapping plan, its implementation, a pending integration plan, and ownership rules.
The review histories and changed-file lists were read; relevant patches and landed
checks were inspected. Historical failure claims below are attributed to those
reviews, not presented as newly rerun failures on every old commit.

## Work origin and limits

Of the 30 recent PRs inspected, 29 carried a `cursor/` branch and Cursor PR-body/run
markers. The remaining PR, #41, says it was generated with Claude Code. All use the
same GitHub author account. These markers establish tool origin, not which model or
person did each step. A search for Grok references found review/proof discussion in
#35 and #39, not reliable evidence of GrokBot-authored code. No worker run was
independently attributable to GrokBot in this sample. Ask for a public-safe run or PR
reference before making GrokBot-specific performance claims.

## Findings by PR

| PR and evidence | Origin / review history | What the evidence shows |
| --- | --- | --- |
| [#35: version 2.0.0](https://github.com/mwhitham/ath/pull/35) | Cursor markers; 16 public timeline entries, including 11 numbered code rounds, addenda/proof follow-ups, and a Copilot quota failure | Green tests initially missed a package without its Rust executable, provider parsing errors, and precision loss on correction. Later route fixes missed neighboring zip, path, counting, and multiple-file cases. A code pass was revised after more evidence. |
| [#38: mapping plan](https://github.com/mwhitham/ath/pull/38) | Cursor markers; 13 public review comments, through re-check 12 | The first review found a baseline taken after modification, missing input paths, schema-invalid units, undocumented decisions, and no installed-package checks. Rewrites introduced new contradictions. Later reviews corrected provider-version and envelope/null-resolution assumptions. |
| [#39: mapping implementation](https://github.com/mwhitham/ath/pull/39) | Cursor markers; 3 public gate comments; later fixes also described in the final PR body | The first gate found a UTC-only test, inaccessible/incomplete proof, and approval claims without a traceable source. The final body records partial mapped writes, re-import after source deletion, lost supplied fields, product-specific engine rules, and release failures hidden by Linux-only TypeScript PR testing. |
| [#40: 2.2.0 integration plan](https://github.com/mwhitham/ath/pull/40) | Cursor markers; 0 public timeline entries returned | Six changed files are guidance/history, not runtime code. The body explicitly says earlier-head reviews do not cover the new plan and its proposed decisions are unadopted. This is a useful boundary; there is no visible review history from which to infer a failure rate. Snapshot head `3bbbed4` merges main's ownership change onto the body-described `d96216e`; that difference is not evidence of a runtime defect. |
| [#41: ownership rules](https://github.com/mwhitham/ath/pull/41) | Claude Code footer; 0 public timeline entries returned | One file adds ownership for every path. No application behavior changed and no defect was established. Live main rules independently confirm required owner review. No extra reviewer agent or ownership file is needed. |

## Recurring causes and prevention

1. **Tests did not exercise what was delivered.**
   [#35 round 2](https://github.com/mwhitham/ath/pull/35#issuecomment-6455829804)
   reported 380 passing tests while the installed package failed. The
   [round-4 addendum](https://github.com/mwhitham/ath/pull/35#issuecomment-6501771869)
   found correction precision and binary-mode problems after an earlier pass.
   #39's final body records platform failures and a successful installer exit that
   left no `sqlite3`. Preserve the existing pack/smoke and cross-platform jobs;
   require exact-head installed proof when that surface changes. Do not add a
   duplicate release workflow.
2. **A fix addressed one example and missed related valid inputs.**
   [#35 round 10](https://github.com/mwhitham/ath/pull/35#issuecomment-6528350489)
   found double-counted refusals and a second route overwriting the first.
   [#38 re-check 11](https://github.com/mwhitham/ath/pull/38#issuecomment-6538884423)
   found sleep-page detection based on the wrong provider field. #39 records late
   invalid-row and deleted-source cases. Scoped importer/engine instructions now ask
   for a failing case plus a nearby valid case and the relevant input forms.
3. **Plans and decisions drifted during revisions.**
   [#38 initial review](https://github.com/mwhitham/ath/pull/38#issuecomment-6537920092)
   and [the next review](https://github.com/mwhitham/ath/pull/38#issuecomment-6538110023)
   show missing decision records and new contradictions after fixes. The spec template
   asks for observable criteria and source versions. The decision-history check
   protects already-recorded text and new numbering; semantic contradictions still
   need review. It does not demand a D-number for every test or documentation edit.
4. **Evidence and approval were hard to verify.**
   [#39 gate 1](https://github.com/mwhitham/ath/pull/39#issuecomment-6545564711)
   found an inaccessible artifact, missing acceptance proof, and untraceable choices.
   #35 also had repeated proof-only follow-ups. #40 acknowledges stale-head review.
   Templates separate the tested revision, approval source, code defects, proof
   gaps, and unrun checks. Text reproduction is useful when media is inaccessible;
   a mandatory video for routine changes would add friction without evidence.

## What this audit does not establish

It does not establish that GrokBots caused these failures, that every review verdict
was correct, or that no further defects remain. Review rounds are not a productivity
score: some changed scope or proof requirements. No private repository or worker
transcript was used. The new loop needs five confirmed GrokBot examples before a
worker-specific comparison is possible. No existing product decision is changed here.
