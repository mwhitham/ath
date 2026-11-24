# Repository automation

Root `AGENTS.md` remains authoritative. Existing `CODEOWNERS` covers every path.

## Code Review Rules

- PR checks use unprivileged `pull_request` execution with read-only repository
  permissions. Do not execute PR code in `pull_request_target`, grant it private
  repository access, or pass it release/reviewer secrets. Keep publishing confined
  to the existing tag path; a PR or review comment must not publish or merge.
- Preserve the quick check and the full check (D170, D171, D188, D225, D226).
  Pull requests run formatting, Clippy, and the Rust tests on Linux, macOS, and
  Windows, plus the npm tests on Linux. A release runs the archive and npm
  install checks. A successful install exit code alone does not establish that
  a required tool exists. Validate the outcome and test failure paths for
  changed setup logic.
- Changes to checks, review rules, fixture allowlists, and ownership require owner
  scrutiny. A builder must not weaken the check that reports its failure.
