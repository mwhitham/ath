---
name: provider-period-summary
description: Group one measurement by the provider period stored on the record, such as an Oura day or a WHOOP cycle. Use when asking for this summary across more than one record.
license: MIT
compatibility: Requires the ath CLI and an Athletic Standard 2.0.0 database. The calculation runs with ath apply provider-period-summary or ath calculate provider-period-summary.
metadata:
  version: "2.0.0"
---

# provider-period-summary

Group one measurement by the provider period stored on the record, such as an Oura day or a WHOOP cycle. One request names one measurement, so RMSSD and SDNN stay separate. Personal reports are left out. Devices are not added together.

```ath-calculation
name: provider-period-summary
needs: quantity
needs: period_type
allows: source
allows: from
allows: to
allows: record_kind
per: provider_period
use: count
use: minimum
use: maximum
use: mean
use: sum
method: count, minimum, maximum, mean, and sum per exact provider period and source; no calendar boundary conversion
```
