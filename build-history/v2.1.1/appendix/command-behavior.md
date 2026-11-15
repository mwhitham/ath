# Command behavior reference (version 2.1.0, reference commit c6218e88)

This file lists the command behavior that `command-inventory.json` cannot hold.
Every row is a promise that the native `ath` executable must keep or change on
purpose (spec 2.1.1 section 4.2). Each row becomes at least one executable-level
test. The JSON inventory holds names, flags, defaults, parsers, help text, and
parse-only probes. This file holds what happens after parsing.

Sources are cited as `file:line` at the reference commit. `program.ts` means
`src/program.ts`; `runtime.ts` means `src/cli/runtime.ts`; `render.ts` means
`src/cli/preview-render.ts`. Rows marked **Quirk** describe observed behavior
that looks unintended. A quirk is recorded so the new executable changes it on
purpose, with a documented reason, rather than by accident (spec section 13.1).

Every command in the inventory appears below: 32 commands, plus the root `ath`.

## 1. Rules shared by every command

| ID | Behavior | Rule | Source |
|----|----------|------|--------|
| S1 | Success exit | A command that finishes without an error exits 0. | `src/cli.ts:7-19` |
| S2 | Error exit | An error that escapes an action prints `ath: <message>` on stderr and exits 1. Zod validation errors are formatted first. | `src/cli.ts:10-15` |
| S3 | Refusal exit | `fail()` prints `ath: <message>` on stderr and throws an exit error with status 1. No command uses another status. | `runtime.ts:23-37` |
| S4 | **Quirk:** refusal printed twice | When `fail()` runs inside an action's `try` block whose `catch` calls `fail()` again, the same `ath: <message>` line is printed twice on stderr. Observed for `log`, `delete`, `erase`, `records show`, and every other action with `catch (error) { return fail(...) }`. The exit status is still 1. | `program.ts:1085` with `program.ts:1101-1102`; `runtime.ts:33-37` |
| S5 | Parse errors | Commander errors print `error: <text>` on stderr and exit 1: unknown option, missing argument, missing required option, missing option value, too many arguments. Exact texts are in the inventory probes. | `command-inventory.json` `probes` |
| S6 | Positive whole numbers | `--revision` and `--limit` use `parsePositiveInteger`. A value that is not an integer of at least 1 prints `ath: expected a positive whole number, not '<value>'` and exits 1. Commander does not catch this error; `src/cli.ts` does. | `program.ts:2417-2423`; `src/cli.ts:14` |
| S7 | Repeatable values | `--attachment` collects every occurrence in order (`collectValues`). `backtest -m/--model` does the same (`collectModels`). Both start from an empty list. | `program.ts:2409-2415` |
| S8 | Standard output and error | Results, previews, prompts, and success messages use stdout (`console.log`, readline output). Refusals, errors, `check` issues, and warnings use stderr (`console.error`, `console.warn`). | `runtime.ts:35`, `runtime.ts:59`, `program.ts:1538-1556` |
| S9 | JSON formatting | Every JSON result is `JSON.stringify(value, null, 2)` followed by one newline. | `program.ts:305-319` and every `--json` branch |
| S10 | Database discovery | Without a path, only `athlete.ath.sqlite` in the working directory is used. No other `*.ath.sqlite` name is discovered. | `src/file.ts:37-40` |
| S11 | Explicit path | A `--file` value or a positional `[file]` is resolved against the working directory. It must exist, must not end in `.ath.json`, and must end in `.ath.sqlite`. Each failure has its own message. | `src/file.ts:20-35` |
| S12 | Missing database | With no default file: if exactly one `*.ath.json` is in the folder, refuse with the prerelease-format message; otherwise refuse with `no athlete.ath.sqlite in <cwd> — run \`ath init\` to create one, or pass --file`. Exit 1. | `src/file.ts:42-54`; `runtime.ts:39-45` |
| S13 | `--file` beats `[file]` | Commands with both (`migrate`, `repair`, `check`, `stats`) use `--file` when both are given. | `program.ts:410`, `program.ts:526`, `program.ts:1515`, `program.ts:2242` |
| S14 | Write-capable open | `openOrFail` opens without migrating. A database that needs a SQL migration is refused with `Database SQL schema <n> must be upgraded to <m> before other commands can run. Run ath migrate.` when `--json` is set or stdin or stdout is not a terminal. | `runtime.ts:167-184`; `src/db/migrate.ts:312-321` |
| S15 | Migration on open | In a terminal without `--json`, `openOrFail` prints the migration preview JSON (S16 fields) on stdout, asks `Create a verified backup and apply this migration? [y] yes  [n] no `, then asks `Type the database ID <id> to confirm migration: `. Both must pass. It then migrates, prints `migrated SQL schema <a> to <b> with verified backup at <path>`, and continues the original command. A refusal says `migration was not confirmed; run ath migrate or answer yes when prompted on open` or `second confirmation did not exactly match the database ID; nothing was migrated`. | `runtime.ts:97-146` |
| S16 | Read-only open | `openReadOnlyOrFail` opens an outdated database without asking to migrate, and skips the WAL checkpoint on release. | `runtime.ts:148-165` |
| S17 | Security warnings | A file-permission warning prints `ath: warning: <text>` on stderr, for example `Sensitive athlete data path is readable by other local users: <dir>`. `--json` suppresses it. | `runtime.ts:152-155`, `runtime.ts:170-173`; `src/db/migrate.ts:397` |
| S18 | First confirmation | `confirmWrite` returns "do not write" without asking when `--json` or `--dry-run` is set. It returns "write" when `--yes` is set. Otherwise, when stdout is not a terminal, it refuses with `there is no terminal to ask on, so nothing was written. Pass --yes after reviewing the preview.` (exit 1). In a terminal it asks `<question> [y] yes  [n] no ` and accepts only `y` or `yes`, in any case. | `runtime.ts:69-86` |
| S19 | Where answers are read | Prompts read stdin when stdin is a terminal, otherwise `/dev/tty`. If neither can be read, the answer is empty, which means no. | `runtime.ts:47-67` |
| S20 | Second confirmations | `migrate` and `erase` need an exact second value. In non-interactive use (stdin or stdout not a terminal) the flag value is required. In a terminal, the command asks for it when the flag is absent. | `program.ts:457-473`, `program.ts:1467-1483` |
| S21 | `--yes` with `--json` | `--json` is checked before `--yes` on every command that previews. The pair returns the preview and writes nothing. Exceptions are listed per command (`init`, `export`, `pull`, `models --default`, `skill install`, `conflicts resolve`, `backtest`). | each command below |
| S22 | Standard input | Only `log` reads standard input, and only when no entry words are given. It reads to end of input as UTF-8. When stdin is a terminal it first prints `Paste the entry, then press Ctrl-D:` on stderr. | `runtime.ts:88-95`; `program.ts:1044` |
| S23 | Progress output | No command draws progress in 2.1.0. Imports receive the silent reporter. The terminal progress bar exists, draws only on stderr, and only when stderr is a terminal, but no command uses it. | `src/progress.ts:9-12`, `src/progress.ts:27-32`, `src/progress.ts:49`; `src/import/index.ts:118-131` |
| S24 | Stale skill notice | A pre-action hook would print one stderr line when an installed skill is older than the shipped one. It skips `init`, `skill`, and `--json` runs. In 2.1.0 `staleSkillWarning` always returns null, so nothing is printed. | `program.ts:170-181`; `src/skill.ts:209-211` |
| S25 | Positional options | Root options (`--update`, `--version`, `--help`) are recognized only before a subcommand. `ath log --update` fails with `error: unknown option '--update'`. | `program.ts:144-146`; inventory probe `["log","--update"]` |
| S26 | Help | `-h, --help` on any command prints its help on stdout and exits 0. The text includes the per-command examples. Exact text is `help_text` in the inventory. | `program.ts:156`, `program.ts:187` and each `addHelpText` |
| S27 | Group without subcommand | `ath conflicts` and `ath records` with no subcommand print their help on stderr and exit 1. | inventory probes `["conflicts"]`, `["records"]` |
| S28 | Implicit help command | Only `conflicts` and `records` have an implicit `help [command]` subcommand. `ath conflicts help show` prints the help for `conflicts show` on stdout and exits 0. `ath help`, `ath skill help`, and `ath key help` are "too many arguments" errors, because those commands have actions. | inventory `implicit_help_command`; probes |
| S29 | Unknown command word | An unknown first word, such as `ath nosuch`, is a "too many arguments" error, not "unknown command", because the root command has an action. Under `records`, an unknown word is `error: unknown command '<word>'`. | probes `["nosuch"]`, `["records","nosuch"]` |
| S30 | Version | `ath --version` and `ath -V` print `2.1.0` and a newline on stdout and exit 0. | `program.ts:151` |
| S31 | Database release | Every opened local database is released when the process finishes, after success or failure. | `src/cli.ts:17-19` |
| S32 | Timestamps and IDs | New record IDs are UUIDv7 values created at run time. A preview's IDs are not reused by a later separate run. Tests must compare structure, not these values. | `program.ts:1062-1075` |

