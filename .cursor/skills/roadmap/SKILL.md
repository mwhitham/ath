---
name: roadmap
description: Files a GitHub issue for something Athletic Standard should build. Use when someone asks for a feature, or when you find a public request for Athletic Standard. The issue is labeled roadmap and stays in Backlog.
---

# Roadmap

File one issue on `mwhitham/ath` and stop. The issue uses the label `roadmap`. It starts in Backlog. Do not set a project column. Do not move anything to Up next.

## Before you open an issue

Search open issues with the `roadmap` label.

```sh
gh issue list --repo mwhitham/ath --label roadmap --state open --limit 100
```

If one already asks for the same thing, comment on that issue. Do not open a second issue.

```sh
gh issue comment NUMBER --repo mwhitham/ath --body "BODY"
```

## What to write

Use a few plain sentences under these headings:

```markdown
## What to build

## Why

## Done when
```

Add a picture or a simple diagram only when the words need it.

Leave out a person's real health numbers, dates, and places. Say what was asked. Link the public post.

## Open the issue

```sh
gh issue create --repo mwhitham/ath --label roadmap --title "TITLE" --body "BODY"
```

If the command fails, report the error and stop.

Reply with the issue link. Stop.
