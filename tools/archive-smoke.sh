#!/usr/bin/env bash
# Use a released `ath` executable the way a person would, with no Node.js,
# Rust toolchain, sqlite3 program, or source checkout on its PATH
# (spec 2.1.1 section 13.2).
#
# Usage: tools/archive-smoke.sh <ath executable> <fixtures folder>
#
# <fixtures folder> is a copy of tests/fixtures/ from this repository: synthetic
# data only. The script creates a database, logs and corrects a record, imports
# a provider export, calculates, exports, restores the package, migrates a
# 2.0.0 database, repairs the damaged npm 1.0.0 database, and checks the
# results. Every step must exit 0. Run it from an empty working folder.
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <ath executable> <fixtures folder>" >&2
  exit 2
fi

executable_dir="$(cd "$(dirname "$1")" && pwd)"
ath="$executable_dir/$(basename "$1")"
fixtures="$(cd "$2" && pwd)"
# A private folder with no symbolic link in its path; `ath` refuses links.
work="$(mktemp -d "${TMPDIR:-/tmp}/ath-archive-smoke.XXXXXX")"
work="$(cd "$work" && pwd -P)"

# PATH holds the executable's folder and the few text tools this script uses.
# On Linux and macOS those tools are links in one private folder, because
# /usr/bin on macOS also holds sqlite3. Git Bash on Windows keeps /usr/bin,
# which holds neither sqlite3, Node.js, nor a Rust toolchain.
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    export PATH="$executable_dir:/usr/bin"
    ;;
  *)
    mkdir "$work/tools"
    for tool in cp chmod grep sed head; do
      ln -s "$(command -v "$tool")" "$work/tools/$tool"
    done
    export PATH="$executable_dir:$work/tools"
    ;;
esac
for tool in node npm cargo rustc cc gcc clang sqlite3; do
  if command -v "$tool" >/dev/null 2>&1; then
    echo "archive-smoke: $tool is still on PATH at $(command -v "$tool")" >&2
    exit 1
  fi
done

step() {
  echo "== ath $*"
  "$ath" "$@"
}

cd "$work"
step --version
step init -y --no-skill --timezone UTC

note='{
  "record_type": "athlete_report",
  "record_id": "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f01",
  "data": {
    "report_type": "note",
    "reported_at": "2026-01-02T03:04:05.678Z",
    "input_reported_at": {
      "timestamp_missing_reason": "not_provided_by_source",
      "utc_offset_missing_reason": "not_provided_by_source"
    },
    "reported_via": "text",
    "note": "Easy session."
  }
}'
step log "$note" --yes
corrected='{
  "report_type": "note",
  "reported_at": "2026-01-02T03:04:05.678Z",
  "input_reported_at": {
    "timestamp_missing_reason": "not_provided_by_source",
    "utc_offset_missing_reason": "not_provided_by_source"
  },
  "reported_via": "text",
  "note": "Easy session; no soreness."
}'
step correct 01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f01 --revision 1 --data "$corrected" --yes
step records show 01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f01 --history

step import "$fixtures/exports/whoop" --yes
step stats
step calculate summary hrv_rmssd --group day --timezone UTC
step export ./package
step check

step init -y --empty --no-skill --file restored.ath.sqlite
step import ./package --yes --file restored.ath.sqlite
step check --file restored.ath.sqlite

# Migrate a database with both confirmations: --yes and its database ID.
migrate() {
  local preview database_id
  preview="$("$ath" migrate --json --file "$1")"
  database_id="$(printf '%s' "$preview" | grep -o '"database_id": *"[^"]*"' | head -n 1 | sed 's/.*"\([^"]*\)"$/\1/')"
  if [ -z "$database_id" ]; then
    echo "archive-smoke: the migration preview of $1 named no database_id" >&2
    exit 1
  fi
  step migrate --yes --confirm-migration "$database_id" --file "$1"
}

cp "$fixtures/version-2-0-0/database.ath.sqlite" ./older.ath.sqlite
chmod 600 ./older.ath.sqlite
migrate older.ath.sqlite
step repair --dry-run --file older.ath.sqlite
step stats --file older.ath.sqlite

# A confirmed repair. The npm 1.0.0 fixture holds 110 records whose typed
# rows store their JSON in another key order. The repair rebuilds them before
# the migration, which would otherwise rebuild them in its step 2. After the
# migration, check must find the database valid.
cp "$fixtures/npm-100-encoding/athlete.ath.sqlite" ./encoded.ath.sqlite
chmod 600 ./encoded.ath.sqlite
if ! "$ath" repair --json --file encoded.ath.sqlite | grep -q '"repair_candidates": 110,'; then
  echo "archive-smoke: the repair preview did not find the 110 damaged records" >&2
  exit 1
fi
repaired="$(step repair --yes --file encoded.ath.sqlite)"
printf '%s\n' "$repaired"
if ! printf '%s' "$repaired" | grep -q 'repaired 110 record(s); database revision .* with verified backup at '; then
  echo "archive-smoke: the confirmed repair did not repair the 110 records" >&2
  exit 1
fi
if ! "$ath" repair --json --file encoded.ath.sqlite | grep -q '"repair_candidates": 0,'; then
  echo "archive-smoke: records still need repair after the confirmed repair" >&2
  exit 1
fi
migrate encoded.ath.sqlite
checked="$(step check --json --file encoded.ath.sqlite)"
printf '%s\n' "$checked"
if ! printf '%s' "$checked" | grep -q '"valid": true'; then
  echo "archive-smoke: check did not find the repaired database valid" >&2
  exit 1
fi

echo "archive-smoke: every step passed in $work"