## 2. Root command `ath`

| Behavior | Rule | Source |
|----------|------|--------|
| Bare `ath` | Prints a short guide on stdout and exits 0. The guide names the first commands to run. | `program.ts:157-168` |
| Guide variant | If any file ending in `.ath.sqlite` is in the working directory, the guide names the first one and lists `stats`, `records list`, `log`, `import`, `predict`, and `check`. Otherwise it suggests `init` and `import`. Note that discovery for other commands (S10) uses only `athlete.ath.sqlite`. | `src/help.ts:30-67` |
| `--update` | Finds how ath was installed. npm, pnpm, or bun: read the latest version from the registry, run the matching global install when older, then refresh skills and mappings if a skill copy is present. npx, pnpm dlx, bunx: print that the runner fetches the current version. A checkout or local install: print `ath: <message>` on stderr and exit 1. | `program.ts:157-166`; `src/update.ts:243-294` |
| `--update` exit | A non-zero result from the update calls `process.exit(<code>)`. Registry failures print `ath: the npm registry could not be reached, so nothing was changed.` or `ath: the npm registry did not return a version, so nothing was changed.`; a failed install prints `ath: the install failed.` All exit 1. | `program.ts:164`; `src/update.ts:255-275` |
| `--update` messages | `ath <v> is the current version.`, `This ath is <v>, which is ahead of the published <latest>.`, or `updated ath from <v> to <latest>.` Then the skill refresh line or `left installed skill files unchanged`, then one `added mapping file <path>` per new mapping. | `src/update.ts:266-292` |
| Help footer | `ath --help` ends with the footer text in `HELP_FOOTER`. | `program.ts:156`; `src/help.ts:561-571` |

## 3. Set up

### `init`

| Behavior | Rule | Source |
|----------|------|--------|
| Target file | Writes `--file` (default `athlete.ath.sqlite`) resolved against the working directory. Does not use discovery. | `program.ts:193`, `program.ts:199` |
| Name rule | The path must end in `.ath.sqlite`, else refusal `the live database name must end in .ath.sqlite: <path>`. | `program.ts:200-202` |
| Existing file | Refuses when the file exists, naming `ath stats` and `--file`. Nothing is overwritten. | `program.ts:203-208` |
| Profile prompts | Without `-y` and without `--json`, asks on stdout and reads stdin with readline: `Name (optional): `, `Birth year (optional): `, `Sex, male/female (optional): `, `Default timezone, such as America/Chicago (optional): `. A flag value skips its question. An empty answer means absent. | `program.ts:216-226` |
| **Quirk:** no terminal | Without `-y` or `--json`, when stdin is not a terminal and ends early, the process stops after a prompt with exit 0 and creates no file. With piped answers it was observed to stop after the second prompt, also with exit 0 and no file. There is no non-interactive refusal. | `program.ts:216-226` (observed with `</dev/null` and a pipe) |
| Profile values | `--name` (or the answer) becomes `name` when not empty. `--birth-year` becomes `birth_year` as `Number(<text>)`, with no further check here. `--sex` becomes `sex` only when exactly `male` or `female`; other values are dropped silently. `display_units` is `imperial` only when `--units imperial`; any other value becomes `metric`. `--timezone` becomes `default_time_zone` when given. | `program.ts:210-215`, `program.ts:240-249` |
| Seeded records | Unless `--empty`: one `athlete_profile`, one manual `source` (`data_writer` `ath`, `data_route` `manual_entry`), and one `benchmark_definition` per seed benchmark, all written as tool `ath` at one creation time. | `program.ts:238-284`; `src/benchmarks.ts` |
| `--empty` | Creates the database with no athlete records and revision 0, for a package restore. | `program.ts:238`, `program.ts:309`, `program.ts:323-324` |
| Failure cleanup | If creation or seeding fails, the database and its `-wal` and `-shm` files are removed and the refusal ends with `Nothing was kept.` | `program.ts:285-295` |
| Skill and mappings | Unless `--no-skill`, installs the skill into `.agents/skills` and harness folders already present, and copies missing mapping files into `.agents/mappings`. | `program.ts:300-301` |
| `--json` | A result, not a preview. It also skips the prompts. The database is created. | `program.ts:216`, `program.ts:303-320` |
| Human output | `created <path>`, then the seed or empty line, one line per installed skill path, the mapping count line, a skill note, and `next: \`ath import <export-file>\` to load device data`. | `program.ts:322-340` |
| Exit | 0 on success; 1 on any refusal. | S1, S3 |

JSON fields (`init --json`):

| Field | Meaning |
|-------|---------|
| `created` | Absolute path of the new database. |
| `athleticstandard_version` | Record format version written into the database (`2.1.0`). |
| `database_revision` | Database revision after seeding; 0 with `--empty`. |
| `benchmark_record_ids` | Stable IDs of the seeded benchmark definitions, in seed order. Empty with `--empty`. |
| `skill_installed` | Absolute paths of skill folders written. Empty with `--no-skill`. |
| `mappings_installed` | Absolute paths of mapping files copied. Empty with `--no-skill`. |

### `skill`

| Behavior | Rule | Source |
|----------|------|--------|
| Status | Lists every skill location in the working directory with its installed version or `missing`. Reads only. No database. | `program.ts:349-356`; `program.ts:2480-2499` |
| `--json` | Status as JSON. | `program.ts:351-353` |
| Exit | Always 0 unless an unexpected error occurs. | S1 |

JSON fields (`skill --json`):

| Field | Meaning |
|-------|---------|
| `shipped` | Skill version shipped with this ath. |
| `copies[]` | One entry per skill location. |
| `copies[].path` | Absolute path of the skill folder. |
| `copies[].installed` | Installed version, or null when missing. |
| `copies[].shipped` | Shipped version for that skill. |
| `copies[].state` | `installed` or `missing`. |

### `skill install`

| Behavior | Rule | Source |
|----------|------|--------|
| Writes | Copies the tool skill into `.agents/skills` and each harness folder present, replacing an older tool skill. Calculation skills already installed are kept. Missing mapping files are copied; installed ones are kept. | `program.ts:363-364`; `src/skill.ts:181`, `src/skill.ts:246-258` |
| `--json` | A result after writing. `--json` on either `skill` or `skill install` selects JSON. | `program.ts:365-374` |
| Missing source | Without `--json`, refuses when the shipped skill folder is absent. With `--json` this check is skipped. | `program.ts:376-378` |
| Human output | `added mapping file <path>` per mapping; `left installed skill files unchanged` when nothing was written; else `added skill files into <path>` per folder and `version <v>`. | `program.ts:379-385` |

