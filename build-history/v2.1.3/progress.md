# v2.1.3 progress

Status: **planned; not built**. The plan is [spec.md](spec.md).

| Step | What | Status |
|---|---|---|
| 1 | Plan, decisions D254–D267, storage study, hosting experiments | done |
| 2 | Version 2.1.3, migration 007, coach authorship | not started |
| 3 | Storage interface with the native backend behind it | not started |
| 4 | libSQL remote backend and the qualification suite | not started |
| 5 | `ath-host` native directory mode: connect, read, write, attachments, refresh, restore | not started |
| 6 | Sign-in and grants | not started |
| 7 | Remote mode with object storage and several host copies | not started |
| 8 | Moves and backups | not started |
| 9 | Clients, starter, `docs/hosting.md`, container and setup guide | not started |
| 10 | Scale and fault runs, release evidence | not started |

The experiments in step 1 ran on 2.1.2 with synthetic data on one machine
([hosting-experiments.md](appendix/hosting-experiments.md)). No live provider, region,
or network latency has been measured yet. Steps 4 and 7 do that.
