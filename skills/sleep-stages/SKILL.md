---
name: sleep-stages
description: Adds the sleep stage minutes the source supplied. Use when asking for deep, light, REM, or awake time on one night.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply sleep-stages --record <record-id>.
metadata:
  version: "2.0.0"
---

# Sleep stages

Add the minutes the source supplied for each stage. Do not invent a stage
that is not on the record.

```ath-calculation
name: sleep-stages
use: sum_fields
field: stage_minutes
```
