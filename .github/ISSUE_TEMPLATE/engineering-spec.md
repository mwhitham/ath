---
name: Engineering specification
about: A public-safe, testable handoff for an ATH change
title: ""
labels: ""
assignees: ""
---

## Outcome and approval

What happens today? What should happen instead? Link the public approval, or state
that this is a draft awaiting a decision. A private chat link is not a public spec.

## Scope

- Target public repository and component:
- Included behavior:
- Excluded behavior:
- Approved spec path and relevant D-numbers:
- Open decisions (if none, say none):

## Acceptance criteria

| Input or action | Expected observable result | Verification |
| --- | --- | --- |
| Normal case | | |
| Failure or refusal | | |
| Existing behavior to preserve | | |

## Evidence and implementation boundary

Link public provider documentation at the relevant version when applicable.
Name the affected interfaces, migration/compatibility concerns, and baseline commit.
Use invented examples only. Do not paste private implementation details or exports.

## Handoff

Public code and website work goes to a GrokBot worker. Record its tool and public-safe
run identifier in the PR. Independent Codex review follows implementation; the
maintainer approves merging. Private work stays in its separately authorized repo.
