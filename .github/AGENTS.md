# Repository automation

Root `AGENTS.md` remains authoritative. Existing `CODEOWNERS` covers every path.

## Code Review Rules

- PR checks use unprivileged `pull_request` execution with read-only repository
  permissions. Do not execute PR code in `pull_request_target`, grant it private
  repository access, or pass it release/reviewer secrets. Keep publishing confined
  to the existing tag path; a PR or review comment must not publish or merge.
- Preserve installed-package and cross-platform coverage (D170, D171, D188, D197).
  Successful installation exit codes alone do not establish that a required tool
  exists. Validate the outcome and test failure paths for changed setup logic.
- Changes to checks, review rules, fixture allowlists, and ownership require owner
  scrutiny. A builder must not weaken the check that reports its failure.
