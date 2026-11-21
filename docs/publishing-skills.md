# Publishing the skills

Nothing in this repository publishes a skill, submits it to skills.sh, or triggers a
listing. These are the manual steps for the maintainer, written down so they can be
checked before anything is public.

## What skills.sh does

skills.sh lists skills from the install telemetry of the `skills` CLI: when someone
runs `npx skills add <owner>/<repo>`, the CLI reports the install to skills.sh, and
the skill appears in its rankings ([FAQ](https://skills.sh/docs/faq)). The CLI skips
that report when `DISABLE_TELEMETRY` or `DO_NOT_TRACK` is set
([CLI docs](https://skills.sh/docs/cli)). There is no submission form.

## Before publishing

1. The release checks pass: `cargo fmt --check`, `cargo clippy --all-targets -- -D
   warnings`, `cargo test`, and the npm checks.
2. The independent coaching reviews listed in
   [`build-history/v2.1.2/progress.md`](../build-history/v2.1.2/progress.md) are
   done and their findings resolved.
3. `skills.sh.json` at the repository root lists every skill once
   (`tests/skill_examples.rs` checks this).
4. The repository is public on its default branch, because skills.sh reads
   `skills.sh.json` from there ([customising a page](https://skills.sh/docs/customize)).
5. The npm package and the release archives for this version are published first.
   Every skill tells the agent to install `athleticstandard` and to need this version
   or later. Until it is published, `npm install -g athleticstandard` installs an
   older version, and the skills tell the agent to stop. Check from a clean folder
   that `npx -y athleticstandard@<version> --version` prints the version.

## Publishing the npm package

The npm package must be public before the skills are listed (step 5 above). No
terminal is needed (D251, D252).

1. Merge the pull request that sets the new version in
   `packages/npm/package.json`. A comment on it then says the version is ready.
2. Comment `/release` on that pull request. Only a repository admin's comment
   starts anything. Run workflow on `main` with `publish` ticked does the same, on
   the website or in GitHub Mobile.
3. The release run does the full check, tags the tested commit, stages the tarball
   on npm, and creates the GitHub release with the native archives. It stores no
   npm token: it signs in through npm's trusted publishing.
4. When it finishes, a comment on the pull request says how to approve. Approve the
   staged version on npmjs.com, or run `npm stage list athleticstandard` and
   `npm stage approve <stage-id>`. npm asks for your two-factor code, then the
   version is public.
5. Check from a clean folder: `npx -y athleticstandard@<version> --version`.

Pushing a version tag, such as `v2.1.2`, from a terminal still starts the same
release.

The trust link on npmjs.com (package settings, Trusted Publisher) names GitHub
Actions, `mwhitham/ath`, `release-cross-platform.yml`, and the `release` environment,
and allows staging only. npm expires a new link that is not used within 2 days. If
it expires, delete it, create it again, and release within 2 days.

## Listing the skills

1. Merge to the default branch.
2. Install once from the public repository without disabling telemetry, for
   example `npx skills add <owner>/<repo> --skill athletic-standard`. That first
   install is what makes the repository appear.
3. Open the repository's page on skills.sh and check the groups and descriptions.

## Testing without publishing

Install from a local path, with telemetry off and no network route to skills.sh:

```sh
DISABLE_TELEMETRY=1 DO_NOT_TRACK=1 npx skills add ./skills --list
```

The 2.1.2 local installation test is recorded in the
[agent evaluation](../build-history/v2.1.2/appendix/agent-evaluation.md).
