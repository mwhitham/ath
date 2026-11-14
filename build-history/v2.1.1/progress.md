# v2.1.1 rebuild progress

Status: **complete native rebuild selected; implementation has not started**.

The release target is 2.1.1. The complete specification is [spec.md](spec.md).
[D208–D211](decisions.md) record the target and compatibility requirements.

Completed:

- Recorded 2.1.1 as the selected version for the complete rebuild.
- Retained the full implementation and verification requirements.
- Removed the earlier draft's proposed public API removals from the selected plan.
- Preserved the unapproved 3.0.0 draft as history.

Remaining:

- Implement the complete replacement and remove the duplicated native runtime.
- Verify all existing workflows and public contracts against the fixed reference.
- Meet the recovery, migration, operating-system, packaging, and performance checks.
- Record the results before creating a release.

The installed implementation remains 2.1.0 until the rebuilt 2.1.1 is complete.
No package was published, and no release or tag was created.
