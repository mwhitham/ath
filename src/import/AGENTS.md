# Import adapters

Root `AGENTS.md` and the applicable approved spec remain authoritative.

## Code Review Rules

- Preserve the public input forms affected by the change: file, folder, and zip.
  For route changes, check export-local paths, missing/offset-less times, repeated
  references, and two route files on one workout (D175, D176). Count each refused
  file once and keep preview explanations consistent with what was stored.
- Preserve the current division of work between these adapters and `ath-core`
  (D173, D174). Compare parity with an unchanged base before moving a parser.
  Use synthetic inputs and state uncovered provider behavior explicitly.
