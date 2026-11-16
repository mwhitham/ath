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

## Status of each folder

| Folder | Status |
|---|---|
| [v0.1.0](v0.1.0/spec.md) to [v0.4.0](v0.4.0/spec.md) | Released prerelease formats. 0.x files are refused by 1.0.0 and later. |
| [v1.0.0](v1.0.0/spec.md) | Released. SQLite became the live record ([contribution readiness](v1.0.0/appendix/contribution-readiness.md)). |
| [v2.0.0](v2.0.0/spec.md) | Released. |
| [v2.1.0](v2.1.0/spec.md) | Released. This is the installed software until 2.1.1 is released. |
| [v2.1.1](v2.1.1/spec.md) | Built, not yet released: the complete native rebuild. See its [decisions](v2.1.1/decisions.md), [progress](v2.1.1/progress.md), and [release evidence](v2.1.1/appendix/release-evidence.md). |
| [v2.2.0](v2.2.0/spec.md) | A plan. It is not built. See its [decisions](v2.2.0/decisions.md) and [progress](v2.2.0/progress.md). |

## Drafts that were not approved

A version folder holds only a plan for a version that was approved. A draft
that was replaced before approval goes, unchanged, into the `appendix/` of the
version that replaced it, named `draft-<version>` (D224). Its proposed D-numbers
stay reserved, so they are never reused.

- [The first draft of the 2.1.1 plan](v2.1.1/appendix/draft-2.1.1/spec.md) was
  written as version 3.0.0 and proposed D198–D207. It was not approved. Version
  2.1.1 keeps the same rebuild without its JavaScript removals (D208, D209).

## Why

Specs written in chat threads and planning sessions evaporate. Six months from now, "why is there no MCP server?" or "why TypeScript?" should be answerable by reading this folder, not by archaeology. Each version's `decisions.md` records the alternatives that were considered and rejected, because the rejected options are the part everyone forgets. Each version's `appendix/` holds the long-form research those decisions rest on.
