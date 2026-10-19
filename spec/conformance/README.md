# Conformance vectors

Language-neutral behavior fixtures for Athletic Standard v1. Each case has a JSON
manifest with independently specified expectations. Rust, Node, and Swift runners
should load the same files (CR5 / Rust prerequisite R2).

## Layout

- `cases/<id>/manifest.json` — scenario metadata and expected outcomes
- `cases/<id>/inputs/` — optional seed data (synthetic only)

## Running (Node)

```sh
pnpm test tests/conformance-vectors.test.ts
```

## Case IDs

| ID | Covers |
|---|---|
| `two-tier-wall-reject` | Measured record without `source` must fail validation |
| `rmssd-toy-intervals` | RMSSD from three RR intervals matches reference answer |

Grokbot and the Swift verification app should reuse these IDs when adding UI or
browser scenarios.
