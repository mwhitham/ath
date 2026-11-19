# v2.1.2 progress

Status: **built; not released**. The plan is [spec.md](spec.md).

| Step | What | Status |
|---|---|---|
| 1 | Plan, decisions D227–D249, audit, and research | done |
| 2 | Version 2.1.2, migration 6, session effort, strength set fields | done |
| 3 | One-record calculation repairs and regression tests | done |
| 4 | Range receipt version 2 | done |
| 5 | Installer: skill kinds, named coaching installs, unedited-copy updates | done |
| 6 | Calculation skill rewrites with executed examples | done |
| 7 | Tool skill update | done |
| 8 | CrossFit and HYROX skills, `hyrox-race-splits` | done |
| 9 | skills.sh preparation and contribution standard | done |
| 10 | Agent evaluations, installation tests, repository checks | done |
| 11 | Setup guidance in every skill, from one generated source (D250) | done |

Step 10 results are in the [agent evaluation](appendix/agent-evaluation.md). Every
required check passed on 2026-11-16: `cargo fmt --check`, `cargo clippy
--all-targets -- -D warnings`, `cargo test` (the six `cli_terminal` tests need a
real terminal and were run outside the build sandbox), and the npm `ci`,
`typecheck`, and `test` steps.

Known limits of this build:

- Updating a skill through the skills CLI from a local folder was not verified.
  `ath skill install` updates are tested.
- The seeded `hyrox-full` benchmark definition still says "wall balls 100/75 reps".
  The 2026/27 rulebooks give 100 reps in every Singles division. The `hyrox` skill
  tells agents to follow the rulebook. Changing seeded text is left for a later
  version.

Step 11 is in D250 and [skill-prerequisites.md](appendix/skill-prerequisites.md).
Agents without a working `ath` now stop and say how to install it, instead of
reading the database by hand. npm's `latest` is still 1.1.0, so 2.1.2 must be
published to npm before the skills are listed.

Outstanding outside this build:

- Independent review by qualified CrossFit and HYROX coaches (D247). An AI review
  does not count.
- Publication to npm and listing on skills.sh. The maintainer does both manually.
