---
name: sleep-interruptions
description: Reads how many times sleep was interrupted on one record. Use when asking how broken a night of sleep was.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply sleep-interruptions --record <record-id>.
metadata:
  version: "2.0.0"
---

# Sleep interruptions

How many times this record says sleep was interrupted.

```ath-calculation
name: sleep-interruptions
use: field
field: interruptions
```
