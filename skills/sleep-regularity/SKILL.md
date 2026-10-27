---
name: sleep-regularity
description: Compares each night of sleep with the next night from the same source. Use when asking how regular sleep is. This needs seven nights.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply sleep-regularity --record <record-id>.
metadata:
  version: "2.0.0"
---

# Sleep regularity

Compare each night's asleep minutes with the next night from the same source.
A minute agrees when both nights are asleep or both are awake. This needs
seven nights. With fewer nights, the result says how many were found and
does not return a number.

```ath-calculation
name: sleep-regularity
use: night_agreement
minimum_nights: 7
```
