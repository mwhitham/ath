---
name: sleep-duration
description: Reads time asleep from one sleep record. Use when asking how long someone slept.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply sleep-duration --record <record-id>.
metadata:
  version: "2.0.0"
---

# Sleep duration

Time asleep, in seconds, from the field this file names.

```ath-calculation
name: sleep-duration
field: asleep_seconds
```