JSON fields (`skill install --json`): `shipped` (shipped skill version), `skill_installed` (absolute folders written), `mappings_installed` (absolute mapping files written).

### `migrate`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file`, else `[file]`, else discovery (S10–S13). | `program.ts:410` |
| Inspection | Reads metadata with an inspector that does not migrate, then releases it. | `program.ts:414-421` |
| `--json` | Prints the preview and stops. It never migrates, even with `--yes` and `--confirm-migration`. | `program.ts:433-436` |
| Human preview | Prints the same preview JSON on stdout. | `program.ts:437` |
| Not needed | Prints `no migration is required`, exit 0. | `program.ts:438-441` |
| No path | When a migration is needed but no steps exist, refuses with the compatibility text. Exit 1. | `program.ts:442-444` |
| `--dry-run` | `nothing migrated (--dry-run)`, exit 0. | `program.ts:445-448` |
| No `--yes` | `nothing migrated; pass --yes after reviewing this preview`, exit 0. There is no first prompt without `--yes`. | `program.ts:449-452` |
| Second confirmation | With `--yes`: uses `--confirm-migration`; if absent and stdin or stdout is not a terminal, refuses with `second confirmation is required in non-interactive use; pass --confirm-migration <id>`; in a terminal asks `Type the database ID <id> to confirm migration: `. A mismatch refuses with `second confirmation did not exactly match the database ID; nothing was migrated`. | `program.ts:457-473` |
| Apply | Opens with outdated schema allowed, creates a verified backup at `planned_backup_path`, migrates, prints `migrated SQL schema <a> to <b> with verified backup at <path>`. The help states that 2.1.0 needs a `sqlite3` executable for the backup. | `program.ts:474-491`; `src/help.ts:204-205` |
| Exit | 0 for preview, not-needed, dry run, and no `--yes`; 1 for refusals and failures. | S1, S3 |

JSON fields (`migrate --json`, and the human preview):

| Field | Meaning |
|-------|---------|
| `file` | Absolute database path. |
| `migration_needed` | True when the SQL schema step is older than this ath's step. |
| `source_sql_schema_version` | Current SQL migration step of the file. |
| `target_sql_schema_version` | SQL migration step this ath writes (4 in 2.1.0). |
| `database_id` | Database ID; also the required second confirmation. |
| `planned_steps[]` | Ordered steps. Each has `target_sql_schema_version`, `migration_file`, and `description`. |
| `planned_backup_path` | Where the verified backup will be written, or null when no migration is needed or no local file exists. |
| `compatibility_effect` | Plain text describing the effect on direct readers and writers. |
| `write_confirmed` | Always false in a preview. |

### `repair`

| Behavior | Rule | Source |
|----------|------|--------|
| File and open | `--file`, else `[file]`, else discovery. Opens write-capable with an outdated schema allowed; no migration prompt. | `program.ts:526-538` |
| Counts | Counts provider-encoding repair candidates and revision-alignment candidates, and lists the alignment record IDs. | `program.ts:540-546` |
| `--json` | Prints the preview only, even with `--yes` and both confirmation flags. | `program.ts:554-557` |
| Human preview | Prints the same JSON, then one of: `no provider-encoding repair is required`; `nothing repaired (--dry-run)`; `nothing repaired; pass --yes after reviewing this preview`. All exit 0. | `program.ts:558-570` |
| Alignment rule | With `--yes`, if alignment candidates exist and `--from-revision` is absent, refuse `<n> record(s) need --from-revision and --confirm-revision-align <id>`. With `--from-revision` and candidates, `--confirm-revision-align` is required and must equal the database ID exactly. There is no interactive prompt for it. | `program.ts:571-588` |
| Apply | Runs the repair with a new verified backup. Prints `repaired <n> record(s); database revision <r>` plus `; aligned <n> record(s) from revision` and ` with verified backup at <path>` when they apply. | `program.ts:589-604` |
| Exit | 0 for previews and success; 1 for refusals and failures. | S1, S3 |

JSON fields (`repair --json`): `file` (database path); `repair_candidates` (records whose typed-table JSON bytes differ only in encoding); `revision_align_candidates` (records whose typed tables disagree in meaning with the current revision); `revision_align_record_ids` (their IDs); `write_confirmed` (always false).

### `key`

| Behavior | Rule | Source |
|----------|------|--------|
| Status | Shows which gateways have a saved key and which have an environment key. The key itself is never printed. No database. | `program.ts:1572-1580`; `src/keyring.ts:234` |
| `--json` | Status as JSON. | `program.ts:1576-1579` |
| Refusal | A password-store refusal prints `ath: <message>`, exit 1. | `program.ts:1581-1583` |

JSON fields (`key --json`): `saved` (gateway names with a key in the OS password store: `vercel`, `openrouter`); `environment` (gateway names with a key in `AI_GATEWAY_API_KEY` or `VERCEL_AI_GATEWAY_API_KEY`, or `OPENROUTER_API_KEY`).

### `key set`

| Behavior | Rule | Source |
|----------|------|--------|
| Gateway | `vercel` or `openrouter`. Any other value refuses with `unknown gateway '<v>'. Use vercel or openrouter.` | `program.ts:1602`; `program.ts:2458-2461` |
| Undocumented `garmin` | `ath key set garmin` saves the Garmin personal API key under account `garmin` with the prompt `Paste your Garmin personal API key: `. The argument help says only vercel or openrouter. | `program.ts:1592-1601` |
| Hidden input | Writes the prompt on stdout and reads stdin in raw mode with echo off. Enter or Ctrl-D ends input; Ctrl-C refuses with `nothing was saved.` | `src/keyring.ts:74-121` |
| No terminal | When stdin is not a terminal, refuses: `there is no terminal to paste a key into. Run this somewhere you can type, or set AI_GATEWAY_API_KEY / OPENROUTER_API_KEY in the environment.` | `src/keyring.ts:76-81` |
| Empty key | Refuses with `no key was entered, so nothing was saved.` | `src/keyring.ts:148-151` |
| No store | Refuses when the OS password store is missing or refuses. It never writes a plaintext file. | `src/keyring.ts:152-160` |
| Success | `saved a <gateway> key in the password store. The key is not printed.` (or `saved a garmin key ...`). Exit 0. | `program.ts:1595`, `program.ts:1605` |

### `key clear`

| Behavior | Rule | Source |
|----------|------|--------|
| Target | With a gateway, clears that one. Without, clears both `vercel` and `openrouter`. `garmin` is refused as an unknown gateway, so the Garmin key cannot be cleared here. | `program.ts:1617-1619`; `src/keyring.ts:163-173` |
| Output | `no gateway key was saved.` or `cleared <a> and <b> from the password store.` Exit 0. | `program.ts:1620-1621` |

### `models`

| Behavior | Rule | Source |
|----------|------|--------|
| Network | Fetches the live list of reasoning text models from the chosen gateway every time. Needs a key from the environment or the password store. | `program.ts:1643`; `src/models.ts:143-150` |
| Gateway choice | `--gateway` must be `vercel` or `openrouter`. Without it, a single available gateway is used; when both have keys, refuse `both vercel and openrouter keys are available. Pass --gateway vercel or --gateway openrouter.` | `program.ts:1642`; `src/keyring.ts:213-228` |
| `--default` | Requires a database path (discovery or `--file`) but does not open it. The model must be on the live list (exact, then case-insensitive). Writes `<name>.ath.model` beside `<name>.ath.sqlite` with the model ID and a newline. Unknown names refuse with the closest IDs. | `program.ts:1644-1653`; `src/models.ts:27-33`, `src/models.ts:136-141`, `src/models.ts:171-182` |
| `--json` | With `--default`, a result after writing. Without, the list. | `program.ts:1648-1651`, `program.ts:1655-1658` |
| Exit | 1 for a key or model refusal; 0 otherwise. | `program.ts:1660-1663` |

