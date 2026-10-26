---
name: strength-volume
description: Adds reps times weight for each set on one workout. Use when asking how much weight was lifted.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply strength-volume --record <record-id>.
metadata:
  version: "2.0.0"
---

# Strength volume

For each set, multiply reps by weight, then add the sets. The result names
the weight unit. A text-only result stays text, and this calculation says
the volume cannot be made.

```ath-calculation
name: strength-volume
field: sets
```
