---
name: sleep-timing
description: Reads the start and midpoint of one sleep record. Use when asking when sleep started.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply sleep-timing --record <record-id>.
metadata:
  version: "2.0.0"
---

# Sleep timing

The start time and the midpoint of the sleep on this record.

```ath-calculation
name: sleep-timing
start_field: start_at
end_field: end_at
```