JSON fields: list form `gateway` (gateway used) and `models[]` (`id`, optional `name`). Default form `default` (saved model ID), `file` (database path the default sits beside), `gateway`.

## 4. Get your data in

### `import`

| Behavior | Rule | Source |
|----------|------|--------|
| Target database | `--file` or discovery only. The positional `<path>` is the input, resolved against the working directory. | `program.ts:630-631` |
| Route order | 1) `--mapping` given: mapped JSON import. 2) Garmin or Strava detected: shared import. 3) Not a package path: Apple Health, WHOOP, or Oura provider import. 4) Otherwise: package import. | `program.ts:632`, `program.ts:664`, `program.ts:699`, `program.ts:737` |
| Package path test | A path ending in `.json` (any case) or a directory holding `athlete.ath.json` is a package, once Garmin and Strava detection has not matched. | `program.ts:2451-2456` |
| Shared detection | A symbolic link is never detected. A `.gpx` or `.fit` file is Garmin. A `.json` file whose first 8,000 characters contain `activityName`, `summarizedActivities`, or `startTimeGMT` is Garmin. A directory or `.zip` with `export.xml` or `apple_health_export/` is not shared (it is Apple). One with `activities.csv` is Strava. One with `.gpx`, `.fit`, or `summarizedactivities.json` is Garmin. | `src/shared-import.ts:7-62` |
| Mapping file | `--mapping` with a path separator or drive letter is a path and must exist. A name reads `.agents/mappings/<name>.md` in the working directory first, then the shipped `mappings/<name>.md`. Otherwise refuse `no mapping file named <name>`. | `program.ts:633-638`; `src/skill.ts:218-240` |
| Migration check | Mapped and shared imports open the database with S14/S15 rules and release it before the Rust step runs. | `program.ts:639-640`, `program.ts:666-667` |
| Mapped preview | The step runs with `--preview` when `--json` is set or `--yes` is absent. The Rust step runs the writes and rolls them back. | `program.ts:641-652`; `crates/ath-core/src/mapping.rs:876-880` |
| Mapped `--json` | Prints the result with `write_confirmed: false`. With `--json`, `--yes` is ignored and nothing is written. | `program.ts:653-656` |
| Mapped human | Prints the mapping, understood fields, preserved fields, one `left out:` line each, `outcome: <n> new, <n> changed, <n> unchanged`, and a record count. A preview adds `nothing written; pass --yes after reviewing this preview`. With `--yes` the same summary describes the write. | `program.ts:657-658`; `src/shared-import.ts:84-104` |
| Shared preview | Garmin and Strava always run a preview first. `--json` prints it with `write_confirmed: false` and stops. Without `--yes` the human summary ends with `nothing written; pass --yes after reviewing this preview`. | `program.ts:668-686`; `src/shared-import.ts:64-81` |
| Shared write | With `--yes` and no `--json`, runs again without `--preview` and prints `imported <garmin\|strava>: <compact JSON result>`. There is no conflict check in this route. | `program.ts:687-690` |
| Provider preview | Apple Health, WHOOP, Oura: builds the provider preview. `--json` prints it and stops. Human output is the provider summary, then `nothing written; pass --yes after reviewing this preview` without `--yes`. | `program.ts:700-721`; `render.ts:43-87` |
| Provider conflicts | With `--yes`, any conflict refuses `import has <n> unresolved conflict(s); nothing was written`. | `program.ts:722-727` |
| Provider write | Prints `imported <label>: <n> new, <n> changed, <n> matching at database revision <r>`. | `program.ts:728-734` |
| Package preview | Loads the package and builds the restore or merge preview. Human output is the package summary (`render.ts:3-41`). | `program.ts:737-738`, `program.ts:766` |
| Package `--json` side effect | With `--json`, a preview with conflicts saves the temporary conflict file `<database>.import-conflicts.json`; a preview without conflicts removes it. Athlete data is not changed. | `program.ts:757-764`; `src/db/conflicts.ts:42-47` |
| Package without `--yes` | Saves or clears the conflict file the same way. When saved, prints `review with ath conflicts show, then record each choice with ath conflicts resolve`. Always ends `nothing written; pass --yes after reviewing this preview`. | `program.ts:767-780` |
| Saved choices are reset | Saving a new conflict file writes the conflicts without resolutions. Re-running the preview after `conflicts resolve` discards the recorded choices. | `src/db/conflicts.ts:70-99` |
| Package with `--yes` | With conflicts: reads the conflict file, refuses if the package or database changed (`the package or database changed after the conflict preview; run ath import again`), and refuses `import has <n> unresolved conflict(s); use ath conflicts show and ath conflicts resolve` when any lacks a choice. Then imports, clears the conflict file, and prints `restored package at database revision <r>` or `imported package at database revision <r>`. | `program.ts:781-799`; `src/db/conflicts.ts:134-165` |
| Unrepresentable package | Unknown fields or records 2.1.0 cannot represent produce a preview-shaped result with empty record lists and exit **0**. Human output adds `nothing written; every unknown field and unrepresentable record must be resolved`. No flag discards them. | `program.ts:801-836` |
| Exit | 0 for every preview and success, including the unrepresentable case; 1 for refusals and errors. | S1, S3 |

JSON fields, mapped route (`import <file> --mapping <m> --json`):

| Field | Meaning |
|-------|---------|
| `mapping` | Mapping name from the mapping file. |
| `record_ids` | IDs of records the run created or touched. In a preview these come from the rolled-back run. |
| `outcome.new` | Records that would be or were created. |
| `outcome.changed` | Existing records that would get or got a new revision. |
| `outcome.unchanged` | Existing records that already matched. |
| `understood` | Input fields mapped to typed fields, sorted and unique. |
| `preserved` | Input fields kept under their supplied names. |
| `left_out` | One text line per object left out, with its reason. |
| `write_confirmed` | Always false in this JSON. |

JSON fields, shared route (`import <garmin-or-strava> --json`):

| Field | Meaning |
|-------|---------|
| `source` | `garmin` or `strava`. |
| `record_ids` | IDs written. A preview writes nothing, so the list is empty. |
| `understood` | Typed fields read, sorted and unique. |
| `preserved` | Fields kept under supplied names, sorted and unique. |
| `left_out` | One text line per file or row left out, with its reason. |
| `write_confirmed` | Always false. |

JSON fields, provider route (Apple Health, WHOOP, Oura):

| Field | Meaning |
|-------|---------|
| `provider` | `apple`, `whoop`, or `oura`. |
| `imported` | Human label of what was read. |
| `destination_database_revision` | Database revision the preview was built against. |
| `new_records[]` | `record_id` and `record_type` for each record that would be created. |
| `matching_records[]` | `record_id` and `record_type` for each existing record that already matches. |
| `changed_records[]` | `record_id`, `record_type`, and `current_revision` for each record that would get a new revision. |
| `conflicts[]` | `match_key`, optional `record_id`, and `message` for each ambiguous changed duplicate group. |
| `skipped` | Object keyed by reason; each value has `count` and optional `example`. |
| `route_points_stored` | Total route points in the planned records. |
| `write_confirmed` | Always false. |

JSON fields, package route (also the unrepresentable result):

