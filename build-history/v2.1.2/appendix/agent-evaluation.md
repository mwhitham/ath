# Agent evaluation for 2.1.2

This file records how the 2.1.2 skills change what an agent does (D247). Automated
tests never call a model service; these runs are separate and recorded here.

Status: **in progress**. Results are added as each run completes.

## Method

Each case gives a fresh agent session a realistic request, a folder with a synthetic
athlete database, and one of three skill sets:

- **2.1.2**: the skills in this release.
- **2.1.1**: the skills as shipped in 2.1.1 (`tests/fixtures/calculations-2-1-1` and
  the 2.1.1 tool skill).
- **None**: no skills; only `ath --help`.

The transcript is inspected for the commands the agent ran, the records it proposed
to write, and the claims it made. A case passes only when the commands and the
claims are right, not when the answer merely reads well.

## Cases

Pending.

## Local installation with the skills CLI

Pending.

## Independent coaching review

Not done. Review by qualified CrossFit and HYROX coaches is a release requirement
and is outstanding. No AI review counts as this review.
