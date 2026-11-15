# Contributing

Use a feature branch and open a pull request against `main`. Keep each commit to one logical change.

## Roadmap

Use the Roadmap idea issue form for something to build. The issue is labeled `roadmap` and starts in Backlog. The Roadmap workflow places it on the board, and moves it to Done when the issue closes. The repository owner moves work to Up next. Build from Up next.

The public board is the project titled Roadmap: <https://github.com/users/mwhitham/projects/3>.

Do not put real health numbers, dates, or places in an issue. Describe the request. A public issue keeps whatever you write.

## Protect health data

Never commit a real health export or athlete file. This rule applies to public and private repositories because clones, logs, and old commits retain deleted data.

Keep real validation data in encrypted storage outside Git. Public bug reports, tests, and decisions must use the smallest invented example that proves the behavior. Remove exact workout dates, locations, birth dates, travel clues, device inventories, and sample values.

Files under `tests/fixtures/exports/` and `examples/demo-athlete/` are synthetic. Adding one requires an explicit entry in the allowlist in `scripts/check-public-data.ts`. Health-data files also require an explicit exception in `.gitignore`.

Do not upload real exports to public continuous integration. Run private validation locally and publish only a sanitized conclusion.

## Verify a change

Run:

```sh
pnpm check:public-data
pnpm typecheck
pnpm test
pnpm build
```
