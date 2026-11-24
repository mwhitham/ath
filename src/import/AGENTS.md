# Provider import

Root `AGENTS.md` and the applicable approved spec remain authoritative. Use
`.agents/skills/ath-engineering/SKILL.md` for execution and review follow-up.
`src/import/` owns provider imports (`docs/architecture.md`). D208 replaced the
TypeScript adapter and `ath-core` split (D173, D174).

## Code Review Rules

- Preserve the public input forms affected by the change: file, folder, and zip.
  For route changes, check export-local paths, missing/offset-less times, repeated
  references, and two route files on one workout (D175, D176). Count each refused
  file once and keep preview explanations consistent with what was stored.
- Readers produce typed candidates and do not write SQL (`docs/architecture.md`).
  Compare a parser change with the unchanged base. Use synthetic inputs and state
  uncovered provider behavior explicitly.
