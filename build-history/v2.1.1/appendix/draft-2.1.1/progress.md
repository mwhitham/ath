# v3.0.0 rebuild progress

Status: **plan proposed; implementation has not started**.

The complete target and release requirements are in [spec.md](spec.md).
[D198–D207](decisions.md) are proposals. Earlier approved decisions remain in force
until the specification is approved.

Completed planning work:

- Inspected current command handling, public exports, schemas, storage, import
  routes, calculations, file handling, model integration, packaging, and checks.
- Identified overlapping writers and the legacy provider-import intermediate.
- Defined a complete native target, deliberate JavaScript contract changes, data
  migration and rollback behavior, and required removal of the old implementation.
- Defined compatibility, recovery, cross-platform, performance, and maintenance
  checks that must pass before the rebuild can be accepted.

Outstanding work:

- Review and approve the proposed specification and decisions.
- Complete the rebuild and every acceptance requirement in the specification.
- Record measured results and the final review evidence before any release.

No runtime code, published schema, published migration, fixture, or existing
build-history file was changed to prepare this plan. No release or tag was created.
