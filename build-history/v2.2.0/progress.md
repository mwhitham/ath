# v2.2.0 progress

This folder is the plan. The maintainer has not approved it. Nothing in the
program, the record, or the database has changed for 2.2.0.

| Step | What | Status |
|---|---|---|
| 0 | The plan: proposals D254–D271, and the Open Wearables study. The proposals were D198–D215 until 2.1.1 used D208–D226, then D227–D244 until 2.1.2 used D227–D253. Checked against 2.1.1 at `0de71bb` on 15 November 2026: a page is one import preview and one apply, and D264 uses the revision check. D266 through D268 stay. D269 and D270 answer the review at `e145fa6`. D271 withholds a credential-shaped value. The pass after the review at `d636d7a` amends D254, D257, D258, D263, D269, D270, and D271. The pass after the reviews at `3c3f660` amends D255, D257, D258, D269, and D270 and adds no decision number. The pass after the reviews at `57838e1` amends D258, D269, and D270 and adds no decision number. Three questions stay open for the maintainer, in section 10 of the specification: the Garmin detail default, the owner-correction scope, and storing vendor originals in their own files. Still not adopted. The new head needs a fresh review | written, not approved |
| 1 | Version 2.2.0 and migration step 7. Step 5 is the published 2.1.1 migration. Step 6 is the published 2.1.2 migration. No new column on an existing table. The root check gains `source_report` by the copy in D263. Summary and score values may be omitted in pairs (D254, D263) | not started |
| 2 | `ath pull openwearables` in the `athleticstandard` crate, with the rewritten mapping (D255, D257, D263) | not started |
| 3 | Key, server, and user id kept out of the record (D256) | not started |
| 4 | Fixture tests, with no live key in CI | not started |
| 5 | Generated schema, demo athlete, skill command reference, and the public-data list | not started |
| 6 | Laptop card and the proof in the build pull request | not started |

The build, after approval, is one pull request. It is not three.

The Windows failed-init cleanup, the size of `src/import/mapping.rs`, a
way to clear the Garmin key, and a name check outside import stay out of
that pull request (D258). `persist.rs` is already gone.