| Field | Meaning |
|-------|---------|
| `source_database_id` | Database ID in the package; `unavailable` when it could not be read. |
| `source_database_revision` | Package database revision; 0 when unavailable. |
| `destination_database_id` | Local database ID. |
| `destination_database_revision` | Local database revision. |
| `restore` | True when the destination is empty (revision 0, no records) and adopts the package history. False for a merge. |
| `new_records` | IDs of records the import adds; first 1,000. |
| `new_record_count` | Full count of new records. |
| `matching_records` | IDs already identical locally; first 1,000. |
| `matching_record_count` | Full count. |
| `changed_records` | IDs that gain later revisions; first 1,000. |
| `changed_record_count` | Full count. |
| `record_lists_truncated` | True when any ID list was cut at 1,000. |
| `conflicts[]` | Optional `record_id` and `message` per conflict. |
| `unrepresentable_records` | `<path>: <reason>` for records 2.1.0 cannot represent; first 1,000. |
| `unknown_fields` | Paths of fields the format does not know; first 1,000. |
| `write_confirmed` | Always false. |

### `conflicts`

| Behavior | Rule | Source |
|----------|------|--------|
| No action | Group command. Without a subcommand it prints help on stderr and exits 1 (S27). | `program.ts:843-847` |

### `conflicts show`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Read-only open (S16). | `program.ts:859-860` |
| Staleness | Reads the conflict file, reloads the package from its saved `source_path`, rebuilds the preview, and refuses if the package or database changed. | `program.ts:862-865` |
| Filter | With `[record-id]`, shows only that conflict; an ID not in the set refuses `record is not in the temporary conflict set: <id>`. | `program.ts:866-874` |
| Output | Always JSON on stdout. `--json` changes nothing except suppressing warnings. | `program.ts:887-893` |
| **Quirk:** missing conflict file | With no conflict file, 2.1.0 prints a raw `ENOENT ... lstat '<database>.import-conflicts.json'` message, not the intended `no temporary import conflicts; run ath import <package> to create a preview`. Exit 1. | `src/db/conflicts.ts:103-114` (observed) |

JSON fields (`conflicts show`):

| Field | Meaning |
|-------|---------|
| `source_path` | Package path the conflicts came from. |
| `source_database_revision` | Package database revision at preview time. |
| `destination_database_revision` | Local revision at preview time. |
| `conflicts[].record_id` | Conflicting stable record ID. |
| `conflicts[].message` | Why it conflicts. |
| `conflicts[].local` | Complete current local record, or null. |
| `conflicts[].imported` | Complete record from the package. |
| `conflicts[].resolution` | Recorded choice (`{choice: local}`, `{choice: imported}`, or `{choice: corrected, data}`), or null. |

### `conflicts resolve`

| Behavior | Rule | Source |
|----------|------|--------|
| Choice | `--choice` is required and must be `local`, `imported`, or `corrected`. `--data` is required with `corrected` and refused otherwise. `--data` must be valid JSON. These checks run before any file is found. | `program.ts:918-934` |
| File | `--file` or discovery. Write-capable open (S14, S15). | `program.ts:935-936` |
| Staleness | Same check as `conflicts show`. | `program.ts:938-941` |
| Write | Records the choice in the temporary conflict file only. Athlete data does not change. There is no preview or confirmation. | `program.ts:949-954` |
| `--json` | A result after recording the choice. | `program.ts:964-965` |
| Human output | `recorded <choice> for <id>; <n> unresolved conflict(s)` and `nothing written to athlete data; run ath import <source_path> --yes after all conflicts are resolved`. | `program.ts:967-973` |

JSON fields (`conflicts resolve --json`): `record_id` (record resolved); `choice` (`local`, `imported`, or `corrected`); `unresolved_conflicts` (conflicts still without a choice); `athlete_data_changed` (always false).

### `export`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Read-only open (S16). | `program.ts:995-996` |
| Directory | `<directory>` is resolved against the working directory and must not exist. A symbolic link is refused. | `program.ts:1000`; `src/db/export.ts:425-427` |
| Determinism | Without `--exported-at` the manifest has no time, so two exports of one revision are byte-identical. `--exported-at` adds that exact UTC time. | `program.ts:1001-1003`; `src/help.ts:236-238` |
| `--json` | A result after writing. | `program.ts:1005-1008` |
| Human output | `exported database revision <r> to <dir>`, then `  athlete.ath.json` and `  export-manifest.json`. | `program.ts:1009-1013` |

JSON fields (`export --json`):

| Field | Meaning |
|-------|---------|
| `directory` | Absolute package directory. |
| `database_id` | Exported database ID. |
| `database_revision` | Exported database revision. |
| `manifest.athleticstandard_version` | Package format version. |
| `manifest.source_database_id` | Database ID. |
| `manifest.database_revision` | Database revision. |
| `manifest.files[]` | `file`, `sha256`, `byte_count` for each package file. |
| `manifest.record_counts[]` | Per `record_type`: `current_record_count`, `revision_count`, `deletion_count`, `import_key_count`. |
| `manifest.erasure_marker_count` | Number of value-free erasure markers. |
| `manifest.exported_at` | Present only with `--exported-at`. |

### `log`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Write-capable open (S14, S15). | `program.ts:1041-1042` |
| Input | Entry words are joined with single spaces. With no words, the entry is read from standard input (S22). | `program.ts:1044` |
| Author | `athlete` by default; `--agent <name>` makes the author an agent with that name. | `program.ts:1045-1048` |
| Plan | Plain text becomes one record. A JSON object or array is a structured batch validated as one unit. `--at` sets an exact UTC event time; `--benchmark` names the workout. | `program.ts:1049-1054`; `src/help.ts:240-271` |
| Attachments | Each `--attachment` file is prepared and linked to the one athlete report or nutrition entry in the batch. | `program.ts:1055-1061` |
| `--json` | Prints the preview and stops. `--yes` beside it writes nothing. | `program.ts:1076-1079` |
| Human preview | `understood: <interpretation>`, `records to write: <n>`, then each ID, type, and indented data and extensions. | `program.ts:1080`; `render.ts:89-111` |
| `--dry-run` | `nothing written (--dry-run)`, exit 0. | `program.ts:1081-1084` |
| Confirmation | S18 with `Write these records?`. A "no" prints `nothing written`, exit 0. No terminal and no `--yes`: refusal, exit 1. | `program.ts:1085-1088` |
| Write | One transaction. Prints `wrote <n> record(s) at database revision <r>` and one `  <id>  <type>` line per record. | `program.ts:1089-1100` |

JSON fields (`log --json`):

| Field | Meaning |
|-------|---------|
| `file` | Database path. |
| `interpretation` | How the entry was understood, such as `athlete note`. |
| `records[].record_id` | Stable ID the write would use in this run. |
| `records[].record_type` | Record type. |
| `records[].created_at` | Creation time the write would use. |
| `records[].data` | Complete record data. |
| `records[].extensions` | Present only when the record has extensions. |
| `write_confirmed` | Always false. |

### `link`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Write-capable open. | `program.ts:1125-1126` |
| Checks | `<result-id>` must be a current benchmark result and `<session-id>` a current workout session, else refuse `<id> is not a current benchmark result` or `... workout session`. | `program.ts:1128-1135` |
| `--json` | Prints the preview and stops. | `program.ts:1145-1148` |
| Human preview | `result <id> revision <n>` and `  workout session: <old or none> → <new>`. | `program.ts:1149-1152` |
| `--dry-run` | `nothing written (--dry-run)`. | `program.ts:1153-1156` |
| Confirmation | S18 with `Write this link as a correction?`. | `program.ts:1157-1160` |
| Write | A correction of the result by the athlete, with `--revision` as the expected revision. A stale revision is refused. Prints `linked <result> to <session> at revision <n>`. | `program.ts:1161-1169` |

