---
name: sleep-efficiency
description: Divides time asleep by time in bed. Use when asking how much of the time in bed was sleep.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply sleep-efficiency --record <record-id>.
metadata:
  version: "2.0.0"
---

# Sleep efficiency

Time asleep divided by time in bed. Both fields are required.

```ath-calculation
name: sleep-efficiency
asleep_field: asleep_seconds
in_bed_field: in_bed_seconds
```
