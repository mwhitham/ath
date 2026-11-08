# Rust engine changes

Root `AGENTS.md` and the applicable approved spec remain authoritative. Use
`.agents/skills/ath-engineering/SKILL.md` for execution and review follow-up.

## Code Review Rules

- Preserve the recorded import transaction and re-import rules (D182, D189, D190,
  D196). A late invalid mapped row must leave no earlier writes. Re-import must not
  resurrect a deleted source or add revisions to unchanged mapped records. Require
  a synthetic regression for the touched path; do not extend this to a different
  import contract without checking its decision.
- Follow values through storage, correction, check, and export. Flag dropped unknown
  fields, changed timestamp meaning, or precision loss between typed rows and
  revisions (D166, D169, D191). Check a nearby valid case as well as the refusal.
- Keep provider layout in mapping files where D193 requires it. Do not silently
  narrow supported input shapes or mix HRV statistics to simplify a parser.
