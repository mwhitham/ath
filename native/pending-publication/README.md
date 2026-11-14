# Files waiting for the 2.1.1 cutover

These files replace published files at the cutover (spec section 15, step 8).
They live here until then because the 2.1.0 test suite checks the published
copies, and that suite must keep passing while the old implementation remains.

| File here | Replaces at cutover |
|---|---|
| `spec/sql/contract.json` | `spec/sql/contract.json` (SQL schema step 5) |
| `spec/sql/README.md` | `spec/sql/README.md` |

The native library reads the copies here. The cutover moves them into place
and points the library at the published paths.