JSON fields (`link --json`): `file`; `record_id` (result ID); `expected_revision` (from `--revision`); `current_revision` (result's current revision); `old_workout_session_id` (current link or null); `new_workout_session_id` (session ID); `write_confirmed` (false).

### `correct`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Write-capable open. | `program.ts:1209-1210` |
| Target | Must be a current, not deleted, record: else `current record not found: <id>`. | `program.ts:1212-1215` |
| Data | `--data` must be valid JSON: else `--data is not valid JSON: <reason>`. It replaces the complete data object. | `program.ts:1216-1221` |
| Author | `athlete`, or agent with `--agent`. | `program.ts:1222-1225` |
| Attachments | Allowed only on `athlete_report` and `nutrition_entry`, and only when `--data` is an object. New attachment IDs are appended to `attachment_ids`. | `program.ts:1226-1261` |
| `--json` | Prints the preview and stops. | `program.ts:1276-1279` |
| Human preview | Prints the same JSON. | `program.ts:1280` |
| `--dry-run` | `nothing written (--dry-run)`. | `program.ts:1281-1284` |
| Confirmation | S18 with `Write this correction?`. | `program.ts:1285-1288` |
| Write | Correction with the expected revision; with attachments, the attachment records and the correction are one database revision. Prints `corrected <id> to revision <n> (database revision <r>)`. | `program.ts:1289-1327` |

JSON fields (`correct --json`): `file`; `record_id`; `record_type`; `expected_revision` (from `--revision`); `current_revision`; `before` (current data); `after` (data that would be written, with attachment IDs added); `attachments[]` (`record_id` and `data` of each new attachment record); `write_confirmed` (false).

### `delete`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Write-capable open. | `program.ts:1356-1357` |
| Target | Must be current and not deleted. | `program.ts:1359-1362` |
| `--json` | Prints the preview and stops. | `program.ts:1372-1375` |
| Human preview | Prints the same JSON. | `program.ts:1376` |
| `--dry-run` | `nothing written (--dry-run)`. | `program.ts:1377-1380` |
| Confirmation | S18 with `Delete this record from current views?`. | `program.ts:1381-1384` |
| Write | Normal deletion by the athlete with the expected revision. History is kept. Prints `deleted <id> at database revision <r>; history retained`. | `program.ts:1385-1392` |

JSON fields (`delete --json`): `file`; `record_id`; `record_type`; `expected_revision`; `current_revision`; `retained_history` (always true); `write_confirmed` (false).

### `erase`

| Behavior | Rule | Source |
|----------|------|--------|
| Reason | `--reason` (default `athlete_request`) must be one of `athlete_request`, `legal_requirement`, `recorded_in_error`, `other`, checked before any file is found. | `program.ts:1407-1411`, `program.ts:1429-1434` |
| File | `--file` or discovery. Write-capable open. | `program.ts:1435-1436` |
| Preview | Lists the root record, every dependent record, and each attachment file. A stale `--revision` is refused. | `program.ts:1438-1449` |
| `--json` | Prints the preview and stops, even with `--yes` and `--confirm-erasure`. | `program.ts:1450-1453` |
| Human preview | Prints the same JSON, then `WARNING: this permanently removes <n> record(s) and cannot be undone from the database.` on stdout. | `program.ts:1454-1458` |
| `--dry-run` | `nothing erased (--dry-run)`. | `program.ts:1459-1462` |
| First confirmation | S18 with `Permanently erase these records?`; "no" prints `nothing erased`. | `program.ts:1463-1466` |
| Second confirmation | S20. Non-interactive without the flag: `second confirmation is required in non-interactive use; pass --confirm-erasure <id>`. Terminal: `Type the record ID <id> to confirm permanent erasure: `. Mismatch: `second confirmation did not exactly match the record ID; nothing was erased`. | `program.ts:1467-1483` |
| Write | Prints `erased <n> record(s) and <m> attachment file(s) at database revision <r>`. | `program.ts:1484-1494` |

JSON fields (`erase --json`):

| Field | Meaning |
|-------|---------|
| `file` | Database path. |
| `root_record_id` | Record named on the command line; also the second confirmation. |
| `expected_revision` | Revision from `--revision`. |
| `database_revision` | Database revision the preview was built against. |
| `permanent` | Always true. |
| `records[]` | `record_id`, `record_type`, `current_revision` of each record that would be erased. |
| `attachments[]` | `record_id`, `path`, `present` (file exists) for each attachment file that would be removed. |
| `reason_code` | Erasure reason. |
| `write_confirmed` | Always false. |

### `pull`

| Behavior | Rule | Source |
|----------|------|--------|
| Source | `<source>` must be `garmin`, else `pull currently reads garmin`. | `program.ts:2032` |
| Key | `GARMIN_PERSONAL_API_KEY` from the environment wins, then the password store account `garmin`. Missing: `no Garmin key is saved. Run ath key set garmin, or set GARMIN_PERSONAL_API_KEY.` | `program.ts:2034-2037`; `src/keyring.ts:139-145` |
| Migration check | Opens with S14/S15 rules and releases before the Rust step. | `program.ts:2038-2039` |
| Network | Calls the Garmin personal API for one `--date` day. `--base-url` replaces the API root, for a test server. The key is passed to the child step through an environment variable, not an argument. | `program.ts:2041-2051`; `crates/ath-core/src/main.rs:172-180` |
| Write | Writes immediately. There is no preview, `--dry-run`, `--yes`, or confirmation. `--json` is a result. | `program.ts:2056` |
| Key protection | A result containing the key refuses `refusing to print a result that contains the Garmin key`; an error containing the key becomes `Garmin personal connection failed`. | `program.ts:2052-2063` |
| Human output | `pulled garmin for <date>`. | `program.ts:2056` |

JSON fields (`pull garmin --json`): `source` (`garmin_personal`); `record_ids` (records written); `understood` (typed fields read); `preserved` (fields kept under supplied names).

## 5. Read your data

### `check`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file`, else `[file]`, else discovery. Read-only open. | `program.ts:1515-1516` |
| Checks | SQLite integrity, schema versions, foreign keys, domain rules, relationships, revision chains, session order, attachments. | `program.ts:1518`; `src/help.ts:123-126` |
| `--validate-export` | Also writes a complete package into a new temporary directory, validates it, and removes the directory. | `program.ts:1519-1528` |
| `--json` | Prints the result. Exit status is still 1 when the database is invalid. | `program.ts:1529-1536`, `program.ts:1558-1560` |
| Human issues | Each issue prints `<severity>  <check>[ <record_id>]: <message>`. Errors go to stderr with `console.error`; warnings to stderr with `console.warn`. | `program.ts:1538-1543` |
| Valid | stdout: `<path>: valid Athletic Standard <version> database (<n> records)`, plus `ok  export: complete JSON package validated` when exported. | `program.ts:1544-1551` |
| Invalid | stderr: a blank line, then `<path>: INVALID — <n> error(s)`. Exit 1. | `program.ts:1552-1556`, `program.ts:1558-1560` |
| Errors | `check` has no local catch, so an open or check failure is printed once by `src/cli.ts`. | `program.ts:1517-1563`; `src/cli.ts:14` |

JSON fields (`check --json`):

| Field | Meaning |
|-------|---------|
| `file` | Database path. |
| `metadata` | `application_marker` (`ATH1`), `sql_schema_version`, `athleticstandard_version`, `database_id`, `created_at`; null when unreadable. |
| `valid` | True when no issue has severity `error`. |
| `record_count` | Number of current records. |
| `issues[]` | `severity` (`error` or `warning`), `check` (rule name), `message`, optional `record_id`. |
| `export_validated` | True only when `--validate-export` ran and passed. |

### `apply`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. The TypeScript layer does not open the database, so there is no migration check. | `program.ts:2000` |
| Calculation file | Only the last path part of `<calculation>` is used, without `.md`. `.agents/skills/<name>/SKILL.md` in the working directory wins, then the shipped skill. Missing: `no calculation skill named <name>`. | `program.ts:2002`; `src/skill.ts:115-124` |
| Run | Runs the calculation on `--record` and related same-source records. Writes nothing. | `program.ts:2003-2011`; `crates/ath-core/src/main.rs:85-106` |
| Output | `--json` prints the result. Human output is `<calculation> on <record>`, one `read <field>: present\|missing` line each, `missing: ...` when any, then the compact result JSON. | `program.ts:2012`; `program.ts:2463-2478` |

JSON fields (`apply --json`): `calculation` (skill name); `record_id`; `fields_read[]` (`field` and `present`); `missing` (fields the calculation needed but did not find); `result` (calculation-specific value, such as zone seconds or a bpm difference; null when inputs are missing).

### `calculate`

| Behavior | Rule | Source |
|----------|------|--------|
| Validation | Before finding the file: `<operation>` must be one of `summary`, `baseline`, `trend`, `coverage`, `compare-sources`, `correlation`, `workout-drift`, `provider-period-summary`; `--group` one of `none`, `day`, `week`, `month`; `--record-kind` one of `measurement`, `provider-score`, `provider-summary`, `session-summary`; `--period-type` a provider period type. Each failure has its own message. | `program.ts:2110-2133` |
| File | `--file` or discovery. Read-only open. Writes nothing. | `program.ts:2134-2135` |
| Option mapping | Each option is passed only when given: `--source` → `source_id`, `--compare-source` → `compare_source_id` (compare-sources, correlation), `--other-quantity` → `other_quantity` (correlation), `--session` → `workout_session_id` (workout-drift), `--from` and `--to` → a calendar day or exact UTC time, `--timezone` → `time_zone`. `--group` defaults to `none` and `--record-kind` to `measurement`. Which options each operation requires is enforced by the calculation, not by the argument parser. | `program.ts:2137-2159` |
| Rules | Manual sources never enter device calculations. Different sources, RMSSD and SDNN, temperature types, and distance types are never pooled. A provider period is never turned into a calendar day. | `src/help.ts:487-499` |
| Output | `--json` prints the receipt; otherwise the rendered calculation. No matching records is an error, exit 1. | `program.ts:2160-2164` |

JSON fields (`calculate --json`):

| Field | Meaning |
|-------|---------|
| `calculation_version` | `ath-runtime-v1`. |
| `operation` | Operation run. |
| `record_kind` | Kind of record read. |
| `provider_period_type` | Present for provider-period operations. |
| `method` | Named method used. |
| `quantity` | Quantity, metric, or summary type. |
| `unit` | Unit of the result values. |
| `range.from`, `range.to`, `range.time_zone` | Resolved range and calendar timezone, when used. |
| `sources` | Source IDs that contributed. |
| `input.record_count` | Records used. |
| `input.record_references[]` | `record_id` and `revision` of each record used. |
| `input.references_truncated` | True when the reference list was cut. |
| `exclusions` | Plain reasons for records left out. |
| `result` | Operation-specific values. |

### `series`

| Behavior | Rule | Source |
|----------|------|--------|
| Quantity | Must be a known series quantity, else a refusal listing them. Checked before the file is found. | `program.ts:2199-2206` |
| File | `--file` or discovery. Read-only open. | `program.ts:2207-2208` |
| Window and source | `--from` and `--to` take a calendar day or an exact UTC time. `--source` keeps one source. `--raw` returns every sample instead of one row per day and source. | `program.ts:2210-2216` |
| Timezone | Calendar days use `--timezone`, else the athlete profile's default timezone. With neither, refuse `a calendar-day series view needs --timezone <IANA name> or a default timezone in the athlete profile`. | `program.ts:2210-2217` (observed message) |
| Limit | `--limit` (default 10,000) caps the samples read. | `program.ts:2182`, `program.ts:2216` |
| Empty | No rows refuses: with `--source`, `no <q> samples from '<source>'. Drop --source or run \`ath stats\` for source IDs.`; otherwise `no <q> samples matched. Run \`ath stats\` to see the recorded series.` Exit 1. | `program.ts:2218-2224` |
| Output | `--json` prints the view; otherwise the rendered rows. | `program.ts:2225` |

JSON fields (`series --json`): `quantity`; `unit`; `time_zone`, `from`, `to`, `source_id` (present when used); `truncated` (true when `--limit` cut the read); `samples` (with `--raw`: current `measurement_sample` record objects); `days[]` (without `--raw`: `date`, `source_id`, `sample_count`, `minimum`, `maximum`, `mean`, `first_at`, `last_at`).

### `stats`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file`, else `[file]`, else discovery. Read-only open. | `program.ts:2242-2243` |
| Output | `--json` prints the statistics; otherwise the rendered overview. Exit 0. | `program.ts:2245-2248` |

JSON fields (`stats --json`):

| Field | Meaning |
|-------|---------|
| `athleticstandard_version` | Database record format version. |
| `database_id` | Database ID. |
| `database_revision` | Current database revision. |
| `athlete` | Athlete profile data, or null. |
| `current_records.total` | Current record count. |
| `current_records.by_type` | Count per record type. |
| `sources[]` | `record_id`, `source_type`, `data_writer`, `data_route`, `vendor`, `sensor_type`, `current_record_count`, `first_recorded_at`, `last_recorded_at`. |
| `baselines[]` | Per source and quantity: `source_id`, `quantity`, `unit`, `mean`, `standard_deviation`, `observation_count`, `from`, `to`, `window_days` (90), `method` (`sample_mean_and_standard_deviation`). Never pooled across sources. |
| `series[]` | `source_id`, `quantity`, `unit`, `sample_count`, `from`, `to` for each stored sample series. |
| `benchmarks.defined`, `benchmarks.results` | Benchmark definitions and results. |
| `predictions.total`, `predictions.graded` | Prediction and grade counts. |
| `predictions.by_author[]` | `author` and `predictions` count. |

### `records`

| Behavior | Rule | Source |
|----------|------|--------|
| No action | Group command. Without a subcommand it prints help on stderr and exits 1 (S27). | `program.ts:2256-2260` |

### `records list`

| Behavior | Rule | Source |
|----------|------|--------|
| Type | `--type` must be a known record type, checked before the file is found. | `program.ts:2277-2282` |
| File | `--file` or discovery. Read-only open. | `program.ts:2283-2284` |
| Filter | Current records in stable order; `--include-deleted` adds normally deleted ones; `--limit` default 100. | `program.ts:2286-2290` |
| Output | `--json` prints the list; otherwise the rendered list. | `program.ts:2291-2295` |

JSON fields (`records list --json`): `file`; `records[]` (current record objects: `record_id`, `record_type`, `created_at`, `current_revision`, `is_deleted`, `data`, optional `extensions`).

### `records show`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Read-only open. | `program.ts:2322-2323` |
| Target | Unknown ID refuses `record not found: <id>`. A deleted record is shown. | `program.ts:2325-2328` |
| Revision | `--revision` must exist, else `record <id> has no revision <n>; current revision is <c>`. | `program.ts:2329-2341` |
| Links | `--links` adds incoming and outgoing links. | `program.ts:2342` |
| Output | `--json` prints the object; otherwise the rendered record. | `program.ts:2343-2367` |

JSON fields (`records show --json`):

| Field | Meaning |
|-------|---------|
| `file` | Database path. |
| `record` | Current record object (see `records list`). |
| `shown_revision` | Present with `--revision`: the retained revision object (`record_id`, `record_type`, `revision`, `previous_revision`, `became_known_at`, `became_known_at_basis`, `changed_by`, `data`, optional `extensions`). |
| `history` | Present with `--history`: every retained revision object, in order. |
| `links[]` | Present with `--links`: `direction` (`outgoing` or `incoming`), `relation`, `record_id`, optional `revision`. |

## 6. Predict and grade

### `predict`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Write-capable open, even for `--json`. | `program.ts:1681-1682` |
| As-of time | `--as-of` takes a calendar day or exact UTC time; a day needs `--timezone` or a profile timezone. It hides later data. | `program.ts:1684-1688` |
| `--json` | Prints offline evidence for an agent and stops. It needs no key, makes no network call, and never writes, even with `--yes`. | `program.ts:1689-1703` |
| Model call | Without `--json`, picks `--model`, else the saved default beside the database, else refuses with the "No model chosen" text. Calls the gateway with a fixed system prompt. | `program.ts:1704-1716`; `src/models.ts:158-169`, `src/models.ts:184-195` |
| Response | Code fences are stripped. The reply must be one JSON object; else `model did not return valid prediction JSON: <reason>` or `model prediction response must be one JSON object`. The data is checked against the prediction schema. | `program.ts:1717-1745`; `program.ts:2425-2440` |
| Human preview | Prints the preview JSON. | `program.ts:1746-1752` |
| `--dry-run` | `nothing written (--dry-run)`. | `program.ts:1753-1756` |
| Confirmation | S18 with `Save this prediction?`. | `program.ts:1757-1760` |
| Write | Writes a `prediction` record authored by agent `ath-terminal`. Prints `saved prediction <id> at database revision <r>`. | `program.ts:1761-1773` |

JSON fields (`predict --json`, evidence):

| Field | Meaning |
|-------|---------|
| `purpose` | Always `prediction_evidence`. |
| `is_prediction` | Always false; this is evidence, not a prediction. |
| `as_of` | Exact UTC cut-off time. |
| `benchmark` | Current `benchmark_definition` record object. |
| `prior_results` | Earlier `benchmark_result` records for this benchmark. |
| `device_measurements` | Device `point_measurement` records. |
| `athlete_reports` | `athlete_report` records. |
| `sleep_sessions` | `sleep_session` records. |
| `workout_sessions` | `workout_session` records. |
| `prior_predictions` | Earlier `prediction` records. |
| `gaps` | Plain statements of missing evidence. |
| `calendar_time_zone` | Present when `--as-of` was a calendar day: the timezone used. |

JSON fields (`predict` human preview): `benchmark` (benchmark key); `model` (model ID returned by the gateway); `data` (complete prediction data that would be written); `write_confirmed` (false).

### `backtest`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Write-capable open. | `program.ts:1796-1797` |
| Models | `--all`: every model on the live list. `-m/--model` (repeatable): each must be on the list. Neither: the saved default. All need a gateway key. | `program.ts:1799-1815` |
| `--all` confirmation | Without `--yes`, S18 with `Run a backtest with <n> models?`. A "no" prints `nothing run`, exit 0. | `program.ts:1816-1821` |
| **Quirk:** `--all --json` | `--json` makes the confirmation return "no", so `backtest --all --json` without `--yes` prints the plain text `nothing run` and exits 0. | `program.ts:1816-1821`; `runtime.ts:73-75` |
| Athlete data | Replays history without writing athlete data. If the database revision changes during the run, refuse `backtest changed the athlete database; report was not written`. | `program.ts:1822-1831` |
| Report file | Writes `backtest-<timestamp>.json` beside the database. A per-model failure is recorded in the report, not raised. | `program.ts:1832`; `src/db/backtest.ts:251-254` |
| `--json` | A result after running and writing the report. | `program.ts:1833-1836` |
| Human output | `replayed <n> results with <m> model(s)`, one summary line per model, then `wrote <report>`. | `program.ts:1837-1847` |

JSON fields (`backtest --json`):

| Field | Meaning |
|-------|---------|
| `report` | Path of the report file written. |
| `athleticstandard_version` | Version that wrote the report. |
| `created_at` | Report time. |
| `models` | Models run. |
| `replayable_result_count` | Results with an earlier result on the same benchmark. |
| `skipped_first_result_count` | First results that cannot be replayed. |
| `replays[]` | `model`, `benchmark_result_id`, `benchmark_key`, `as_of`, `actual_value`, optional `predicted_value`, optional `absolute_error_percentage`, optional `error` (a model failure). |
| `summary[]` | `model`, `completed`, `failed`, `median_absolute_error_percentage` (null when none completed). |

### `share`

| Behavior | Rule | Source |
|----------|------|--------|
| Not built | The explicit placeholder stays unimplemented. It names the newest `backtest-YYYY-MM-DDTHHMMSS.json` beside the database and in the working directory. | `program.ts:1862-1884`; `src/share.ts:8-25` |
| File | Uses `--file` or discovery when it succeeds; if not, looks beside `athlete.ath.sqlite` in the working directory. Never refuses for a missing database. | `program.ts:1870-1878` |
| Output | Human: `ath share is not built yet.` and either `Run \`ath backtest\` first...` or the report path. Exit 0. | `program.ts:1883`; `src/share.ts:27-39` |

JSON fields (`share --json`): `built` (always false); `latest_report` (newest report path or null).

### `grade`

| Behavior | Rule | Source |
|----------|------|--------|
| File | `--file` or discovery. Write-capable open. | `program.ts:1909-1910` |
| Plan | Grades the prediction `<prediction-id>` against the benchmark result named by the required `--result`, using `--calculation-version` (default `prediction-grade-v1`), as tool `ath`. Never logs a result. | `program.ts:1912-1921`; `src/help.ts:385-391` |
| `--json` | Prints the preview and stops. | `program.ts:1958-1961` |
| Human preview | Prints the same JSON. | `program.ts:1962` |
| `--dry-run` | `nothing written (--dry-run)`. | `program.ts:1963-1966` |
| Confirmation | S18 with `Write this grade?`. | `program.ts:1967-1970` |
| Write | Writes the link action and the grade. Prints `hit\|miss — signed error <e>, absolute error <p>%` and `grade <id> at database revision <r>`. A hit means the result is inside the predicted range. | `program.ts:1971-1982` |

JSON fields (`grade --json`):

| Field | Meaning |
|-------|---------|
| `file` | Database path. |
| `prediction.record_id`, `prediction.revision` | Exact prediction revision graded. |
| `result.record_id`, `result.revision` | Exact result revision graded. |
| `prediction_result_link.action` | `create` (new link), `revise` (correct an existing link), or `reuse` (existing link already right). |
| `prediction_result_link.record_id` | Link record ID. |
| `prediction_result_link.data` | Link data for `create` and `revise`. |
| `prediction_result_link.current_revision`, `.new_revision` | For `revise`: current and next link revision. |
| `prediction_result_link.revision` | For `reuse`: the link revision reused. |
| `grade.record_id` | Grade record ID this run would write. |
| `grade.data` | Complete grade data, including the deterministic arithmetic. |
| `write_confirmed` | Always false. |

## 7. Uncertainty

- The two `init` quirks were observed on macOS with Node 25 and stdin from `/dev/null` or a pipe. The exact prompt where a piped run stops may vary by Node version.
- `models`, `predict` without `--json`, `backtest`, `key set`, `key clear`, `pull`, and `--update` were read from source only. They were not run, because they need network access, a password store, or a package manager.
- The mapped import's preview `record_ids` were not compared with the IDs of a later confirmed run. Section 4 states only what the source shows.
- `apply` was read from source only; it starts the `ath-core` Rust child process, which may try to build itself in a checkout.
