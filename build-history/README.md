# Build History

This folder is the permanent record of what we decided to build and why.

## The rule

Every plan we execute gets its spec written into a versioned subfolder here **before or as** the build happens. Within a version, all decisions go in that version's `decisions.md` — append new D-numbers, don't rewrite earlier ones. Supporting research goes in `appendix/` in the same version folder and is linked from the D-number. If a later build is a new plan, it gets a new version folder; the old one stays as-is.

Folders are named for the **record format version** they describe — the same version
that appears in `athleticstandard_version` on records and packages. The npm package
version can move faster (for example `1.1.0` on npm while the format stays `1.0.0`);
those releases still append decisions under the format folder they change (see D144).
A plan that revises the format gets a folder matching the new format version.

D-numbers run continuously across version folders. D25 follows D24 even though it lives in a different folder, so a decision can be cited by number alone.

## Structure

```
build-history/
  v0.1.0/
    spec.md        # the full build specification for the first version
    decisions.md   # the decision log: what was chosen, what was rejected, and why (D1–D24)
    progress.md    # what's done and what's next
    appendix/      # long-form research those decisions rest on; linked from D-numbers
  v0.2.0/
    spec.md        # the format revision: sidecar series, vendor scores, importers
    decisions.md   # D25 onward
    progress.md
  ...
```

## Current records

- [Version 1.0.0 specification](v1.0.0/spec.md)
- [Version 1.0.0 decisions](v1.0.0/decisions.md)
- [Version 1.0.0 progress](v1.0.0/progress.md)
- [Contribution readiness](v1.0.0/appendix/contribution-readiness.md)
- [Version 2.0.0 specification](v2.0.0/spec.md)

Version 2.0.0 is the software now. The program, the record, and the database
use that number.

## Why

Specs written in chat threads and planning sessions evaporate. Six months from now, "why is there no MCP server?" or "why TypeScript?" should be answerable by reading this folder, not by archaeology. Each version's `decisions.md` records the alternatives that were considered and rejected, because the rejected options are the part everyone forgets. Each version's `appendix/` holds the long-form research those decisions rest on.
