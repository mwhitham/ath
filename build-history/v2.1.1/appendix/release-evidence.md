# Release evidence for 2.1.1

This file records the review evidence that spec section 16 asks for. It was
taken from commit `ba969ab` (`ba969abf07f5305eef500dd0f49e5f6581c97fc7`) on
`build/native-2-1-1`, and from the GitHub checks of that commit. The commit
that adds this file changes only this file and `progress.md`. No release, tag,
or package publication was created.

## 1. Commands and public exports

- **Commands.** [`command-inventory.json`](command-inventory.json) lists the
  2.1.0 program's 32 commands, 138 options, and 31 parse probes. The native
  help text and probe results equal it, in process and through the executable
  (`tests/cli_inventory.rs`, `tests/cli_executable.rs`). `tests/cli_runtime.rs`
  › `every_command_is_connected` reaches each command's handler. The six
  deliberate command-line changes are in [`cli-changes.md`](cli-changes.md).
- **JavaScript exports.** [`public-api.json`](public-api.json) lists 164
  exports from the package root, 144 from `./database`, 162 from `./schema`,
  and 4 JSON-only subpaths. `packages/npm/test/public-api.test.ts` keeps every
  2.1.0 value export and adds none. `packages/npm/test/public-api.generated.ts`
  checks types and signatures at compile time.
- **Zod schemas.** They are frozen compatibility code (D209).
  `packages/npm/test/zod-native-vectors.test.ts` gives them the same verdict as
  the Rust validator on every recorded vector, apart from the listed
  differences.
- No entry is unassigned. The tests fail when a 2.1.0 command, option, probe,
  or export is missing, and the export test also fails on an added export.

## 2. Invariants and tests

Each rule from spec sections 3 to 13 and the hard rules in `AGENTS.md`, with
the tests that check it. "Direct SQL" marks tests that write with their own
SQLite connection, without the library. Test names are exact, so `grep` finds
them.

### Section 3. Completion requirements

| Invariant | Tests | Notes |
|---|---|---|
| One Rust model defines all 24 record types, and the generated JSON Schema and npm protocol types match it. | `tests/generated.rs` › `json_schemas_match_the_model`; `tests/generated.rs` › `the_npm_protocol_module_matches_the_operation_definitions`; `tests/generated.rs` › `the_demo_holds_every_record_type_a_correction_a_deletion_and_an_attachment`; `tests/published_schema.rs` › `package_schema_matches_published_layout` | |
| The native validator gives the 2.1.0 verdict, issue path, code, message and parsed output on every recorded vector. | `tests/model_vectors.rs` › `every_recorded_vector_matches` | The test skips the listed deliberate differences by name: lone surrogates, `__proto__`, version 2.1.1, and `1e400`. |
| The npm Zod schemas keep their published behavior and agree with the Rust validator. | `packages/npm/test/zod-native-vectors.test.ts` › "gives the same verdict on every vector apart from the listed differences"; `packages/npm/test/examples.test.ts` › "composes, infers, and parses with the published Zod schemas" | |
| Normal writes, restore and repair use one record encoder. | `tests/direct_sql.rs` › `the_contract_lists_exactly_the_child_tables_the_encoder_replaces`; `tests/store_replay.rs` › `replaying_the_demo_history_reproduces_every_row`; `tests/check_repair.rs` › `every_typed_column_disagreement_is_found_and_repaired` | Partial. The tests check what the encoder writes, not that it is the only one. By code reading, `src/store/write.rs`, `src/package/restore.rs` and `src/store/repair.rs` all call `src/store/encode.rs`. |
| A caller cannot send raw SQL or a list of low-level steps. Each operation is one domain call. | `packages/npm/test/protocol.test.ts` › "refuses an operation that is not defined, such as raw SQL"; "generates one dispatch entry per operation and no SQL operation"; `src/protocol/worker.rs` › `an_unknown_operation_is_refused`; `src/store/mod.rs` doc tests on `Store` (`compile_fail,E0624` for `connection`, `transaction`, `bulk_transaction`, `fresh_restore_transaction`) | D223. The Rust library hands out no connection, transaction, or SQL closure. |
| Every 2.1.0 command exists and routes to a real handler. | `tests/cli_inventory.rs` › `every_help_text_matches_the_inventory`; `tests/cli_inventory.rs` › `every_probe_matches_the_inventory`; `tests/cli_runtime.rs` › `every_command_is_connected`; `tests/cli_executable.rs` › `help_through_the_executable_matches_the_inventory`; `tests/cli_executable.rs` › `probes_match_through_the_executable` | |
| `share` stays an explicit placeholder. | `tests/runtime_share.rs` › `without_a_report_it_says_to_run_a_backtest`; `tests/runtime_share.rs` › `names_the_newest_report_beside_the_database_or_in_the_working_directory` | Asserts `built: false`. |
| Local commands need no Node, Rust compiler, `sqlite3` program or network. | Every test in `tests/cli_executable.rs` runs `ath` with an empty environment, including `a_repair_from_revisions_restores_the_values_a_calculation_reads`; `.github/workflows/validate.yml` job `archive-smoke` (`tools/archive-smoke.sh`) | The smoke script is a CI job, not a test function. It fails if node, npm, cargo, rustc, a C compiler or sqlite3 is on PATH. |
| Release artifacts work on Linux x64, macOS arm64 and Windows x64. The npm route compiles nothing. | `.github/workflows/validate.yml` jobs `native`, `native-release`, `archive-smoke`, `npm-pack`, `npm-smoke`; `packages/npm/test/launcher.test.ts` › "names a specific installation error on an unsupported system" | CI jobs. Some tests are Unix only (see "Remaining limits"). |

### Section 4.1. Record guarantees

| Invariant | Tests | Notes |
|---|---|---|
| All 24 record types round-trip through the model and the store. | `tests/store_replay.rs` › `every_demo_record_round_trips_through_the_model`; `tests/store_replay.rs` › `replaying_the_demo_history_reproduces_every_row` | |
| Record IDs are lowercase UUIDv7. | `src/ids.rs` › `ids_are_lowercase_v7`; `tests/cli_edit.rs` › `a_record_id_must_be_a_uuid_v7`; `packages/npm/test/database.test.ts` › "makes lowercase UUIDv7 values" | |
| A correction keeps the record ID and every earlier revision. | `tests/cli_edit.rs` › `correct_previews_then_writes_a_new_revision`; `tests/import_whoop.rs` › `imports_matches_a_repeat_and_revisions_a_correction` | |
| Provider matching keys are separate from record IDs and still work after a restore. | `tests/import_whoop.rs` › `uses_restored_provider_keys_for_a_later_correction`; `tests/import_whoop.rs` › `imports_matches_a_repeat_and_revisions_a_correction` | |
| Two-tier wall, library side: a measured record must name a declared source, athlete reports carry none, and an extension cannot hide a source claim. | `tests/import_whoop.rs` › `a_measured_candidate_must_name_a_declared_source`; `tests/import_whoop.rs` › `journal_answers_are_athlete_reports_without_a_source`; `tests/model_rules.rs` › `a_proto_key_cannot_hide_a_source_claim_on_an_athlete_report`; `tests/log.rs` › `structured_logs_refuse_unknown_fields_and_check_extensions`; `tests/write_rules.rs` › `a_batch_with_one_missing_source_writes_nothing` | The SQLite side is in section 7. |
| A manual measurement is marked entered by hand and stays out of device queries and baselines. | `tests/log.rs` › `a_preview_writes_nothing_and_a_write_is_visibly_manual`; `tests/log.rs` › `every_value_entered_through_log_is_marked_entered_by_hand`; `tests/calculate.rs` › `source_baselines_stay_separate_and_manual_values_stay_out` | |
| Unknown provider fields stay under the names the provider gave them. | `tests/cli_executable.rs` › `an_apple_zip_with_a_route_file_imports_through_the_executable`; `tests/package_import.rs` › `reordered_supplied_fields_match_and_keep_the_local_bytes`; `src/import/shared.rs` › `a_supplied_field_needs_a_name_that_every_reader_keeps` | |
| Extensions keep their names, limits and source rules. | `tests/model_vectors.rs` › `every_recorded_vector_matches`; `tests/package_import.rs` › `a_resolved_conflict_keeps_the_chosen_side_s_extensions`; `tests/import_garmin.rs` › `a_changed_activity_keeps_its_extensions` | |
| `field_basis` keeps its meaning, and migration does not add it to old records. | `tests/import_mapping.rs` › `every_mapped_value_states_its_basis`; `tests/migration.rs` › `migration_adds_no_field_basis_to_older_records`; `every_supported_version_migrates_and_keeps_its_history` | |
| Source values, units, timestamp text and offset text are kept as supplied. | `tests/import_whoop.rs` › `imports_matches_a_repeat_and_revisions_a_correction`; `src/import/oura.rs` › `a_bedtime_receipt_keeps_the_offset_as_written`; `src/import/mapping.rs` › `receipts_keep_the_offset_text` | |
| RMSSD and SDNN stay different quantities and never share a calculation. | `tests/log.rs` › `ambiguous_hrv_is_refused_and_nothing_is_written`; `src/import/mapping.rs` › `an_hrv_value_names_its_statistic_in_any_record`; `src/import/mapping.rs` › `a_field_without_the_statistic_needs_an_rmssd_basis_and_writers`; `tests/import_mapping.rs` › `the_aggregator_fixture_maps_fields`; `tests/cli_calculate.rs` › `compare_sources_keeps_each_source_and_reports_the_mean_difference` | |
| Distinct temperature signals and distance types stay distinct. | `tests/import_mapping.rs` › `the_aggregator_fixture_maps_fields` | Partial. Temperature is checked directly (skin versus wrist sleeping). Distance types are checked only through the 2.1.0 golden comparisons. |
| A provider score is not a measurement. | `tests/import_whoop.rs` › `imports_matches_a_repeat_and_revisions_a_correction`; `tests/import_mapping.rs` › `the_aggregator_fixture_maps_fields` | |
| A provider's reporting period is not turned into a calendar day. | `tests/import_oura.rs` › `date_only_scores_keep_their_provider_day`; `tests/import_oura.rs` › `a_sleep_day_value_without_bedtime_stays_date_only`; `tests/prediction.rs` › `provider_period_values_never_become_dated_evidence` | |
| Deletion keeps history. Erasure removes value-bearing history and dependent records, and leaves markers with no values in them. | `tests/cli_executable.rs` › `a_piped_delete_previews_refuses_and_writes_with_yes`; `tests/package_erase.rs` › `erasure_removes_dependents_and_history_and_leaves_value_free_markers`; `tests/prediction.rs` › `an_erased_evidence_reference_resolves_to_its_value_free_marker` | |

### Section 4.2. Command coverage

| Invariant | Tests | Notes |
|---|---|---|
| Help text and parse-only probes match the 2.1.0 inventory. Only the version and the removed `sqlite3` sentence differ. | `tests/cli_inventory.rs` › `every_help_text_matches_the_inventory`; `tests/cli_inventory.rs` › `every_probe_matches_the_inventory`; `tests/cli_executable.rs` › `help_through_the_executable_matches_the_inventory` | |
| Command output, exit status and refusals match 2.1.0 on synthetic scenarios. | `tests/cli_golden.rs` › `set_up_commands_match_2_1_0`; `read_commands_match_2_1_0`; `model_commands_match_2_1_0`; `write_commands_match_2_1_0`; `package_commands_match_2_1_0`; `keyed_import_routes_and_the_personal_connection_match_2_1_0` | Unix only (`#![cfg(unix)]`). |
| `--json` stays a preview, even with `--yes`. | `tests/cli_edit.rs` › `delete_previews_and_writes_only_after_yes`; `tests/cli_golden.rs` › `write_commands_match_2_1_0` (steps `log --yes --json`, `import … --yes --json`) | Partial. Only `delete` checks the record state afterwards. The golden steps compare output only. |
| With no terminal, a write that needs confirmation is refused and writes nothing. | `tests/cli_executable.rs` › `a_piped_delete_previews_refuses_and_writes_with_yes`; `tests/cli_executable.rs` › `version_guide_and_refusals_through_the_executable`; `tests/cli_edit.rs` › `no_controlling_terminal_means_no`; `tests/cli_runtime.rs` › `init_refuses_questions_without_a_terminal`; `tests/cli_runtime.rs` › `an_outdated_database_is_refused_without_a_terminal` | |
| A stale expected revision is refused. | `tests/cli_edit.rs` › `delete_previews_and_writes_only_after_yes`; `tests/cli_edit.rs` › `correct_refuses_bad_data_only_when_writing`; `tests/package_erase.rs` › `stale_and_unknown_erasures_are_refused` | |
| Migration and erasure keep their extra confirmations. | `tests/cli_terminal.rs` › `migrate_asks_for_the_database_id_and_refuses_a_wrong_one`; `a_migration_on_open_needs_both_answers`; `tests/cli_connected.rs` › `migrate_asks_for_the_database_id_at_a_terminal`; `erase_asks_twice_at_a_terminal`; `tests/cli_runtime.rs` › `a_terminal_can_refuse_the_migration` | The `cli_terminal.rs` tests run the real executable on a pseudoterminal (Unix only). Erasure's two questions are tested in-process. |
| `ath` finds only the default file name, or the file given with `--file`. | `tests/cli_runtime.rs` › `discovery_uses_only_the_default_name` | |
| A change to help text regenerates the installed CLI reference. | `tests/generated.rs` › `the_skill_command_reference_matches_the_commands`; `tests/generated.rs` › `the_skill_lists_every_top_level_command`; `tests/assets_skill.rs` › `the_reference_covers_every_command_and_subcommand` | |

### Section 4.3. Provider coverage

| Invariant | Tests | Notes |
|---|---|---|
| An unchanged repeat import matches the stored records. A changed record gets one new revision. | `tests/import_whoop.rs` › `imports_matches_a_repeat_and_revisions_a_correction`; `tests/import_golden.rs` › `fixture_fresh_repeat_and_correction_match_2_1_0`; `tests/import_oura.rs` › `fixture_fresh_repeat_and_correction_match_2_1_0`; `tests/import_apple.rs` › `the_fixture_folder_imports_repeats_and_takes_corrections_as_in_2_1_0`; `tests/import_garmin.rs` › `garmin_files_fresh_repeat_and_change_match_2_1_0`; `tests/import_mapping.rs` › `a_rerun_matches_and_a_change_keeps_the_first_received_time` | |
| A changed group of identical duplicates becomes a conflict, not a guess. | `tests/import_whoop.rs` › `duplicate_rows_keep_stable_ordinals_and_ambiguous_corrections_are_refused`; `tests/import_golden.rs` › `duplicate_rows_and_ambiguous_correction_match_2_1_0`; `tests/import_apple.rs` › `a_repeated_record_and_its_ambiguous_correction_match_2_1_0` | |
| Two devices describing one workout stay two source records. | `tests/import_whoop.rs` › `two_devices_describing_one_workout_stay_two_records`; `tests/import_mapping.rs` › `a_garmin_file_and_a_mapping_stay_two_sources`; `tests/import_garmin.rs` › `garmin_and_strava_imports_keep_their_meaning` | |
| Route and activity files must stay inside the export. | `tests/import_apple.rs` › `route_files_stay_inside_the_export_as_in_2_1_0`; `tests/import_apple_behavior.rs` › `a_route_file_outside_the_workout_or_named_twice_is_not_stored`; `tests/import_apple_behavior.rs` › `a_symbolic_link_inside_a_route_folder_refuses_the_export`; `tests/import_garmin.rs` › `a_strava_activity_file_outside_the_export_is_not_read` | |
| A value a FIT file marks absent stays absent. | `src/import/fit.rs` › `a_reserved_position_is_absent_and_a_big_endian_time_is_not_the_fit_epoch` | |
| A Garmin device peak is not the athlete's maximum heart rate. | `src/import/garmin.rs` › `a_garmin_peak_is_not_the_athlete_maximum`; `tests/import_garmin.rs` › `garmin_and_strava_imports_keep_their_meaning` | |
| Oura periods and WHOOP cycle offsets keep their meaning. | `tests/import_oura.rs` › `date_only_scores_keep_their_provider_day`; `src/import/whoop.rs` › `combines_a_local_time_with_the_cycle_offset`; `tests/import_whoop.rs` › `imports_matches_a_repeat_and_revisions_a_correction` | |
| Skip counts and reasons are reported in the 2.1.0 order. | `tests/import_golden.rs` › `skip_reasons_sort_as_2_1_0_sorted_them`; `tests/import_whoop.rs` › `skip_reasons_are_listed_in_the_2_1_0_order`; `tests/import_oura.rs` › `a_json_met_cell_counts_as_skipped_activity_data` | |
| A deleted source or record is left out, not brought back. | `tests/import_garmin.rs` › `a_deleted_garmin_source_leaves_its_records_out`; `tests/import_garmin.rs` › `a_repeat_of_a_deleted_record_is_left_out_with_it`; `tests/import_mapping.rs` › `a_deleted_source_leaves_its_rows_out_and_makes_no_second_source`; `tests/import_golden.rs` › `a_deleted_record_named_by_a_later_import_is_a_conflict_as_in_2_1_0` | |

### Section 6. Typed model and generated contracts

| Invariant | Tests | Notes |
|---|---|---|
| Strict records refuse unknown fields. Absent versus null, ranges, timestamp precision, safe integers and cross-field rules all match 2.1.0. | `tests/model_vectors.rs` › `every_recorded_vector_matches`; `tests/log.rs` › `structured_logs_refuse_unknown_fields_and_check_extensions`; `src/model/time.rs` › `timestamps_follow_the_published_pattern` | |
| Each semantic rule has valid and invalid vectors that do not depend on either language. | `tests/model_vectors.rs` › `every_recorded_vector_matches` (index in `tests/vectors/model-index.json`) | Partial. The vectors cover rules R1–R56. The database rules S1–S72 are checked by the section 7 and 9 tests, not by vectors. |
| Generated files equal their committed copies, and one command writes them. | `tests/generated.rs` › `json_schemas_match_the_model`; `the_npm_protocol_module_matches_the_operation_definitions`; `the_skill_command_reference_matches_the_commands`; `the_demo_database_matches_its_generator` | |
| Generators never write a published schema, `spec/`, `tests/fixtures/` or `build-history/`. | `tests/generated.rs` › `generated_paths_avoid_published_history` | |
| The current schema equals the published 2.1.0 schema apart from version markers and IDs, and every published schema keeps its bytes. | `tests/published_schema.rs` › `package_schema_matches_published_layout`; `series_schema_matches_published_layout`; `manifest_schema_matches_published_layout`; `tests/published_files.rs` › `published_files_keep_their_bytes`; `every_historical_schema_and_migration_is_pinned` | |
| The Zod facade keeps every 2.1.0 export and agrees with the Rust validator. | `packages/npm/test/public-api.test.ts` › "keeps every 2.1.0 value export of ${entry} and adds none"; `packages/npm/test/zod-native-vectors.test.ts` › "gives the same verdict on every vector apart from the listed differences" | |

### Section 7. One storage implementation

| Invariant | Tests | Notes |
|---|---|---|
| A new database matches the published SQL contract: tables, views, triggers, pragmas, application ID and `user_version`. | `tests/direct_sql.rs` › `the_contract_file_agrees_with_the_code`; `tests/direct_sql.rs` › `a_new_database_matches_the_contract` | direct SQL (read) |
| SQLite refuses a measured row without a source, even with foreign keys off. A missing source fails at commit. | `tests/direct_sql.rs` › `a_measured_record_requires_its_source` | direct SQL |
| SQLite refuses an athlete report, nutrition entry, attachment, or athlete profile that claims a source, on insert and update, with foreign keys off. | `tests/direct_sql.rs` › `an_athlete_report_cannot_claim_a_source`; `nutrition_entries_attachments_and_profiles_cannot_claim_a_source` | direct SQL. The data trigger covers athlete reports and nutrition entries; the extensions trigger covers all four. |
| SQLite enforces revision chains, record-type agreement, an existing current revision and the typed-row type. | `tests/direct_sql.rs` › `revision_chains_and_record_types_are_enforced` | direct SQL |
| SQLite keeps the deletion flag in step with deletion rows, and erasure markers block current rows. | `tests/direct_sql.rs` › `deletion_and_erasure_rules_are_enforced` | direct SQL |
| SQLite refuses a prediction link to a revision that does not exist. | `tests/direct_sql.rs` › `a_prediction_link_must_name_existing_revisions` | direct SQL |
| SQLite keeps one athlete profile, one source per writer, route, and sensor, one benchmark per key, and one record per import key; sessions cannot end before they start; revision extensions are limited to 65,536 characters; an owner links an attachment once. | `tests/direct_sql.rs` › `one_profile_one_source_per_route_one_benchmark_per_key_and_one_record_per_import_key`; `sleep_and_workout_sessions_cannot_end_before_they_start`; `revision_extensions_are_limited_to_65536_characters`; `an_owner_links_an_attachment_once` | direct SQL. Two different owners linking one attachment is refused by the library, not by SQL. |
| The direct-SQL write recipe works. Deferred foreign keys last one transaction, collections are replaced, and the library reads the result and `check` passes. | `tests/direct_sql.rs` › `defer_foreign_keys_lasts_only_one_transaction`; `tests/direct_sql.rs` › `a_direct_correction_replaces_child_collections` | direct SQL |
| The internal recovery table holds no record values and is not a stable writable table. | `tests/direct_sql.rs` › `the_internal_table_is_not_a_stable_writable_table` | direct SQL (read) |
| One logical change is one transaction and one database revision. A refused batch writes nothing. | `tests/write_rules.rs` › `a_batch_with_one_missing_source_writes_nothing`; `tests/log.rs` › `a_batch_is_written_as_one_unit`; `packages/npm/test/database.test.ts` › "writes a batch as one database revision and keeps the caller's order"; `packages/npm/test/protocol.test.ts` › "cancels a write without leaving a partial change" | |
| Writes enforce link types, one benchmark per link, exact grade arithmetic, knowledge time that never goes backward, and references that exist. | `tests/write_rules.rs` › `a_link_must_name_a_prediction_and_a_benchmark_result`; `a_link_must_join_one_benchmark_and_a_grade_must_match_its_arithmetic`; `knowledge_time_cannot_go_backward`; `references_to_missing_attachments_inputs_and_benchmarks_are_refused` | |
| Library writes check foreign keys at commit, honor the busy timeout, and let two writers wait for each other. | `tests/write_rules.rs` › `references_to_missing_attachments_inputs_and_benchmarks_are_refused`; `tests/read_policy.rs` › `a_writer_waits_for_a_held_lock_within_the_busy_timeout`; `tests/read_policy.rs` › `two_writers_wait_for_each_other` | |
| Repair rebuilds typed rows from the retained revision and never adds a revision. | `tests/check_repair.rs` › `check_and_repair_match_2_1_0_on_every_scenario`; `every_typed_column_disagreement_is_found_and_repaired`; `from_revision_rebuilds_without_reading_the_damaged_row`; `a_meaning_difference_needs_from_revision`; `tests/cli_executable.rs` › `a_repair_from_revisions_restores_the_values_a_calculation_reads` | direct SQL setup: the damage is written with an independent connection. |
| Repair makes an owner-only backup first. | `tests/check_repair.rs` › `a_repair_backup_uses_the_2_1_0_name` | Partial. It checks the file and its 0600 mode. Verification is checked through the shared backup code in `tests/backup.rs`. |
| Backups use SQLite's in-process API, are verified and owner-only, never replace a file, refuse links, and include committed WAL data without checkpointing. | `tests/backup.rs` › `a_backup_is_one_verified_owner_only_file`; `an_existing_backup_is_never_replaced`; `a_backup_path_through_a_symbolic_link_is_refused`; `a_backup_that_fails_verification_is_removed`; `backups_work_at_unusual_paths`; `a_backup_includes_committed_data_still_in_the_wal` | |
| Read-only opens and read commands leave the database file byte for byte unchanged, including the journal mode. | `tests/read_policy.rs` › `reads_leave_a_rollback_journal_database_byte_for_byte`; `reads_leave_unhealthy_and_read_only_files_unchanged`; `tests/check_repair.rs` › `check_reads_without_changing_the_file`; `tests/cli_runtime.rs` › `a_read_only_open_does_not_migrate_or_change_the_file`; `tests/cli_executable.rs` › `refusals_leave_no_sidecars_and_reads_keep_a_rollback_journal_database_unchanged` | Two deliberate exceptions. A read-only open rolls back a rollback-journal write that stopped partway (D222; `src/package/tests.rs` › `a_stopped_rollback_journal_write_reads_as_before`). A read of a WAL database can leave an empty `-wal` file and a `-shm` file, which it may not delete (change 79). |
| A foreign application ID, a wrong application marker, a mismatched `user_version` or a newer schema is refused without change. | `tests/read_policy.rs` › `a_mismatched_header_is_refused_without_change`; `tests/migration.rs` › `foreign_and_mismatched_headers_are_refused_without_change`; `a_wrong_application_marker_is_refused_without_change`; `tests/cli_executable.rs` › `an_outdated_or_mismatched_database_is_refused_without_sidecars` | |
| Corrupt and read-only database files are refused without sidecar files. | `tests/cli_executable.rs` › `a_corrupt_database_is_refused_without_sidecars_or_changes`; `tests/cli_executable.rs` › `a_read_only_database_file_is_refused_without_sidecars` | The read-only case is Unix only. |
| Readers see committed WAL data, never checkpoint or delete `-wal` and `-shm`, and see only committed data while a writer holds the file. | `tests/read_policy.rs` › `a_reader_sees_committed_wal_data_and_never_checkpoints_or_deletes_sidecars`; `a_reader_sees_only_committed_data_while_a_writer_holds_the_file`; `a_failed_write_keeps_the_sidecars`; `tests/wal_failed_write.rs` › `keeps_committed_wal_data_when_a_later_write_fails` | |
| A WAL read that cannot work gets a clear error and leaves the file untouched. | `tests/read_policy.rs` › `a_wal_database_in_a_read_only_folder_is_refused_or_read_without_change` | Unix only. |
| History queries keep event time apart from the time a revision became known. Later corrections never leak into earlier evidence. | `tests/prediction.rs` › `corrections_after_the_cut_off_never_leak_into_evidence`; `a_late_import_sees_only_what_was_known_before_the_result`; `deleted_evidence_disappears_only_once_the_deletion_was_known` | |
| Bounded queries use lookup indexes and never read stored revision JSON. | `src/calculate/range.rs` › `bounded_inputs_use_lookup_indexes_and_never_read_stored_json`; `tests/check_performance.rs` › `check_plans_read_by_key` | |

### Section 8. Import previews and bounded processing

| Invariant | Tests | Notes |
|---|---|---|
| A preview writes nothing. | `tests/import_whoop.rs` › `imports_matches_a_repeat_and_revisions_a_correction`; `tests/log.rs` › `a_preview_writes_nothing_and_a_write_is_visibly_manual` | |
| Apply refuses a plan when the database, a target record or the database ID changed after the preview. | `tests/import_whoop.rs` › `a_plan_is_refused_when_the_database_changed_after_the_preview`; `a_correction_is_refused_when_its_record_changed_after_the_preview`; `a_plan_is_refused_by_another_database` | |
| One bad row refuses the whole import, in both preview and write. | `tests/import_mapping.rs` › `a_bad_row_refuses_the_whole_import_in_preview_and_write`; `tests/import_whoop.rs` › `an_invalid_candidate_fails_the_preview_with_its_record_path`; `tests/import_apple_behavior.rs` › `a_sample_that_cannot_be_placed_in_time_refuses_the_preview` | |
| The source, new records, corrections, keys and the database revision commit together. | `tests/import_whoop.rs` › `imports_matches_a_repeat_and_revisions_a_correction`; `tests/write_rules.rs` › `a_batch_with_one_missing_source_writes_nothing`; `src/stopped_writes.rs` › `a_stopped_provider_import_writes_all_or_nothing_and_a_later_import_removes_its_plan`; `a_failed_provider_import_writes_nothing` | The stopped-import test kills a child process before and after the commit. |
| No writer lock is held between preview and apply, during a prompt, or during a model call. | `tests/import_whoop.rs` › `a_plan_is_refused_when_the_database_changed_after_the_preview`; `tests/cli_terminal.rs` › `a_write_confirmed_with_y_is_written_after_another_writer_commits`; `a_stale_delete_is_refused_after_the_record_changed_during_the_prompt`; `tests/cli_connected.rs` › `no_write_lock_is_held_during_a_model_call` | Partial. An archive read and a provider fetch are not held open by a test; both happen before the write transaction opens. |
| Mapping files keep their syntax, installed-file precedence, CRLF handling, typed parsing before any write, and the four refusals. | `tests/import_mapping.rs` › `mapping_refusals_match_2_1_0`; `invented_mapping_files_match_2_1_0`; `an_installed_mapping_file_wins_and_paths_are_named_clearly`; `the_shipped_mapping_files_load`; `src/import/mapping.rs` › `a_crlf_mapping_file_parses_the_same`; `a_line_target_is_typed`; `check_failures_name_the_line_as_2_1_0_did` | |
| Archive traversal, absolute names, links and extreme expansion ratios are refused. | `tests/import_safety.rs` › `rejects_archive_traversal_before_reading_an_entry`; `rejects_absolute_and_backslash_archive_names`; `rejects_archive_links_and_extreme_expansion_ratios`; `rejects_symbolic_links_inside_provider_directories` | The last test is Unix only. |
| Size, count, nesting and document limits refuse work instead of cutting it short. | `tests/import_garmin.rs` › `a_shared_file_over_the_limit_is_refused`; `tests/import_mapping.rs` › `a_mapped_input_over_the_limit_is_refused`; `tests/package_import.rs` › `malformed_deep_oversized_and_linked_documents_are_refused`; `tests/package_attachments.rs` › `unsupported_oversized_and_irregular_files_are_refused` | |
| The real expanded size is measured, not only the size the archive header claims. | `tests/import_safety.rs` › `a_zip_entry_is_measured_while_it_is_read`; `an_entry_that_expands_past_its_declared_size_is_refused_while_it_is_read` | |
| A large import runs in bounded memory. | `tests/import_apple_large.rs` › `a_large_export_imports_in_bounded_memory` (ignored, run by hand); the performance comparison in [`performance/report-2.1.1.md`](../../../performance/report-2.1.1.md) | Measured, not asserted by a unit test. Peak memory stays under 512 MiB at 3 million records. |
| An import applies the same input it previewed. | `tests/package_import.rs` › `a_series_file_changed_after_preview_rolls_back_the_restore`; `src/package/tests.rs` › `a_series_file_changed_after_load_is_refused`; `tests/import_stale_input.rs` › `a_folder_export_changed_after_its_preview_is_refused`; `a_file_export_changed_after_its_preview_is_refused`; `a_route_file_beside_an_apple_export_changed_after_its_preview_is_refused`; `an_export_whose_counts_changed_after_its_preview_is_refused`; `an_unchanged_export_imports_after_its_preview`; `packages/npm/test/database.test.ts` › "refuses a preview whose export changed even when its counts did not" | Change 78. |
| The import staging file is owner-only and is removed after every ending. A later import removes the staging of a stopped process. | `tests/import_staging.rs` › `the_staging_file_is_private_and_removed_after_every_ending`; `src/stopped_writes.rs` › `a_stopped_provider_import_writes_all_or_nothing_and_a_later_import_removes_its_plan` | The mode checks are Unix only. |

### Section 9.1. Package behavior

| Invariant | Tests | Notes |
|---|---|---|
| Export reads one consistent snapshot while another writer commits. | `src/package/tests.rs` › `an_export_reads_one_snapshot_while_another_writer_commits` | |
| Export bytes equal 2.1.0 after the version change, and two exports of one revision are identical. | `tests/package_export.rs` › `demo_export_matches_the_2_1_0_bytes_after_the_version_change`; `tests/package_export.rs` › `two_exports_of_one_revision_are_byte_identical`; `tests/package_golden.rs` › `the_demo_package_restores_and_exports_to_the_same_bytes` | |
| Every sample travels with all of its revisions across series files, and the manifest hashes match. | `tests/package_golden.rs` › `a_package_with_more_samples_than_one_series_file_round_trips_like_2_1_0` | Partial. Earlier revisions change the source and the time. None changes the quantity. |
| Restore refuses unknown fields, unknown versions, unrepresentable content, manifest mismatches and files outside the package. | `tests/package_golden.rs` › `an_unrepresentable_package_is_reported_like_2_1_0`; `tests/package_import.rs` › `every_supported_package_version_is_accepted_and_others_are_reported`; `a_manifest_mismatch_is_refused`; `a_series_file_outside_the_package_is_refused`; `src/package/load.rs` › `package_paths_must_stay_inside_the_package` | |
| A failed or stopped restore writes nothing. A partial restore is never reported as success. | `src/package/tests.rs` › `a_failed_restore_writes_nothing`; `a_stopped_restore_is_not_partial_and_can_run_again`; `a_failed_streamed_restore_reports_the_whole_reading_error`; `tests/package_import.rs` › `a_destination_changed_after_preview_is_refused_and_copied_files_are_removed` | |
| JSON output matches JavaScript byte for byte: UTF-16 key order, integer-like keys, supplementary characters, escaping, -0, exponents, large safe integers and floating round trips. Lone surrogates and out-of-range numbers are refused. | `tests/serialization_vectors.rs` › `the_package_serializer_writes_what_javascript_writes`; `tests/json_vectors.rs` › `serialization_matches_javascript`; `src/json.rs` › `numbers_print_like_javascript`, `array_index_keys_come_first`, `lone_surrogates_are_refused` | |
| A restore into an empty database keeps the database ID, record IDs, history and revision. | `tests/package_import.rs` › `a_restore_preview_writes_nothing_then_the_restore_keeps_everything`; `src/package/restore.rs` › `only_an_empty_database_takes_a_restore`; `tests/package_golden.rs` › `the_version_2_0_0_package_restores_every_row_as_2_1_0_did` | |
| A merge into the same database keeps the conflict workflow and refuses stale choices. | `tests/package_import.rs` › `package_conflicts_are_shown_resolved_and_refused_when_stale`; `local_and_corrected_choices_apply`; `a_merge_adds_later_revisions_and_new_records`; `a_merge_adds_deleted_records_and_deletions`; `src/package/conflicts.rs` › `the_conflict_file_is_strict` | |
| A different non-empty database is a conflict, and 0.x JSON is refused. | `tests/package_import.rs` › `another_nonempty_database_conflicts_and_prerelease_json_is_refused` | |

### Section 9.2. Filesystem safety

| Invariant | Tests | Notes |
|---|---|---|
| The database, sidecars, attachments, packages and backups are owner-only (0600), and folders are 0700. | `tests/write_rules.rs` › `a_database_and_its_sidecars_are_owner_only`; `tests/package_export.rs` › `package_files_and_folders_are_owner_only`; `tests/package_attachments.rs` › `an_attachment_is_copied_and_linked_in_one_change`; `tests/backup.rs` › `a_backup_is_one_verified_owner_only_file` | The mode checks are Unix only. |
| An existing destination is never overwritten. | `tests/package_export.rs` › `an_existing_destination_or_link_is_refused_and_left_alone`; `tests/backup.rs` › `an_existing_backup_is_never_replaced`; `src/files/durable.rs` › `durable_write_creates_an_owner_only_file_and_refuses_to_replace`; `tests/runtime_update.rs` › `an_existing_plan_file_is_not_overwritten`; `packages/npm/test/database.test.ts` › "keeps a backtest report from replacing an existing file" | |
| A path through a symbolic link is refused. | `tests/write_rules.rs` › `a_database_reached_through_a_symbolic_link_is_refused`; `tests/backup.rs` › `a_backup_path_through_a_symbolic_link_is_refused`; `tests/package_export.rs` › `an_attachment_reached_through_a_symbolic_link_is_refused`; `tests/runtime_models.rs` › `refuses_a_symlink_at_the_model_path_and_leaves_the_database_unchanged` | Unix only. |
| Attachments are checked for media type, size, regular file, hash and location. | `tests/package_attachments.rs` › `unsupported_oversized_and_irregular_files_are_refused`; `src/files/attachments.rs` › `media_types_are_sniffed_and_checked_as_2_1_0_did`; `src/package/attachments.rs` › `pending_paths_must_name_one_attachment_file`; `tests/package_export.rs` › `a_missing_attachment_file_is_named` | |
| An export never appears half written. The next export cleans up a stopped export's staging folder. | `src/package/tests.rs` › `a_failed_export_publishes_nothing_and_removes_its_staging`; `a_stopped_export_never_appears_partial_and_the_next_export_cleans_up`; `an_export_stopped_after_publication_is_complete`; `tests/package_export.rs` › `finished_packages_have_no_marker_and_survive_a_later_cleanup` | |
| Cleanup removes only the owned staging of stopped processes, never by name prefix alone. | `src/files/staging.rs` › `removes_a_marked_directory_whose_owner_stopped`; `keeps_a_marked_directory_whose_owner_is_running`; `ignores_unmarked_directories_with_a_matching_name`; `tests/package_export.rs` › `concurrent_exports_into_one_parent_both_succeed`; `src/package/tests.rs` › `cleanup_leaves_a_running_write_alone`; `tests/package_attachments.rs` › `cleanup_removes_only_unreferenced_files_of_stopped_operations` | |
| An attachment reference never commits before its file is safely on disk. A stopped write leaves only unreferenced files, which cleanup removes. | `src/package/tests.rs` › `a_failed_attachment_write_leaves_nothing_behind`; `a_stopped_attachment_write_leaves_only_unreferenced_files_that_cleanup_removes`; `an_attachment_write_stopped_after_commit_is_complete`; `tests/package_attachments.rs` › `a_failed_change_removes_the_new_files` | |
| Erasure records the file removal as pending and reports success only after the file is gone. A failed removal stays visible, and a later writable operation finishes it. | `src/package/tests.rs` › `an_erasure_that_fails_before_commit_changes_nothing`; `an_erasure_whose_file_step_fails_is_not_reported_as_complete`; `a_stopped_erasure_is_finished_by_a_later_writable_operation`; `tests/package_erase.rs` › `a_failed_file_removal_stays_pending_until_a_later_writable_operation`; `erasing_an_attachment_removes_its_file_after_the_records` | The `package_erase` pending test is Unix only. |
| A read-only command reports pending work without changing anything, and `check` reports it. | `tests/package_erase.rs` › `a_failed_file_removal_stays_pending_until_a_later_writable_operation`; `tests/check_repair.rs` › `pending_attachment_work_is_a_warning`; `tests/cli_edit.rs` › `a_writable_command_finishes_pending_attachment_cleanup` | |
| A process killed at each file and SQL boundary leaves usable data, and the pending cleanup completes later. | `src/package/tests.rs`: child processes stopped at each fail point for attachment write, erasure, export and restore; `src/stopped_writes.rs` › `a_stopped_migration_is_at_its_first_or_last_step_and_its_backup_is_intact`; `a_stopped_provider_import_writes_all_or_nothing_and_a_later_import_removes_its_plan`; `a_stopped_repair_repairs_nothing_and_can_run_again` | `ath log` with an attachment uses the attachment path that `src/package/tests.rs` already stops at every boundary. |
| WAL and shared-memory files are never deleted to force cleanup. | `tests/read_policy.rs` › `a_reader_sees_committed_wal_data_and_never_checkpoints_or_deletes_sidecars`; `tests/read_policy.rs` › `a_failed_write_keeps_the_sidecars` | |
| Full disk, denied permissions, corrupt files and sharing violations are handled. | Denied permissions: `tests/package_erase.rs` › `a_failed_file_removal_stays_pending_until_a_later_writable_operation`. Corrupt file: `tests/cli_executable.rs` › `a_corrupt_database_is_refused_without_sidecars_or_changes` | Partial. There is no full-disk test and no Windows sharing-violation test. |

### Section 10. Calculations and reference-agent behavior

| Invariant | Tests | Notes |
|---|---|---|
| The eight range calculations and every one-record skill match 2.1.0. | `tests/calculate_golden.rs` › `range_calculations_match_the_reference`; `one_record_calculations_match_the_reference`; `tests/cli_calculate.rs` › `correlation_pairs_same_day_means`; `workout_drift_compares_the_two_halves_of_the_session` | |
| Sources stay separate, and manual sources are left out. | `tests/calculate.rs` › `source_baselines_stay_separate_and_manual_values_stay_out`; `named_sources_are_compared_without_pooling`; `tests/cli_calculate.rs` › `compare_sources_keeps_each_source_and_reports_the_mean_difference` | |
| More than 100,000 inputs is refused. A receipt lists at most 1,000 references, says it was shortened, and still uses every input. | `tests/calculate.rs` › `more_than_100000_inputs_is_refused_until_the_range_is_narrowed`; `a_receipt_lists_1000_references_says_so_and_uses_every_input`; `workout_drift_bounds_its_inputs_before_the_safety_limit` | |
| Calculation files are typed settings. An installed file wins for `apply`, and the shipped copy stays unchanged. | `src/calculate/definition.rs` › `shipped_files_read_into_typed_settings`; `supplied_intensity_and_feeling_are_refused`; `tests/calculate.rs` › `an_edited_calculation_file_changes_the_result_and_not_the_shipped_copy`; `an_installed_calculation_file_cannot_run_an_expression`; `tests/assets_skill.rs` › `an_installed_calculation_file_wins_for_apply_but_not_for_the_shipped_lookup` | Expression text is read as plain text. |
| Calendar days follow real clock changes, and coverage needs a time zone. | `tests/calculate.rs` › `calendar_days_use_the_real_short_and_long_day_boundaries`; `calendar_coverage_needs_a_timezone`; `src/view/calendar.rs` › `day_bounds_follow_clock_changes` | |
| Reading a calculation does not save a record. | `tests/calculate.rs` › `calculations_write_nothing`; `tests/cli_calculate.rs` › `calculations_leave_the_database_unchanged` | Both check the database revision and the file bytes. |
| Import-time arithmetic (beats, ECG peaks, RMSSD, route splits, elevation) handles noise and artifacts. | `tests/import_apple_behavior.rs` › `ecg_beats_peaks_and_rmssd_match_2_1_0_with_noise_drift_and_artifacts`; `haversine_distances_match_v8_bit_for_bit`; `route_summaries_match_2_1_0`; `src/import/apple/rpeaks.rs` › `survives_drift_and_a_large_artifact`; `src/import/apple/hrv.rs` › `implausible_intervals_are_dropped_and_counted`; `the_pair_across_a_missed_beat_is_ignored`; `src/import/apple/routes.rs` › `splits_interpolate_each_kilometre_and_show_a_fade`; `climbing_ignores_wobble_and_counts_real_rises` | |
| Prediction evidence works offline, and athlete reports are kept apart from device measurements. | `tests/prediction.rs` › `evidence_is_offline_and_names_its_benchmark`; `tests/prediction_golden.rs` › `evidence_and_backtest_match_the_reference` | Partial. The separation is shown by the list names and the 2.1.0 golden file, not by its own assertion. |
| Linking and grading are exact and use revisions. A stale grading plan is refused. | `tests/prediction.rs` › `grading_creates_a_link_then_reuses_it_and_refuses_a_stale_plan`; `tests/prediction_golden.rs` › `grading_matches_the_reference`; `tests/log.rs` › `one_open_prediction_is_linked_and_graded_with_its_result` | |
| Backtests leave out later events and revisions learned later. The first result is an anchor, and deleted results are not replayed. | `tests/prediction.rs` › `a_late_import_sees_only_what_was_known_before_the_result`; `a_corrected_outcome_never_shows_itself_to_the_model`; `a_changed_definition_shows_the_revision_known_at_the_cut_off`; `first_results_are_anchors_and_deleted_results_are_not_replayed` | |
| A failed or invalid model reply writes nothing. A reply cannot set its authorship or move the benchmark. | `tests/prediction.rs` › `a_failed_model_call_or_invalid_reply_writes_nothing`; `a_model_reply_cannot_claim_authorship_or_move_the_benchmark`; `src/prediction/predict.rs` › `replies_must_be_one_object_and_cannot_set_authorship` | |
| Backtest reports are written apart from athlete data. One model's failure does not stop the others. | `tests/prediction.rs` › `a_backtest_replays_a_later_result_and_writes_only_its_report`; `model_failures_are_recorded_and_the_other_models_continue` | |
| Vercel and OpenRouter, model discovery, saved defaults and key precedence all work. | `tests/runtime_gateway.rs` › `lists_models_over_http_with_the_key_and_openrouter_headers`; `connecting_needs_a_key_and_prefers_the_environment`; `tests/runtime_models.rs` › `the_saved_default_is_used_when_no_model_is_named`; `tests/runtime_keyring.rs` › `the_environment_wins_over_a_saved_key` | |
| Password-store entry names are unchanged, environment variables work, and a key is never written to a plain file. | `tests/runtime_keyring.rs` › `the_service_and_accounts_are_unchanged`; `reads_a_key_from_the_environment_for_scripts`; `a_missing_store_refuses_and_never_writes_a_file`; `credential_manager_saves_reads_and_clears_a_key_on_windows` | The macOS and Linux store tests use fake tools. The Windows test uses the real Credential Manager on the CI runner only. |

### Section 11.1. Native executable

| Invariant | Tests | Notes |
|---|---|---|
| Results go to standard output. Refusals, warnings, and progress go to standard error. | `tests/cli_executable.rs` › `version_guide_and_refusals_through_the_executable`; `progress_never_reaches_standard_output`; `tests/cli_edit.rs` › `readable_paths_are_warned_about_except_in_json`; `tests/cli_golden.rs` (compares stdout and stderr separately) | No command draws a progress bar today, as in 2.1.0. |
| Errors are typed and name the field location. | `tests/cli_edit.rs` › `correct_refuses_bad_data_only_when_writing`; `tests/import_whoop.rs` › `an_invalid_candidate_fails_the_preview_with_its_record_path` | |
| Local commands never start `ath-core`, Node or `sqlite3`. | `tests/cli_executable.rs` (empty environment); `.github/workflows/validate.yml` job `archive-smoke` | Partial. Shown by running with no PATH. The code check is in the removal table. |
| Embedded assets equal the repository files, and a copied executable installs skills from an empty folder. | `tests/assets_skill.rs` › `embedded_assets_match_the_repository_byte_for_byte`; `a_copied_executable_installs_skills_from_its_embedded_bytes` | Partial. The copied program is the test binary, which links the same library. No test copies the release `ath`. |
| Installed overrides win, old tool skills are replaced, and user-edited files are kept. | `tests/assets_skill.rs` › `install_replaces_an_older_tool_skill_and_leaves_an_edited_current_one`; `install_keeps_an_edited_calculation_skill_even_when_older`; `installed_mappings_are_copied_once_and_never_replaced`; `the_installed_mapping_file_is_the_one_that_runs` | |

### Section 11.2. npm distribution and client

| Invariant | Tests | Notes |
|---|---|---|
| The launcher uses a prebuilt native executable. An unsupported system gets a specific error, and nothing is downloaded or compiled. | `packages/npm/test/launcher.test.ts` › "names a specific installation error on an unsupported system"; "finds each packed target under native/<target>/"; "reports a missing packed executable and runs nothing" | |
| The launcher passes through arguments, input, output, error, environment, exit status and signals. | `packages/npm/test/launcher.test.ts` › "passes arguments, standard input, output, error, environment, and exit status"; "forwards ${signal} to the native process and exits with its status"; "ends with the signal that ended the native process" | Skipped on Windows (`describe.skipIf(!posix)`). |
| The packed tarball holds the tested executables and installs without a compiler. | `.github/workflows/validate.yml` jobs `npm-pack` and `npm-smoke` (`packages/npm/scripts/smoke.mjs`); `.github/workflows/release-cross-platform.yml` step "Check the tag, the archives, and the tarball agree" | CI jobs. |
| The protocol refuses a wrong or missing version before opening a database, answers with the request ID, and has no SQL operation. | `packages/npm/test/protocol.test.ts` › "refuses another version before opening the database"; "refuses a request without a version"; "answers with the request ID and version"; "generates one dispatch entry per operation and no SQL operation"; `src/protocol/worker.rs` › `another_protocol_version_is_refused_before_anything_runs` | |
| Large results stream. A cancelled write leaves no partial change. A worker that exits early is reported. | `packages/npm/test/protocol.test.ts` › "streams a large result one record per line"; "cancels a write without leaving a partial change"; "reports a worker that exits before it answers" | Partial. The streamed result is a little over 64 KiB, far below the old 64 MiB limit. |
| Callback operations run inside one worker. | `packages/npm/test/database.test.ts` › "runs a backtest with an injected completion function and writes its report" | |
| Public exports, aliases, subpaths and generated type names are kept. | `packages/npm/test/public-api.test.ts` › "keeps every 2.1.0 value export of ${entry} and adds none"; "keeps the 2.1.0 aliases and the database namespace"; "keeps the generated compile-time check current"; "keeps the name, the ath command, and every 2.1.0 subpath"; `packages/npm/test/examples.test.ts` › "runs the README imports against a real database" | |
| A closed handle refuses every operation. | `packages/npm/test/database.test.ts` › "refuses every operation on a closed handle" | |
| A remote URL is never read as a local file name. | `packages/npm/test/database.test.ts` › "keeps remote URL targets remote and refuses them as 2.1.0 did"; `src/protocol/ops/prediction.rs` › `remote_urls_are_recognized_like_the_old_parse` | |
| Each request checks again that its target is the database the handle opened. | `packages/npm/test/protocol.test.ts` › "refuses a handle whose path now holds another database" | |

### Section 11.3. Updates and installation ownership

| Invariant | Tests | Notes |
|---|---|---|
| npm, pnpm and bun installs update through the same package manager, then the new executable refreshes the skills. | `tests/runtime_update.rs` › `recognises_a_global_npm_pnpm_or_bun_install`; `hands_the_install_to_the_launcher_with_the_tool_that_installed_this_copy`; `packages/npm/test/launcher.test.ts` › "installs with ${manager} and refreshes skills with the new executable" | The launcher part is skipped on Windows. |
| `npx`, `dlx` and `bunx` runs keep their behavior. | `tests/runtime_update.rs` › `a_fetched_copy_is_not_a_global_install`; `an_npx_launch_leaves_a_folder_without_a_skill_alone`; `an_npx_launch_replaces_an_older_tool_skill` | |
| A source checkout never starts a build. | `tests/runtime_update.rs` › `a_source_checkout_refuses_and_never_builds` | |
| The update handoff is a checked plan, not a shell string. A bad plan runs nothing. | `tests/runtime_update.rs` › `a_plan_with_another_program_or_arguments_is_refused`; `the_plan_json_has_a_fixed_form`; `packages/npm/test/launcher.test.ts` › "refuses every plan this ath would not write"; "refuses a plan with ${name} and runs nothing"; "runs a batch file through cmd.exe with the path quoted and no shell string from data" | |
| A failed or cancelled update skips the skill refresh. | `packages/npm/test/launcher.test.ts` › "reports a failed install and does not refresh"; "reports a manager ended by a signal and does not refresh"; "passes Ctrl-C to the manager, reports failure, and does not refresh" | Partial. These are POSIX only, and a fake package manager cannot show that the installed files are whole. |
| Archive and Cargo installs get instructions and are not mistaken for npm. | `tests/runtime_update.rs` › `a_cargo_install_gets_the_cargo_command_for_its_source`; `a_release_archive_gets_download_instructions_and_is_not_mistaken_for_npm` | |

### Section 12. Versions, migration and rollback

| Invariant | Tests | Notes |
|---|---|---|
| The program, the npm package, packages and database metadata all carry 2.1.1. | `tests/cli_inventory.rs` › `version_is_the_package_version`; `packages/npm/test/launcher.test.ts` › "prints its version and help through the launcher"; `tests/package_export.rs` › `demo_export_matches_the_2_1_0_bytes_after_the_version_change`; `tests/documents.rs` › `the_specification_and_skill_name_the_current_version`; `.github/workflows/validate.yml` job `native-release` step "Check the executable reports the Cargo and npm version" | |
| Migrations 001–004 stay byte for byte, and the SQL files are what runs. | `tests/published_files.rs` › `published_files_keep_their_bytes`; `tests/migration.rs` › `migrated_tables_match_the_2_1_0_migration`; `tests/direct_sql.rs` › `the_contract_file_agrees_with_the_code` | |
| Migration 005 changes no athlete row, revision or ID. A new database has the same schema as a migrated one. | `tests/migration.rs` › `every_supported_version_migrates_and_keeps_its_history`; `tests/migration.rs` › `migrated_tables_match_the_2_1_0_migration` | |
| 1.0.0, 2.0.0 and 2.1.0 databases migrate and keep their complete history. | `tests/migration.rs` › `every_supported_version_migrates_and_keeps_its_history` | |
| Opening a file never migrates it silently, and the migration preview changes nothing. | `tests/migration.rs` › `every_supported_version_migrates_and_keeps_its_history`; `inspection_keeps_a_rollback_journal_database_as_it_is`; `tests/log.rs` › `a_database_that_needs_migration_refuses_before_a_log_is_planned` | |
| A current database needs no migration and gets no backup. | `tests/migration.rs` › `a_current_database_needs_no_migration_and_no_backup` | |
| The backup is verified and tied to the database ID and revision. A change after the backup requires a new one. | `tests/migration.rs` › `a_change_after_the_backup_makes_it_stale`; `migration_uses_the_reviewed_backup_path`; `tests/backup.rs` › `a_backup_that_fails_verification_is_removed` | |
| A failed step rolls back and names the backup it kept. | `tests/migration.rs` › `a_failed_step_rolls_back_and_names_the_verified_backup` | |
| A foreign application ID or a mismatched header is refused, not repaired. | `tests/migration.rs` › `foreign_and_mismatched_headers_are_refused_without_change` | |
| Rollback means restoring the verified backup and keeping the newer database separately. | `tests/migration.rs` › `rollback_means_restoring_the_verified_backup` | |
| The 2.1.0 program refuses a migrated database and reads the restored backup. | `tests/migration.rs` › `the_2_1_0_program_refuses_a_migrated_database` | Ignored unless `ATH_REFERENCE_CHECKOUT` names a 2.1.0 checkout; it fails if the checkout is missing. It passed against c6218e8 in the `hundred-thousand` job of `.github/workflows/release-performance.yml`. |
| Restore accepts 2.0.0, 2.1.0 and 2.1.1 packages and reports any other version. | `tests/package_import.rs` › `every_supported_package_version_is_accepted_and_others_are_reported` | |

### Section 13. Proof that behavior survived

| Invariant | Tests | Notes |
|---|---|---|
| Outputs equal 2.1.0 at the strongest useful level: bytes, rows, grades, receipts and command output. | `tests/package_golden.rs`, `tests/import_golden.rs`, `tests/import_apple.rs`, `tests/import_oura.rs`, `tests/import_garmin.rs`, `tests/import_mapping.rs`, `tests/cli_golden.rs`, `tests/calculate_golden.rs`, `tests/prediction_golden.rs`, `tests/view_golden.rs` › `record_views_match_the_reference`, `tests/log_golden.rs` › `log_planning_matches_the_reference`, `tests/stats.rs` › `stats_match_2_1_0`, `tests/check_repair.rs` › `check_and_repair_match_2_1_0_on_every_scenario`, `tests/package_erase.rs` › `previews_and_an_erasure_match_2_1_0` | The goldens were recorded from 2.1.0 by `tools/reference/`. |
| Unsupported units, fractional offsets, invalid dates and missing midnights are handled. | `src/import/units.rs` › `converts_known_units_and_refuses_others`; `src/import/whoop.rs` › `reads_bare_offsets_in_either_punctuation`; `src/view/calendar.rs` › `dates_are_checked_like_the_old_parser`; `midnight_that_does_not_exist_is_refused_like_the_old_search`; `src/import/instant.rs` › `refuses_what_v8_refuses` | |
| A GPX time with no offset is not read as UTC, and the suite is not forced to UTC. | `tests/import_apple_behavior.rs` › `the_route_mismatch_fixture_differs_from_the_baseline_only_in_the_offset_free_splits`; `src/import/garmin.rs` › `a_time_without_an_offset_is_kept_as_written`; `src/import/shared.rs` › `a_time_without_an_offset_is_not_a_stored_instant` | `cli_golden.rs` runs with `TZ=America/Chicago`. |
| Every record type goes through create, correct, history, export, restore, merge, delete and erasure. | `tests/record_lifecycle.rs` › `every_record_type_lives_through_correction_export_restore_merge_deletion_and_erasure`; `grades_and_evidence_keep_the_revisions_they_name_after_later_corrections`; `tests/store_replay.rs` › `replaying_the_demo_history_reproduces_every_row` | All 24 types, each with a child removed by its correction where it has children. |
| Unicode field names, malicious extensions and strict-schema refusals are handled. | `tests/package_golden.rs` › `a_package_with_more_samples_than_one_series_file_round_trips_like_2_1_0`; `tests/model_rules.rs` › `a_proto_key_cannot_hide_a_source_claim_on_an_athlete_report`; `tests/log.rs` › `structured_logs_refuse_unknown_fields_and_check_extensions` | |
| Paths with spaces, non-ASCII characters, quotes, backslashes and drive letters work. | `tests/backup.rs` › `backups_work_at_unusual_paths`; `tests/assets_skill.rs` › `a_copied_executable_installs_skills_from_its_embedded_bytes`; `tests/import_safety.rs` › `rejects_absolute_and_backslash_archive_names`; `tests/runtime_update.rs` › `recognises_the_same_layouts_with_backslashes` | |
| Native tests run on Linux, macOS and Windows. npm tests run on Node 22 and 24. | `.github/workflows/validate.yml` jobs `native` (three systems), `npm-client` (Node 22 and 24) and `npm-launcher-windows` | Partial. See "Remaining limits". |
| Prompts are tested through a pseudoterminal. | `tests/cli_terminal.rs` (six tests) | Unix only. |
| The release archive works with no Node, Cargo, `sqlite3` or source tree. | `.github/workflows/validate.yml` job `archive-smoke` (`tools/archive-smoke.sh`) | The script creates, imports, corrects, calculates, exports, restores, repairs (confirmed), migrates, and checks. |
| The packed npm tarball installs into an empty folder with no compiler. | `.github/workflows/validate.yml` job `npm-smoke` | |

### AGENTS.md hard rules

| Invariant | Tests | Notes |
|---|---|---|
| Two-tier wall. | See sections 4.1 and 7. | |
| Only reviewed synthetic health-data files are tracked, with pinned hashes. | `tests/public_data.rs` › `only_reviewed_synthetic_health_data_is_tracked`; `tests/public_data.rs` › `health_data_names_are_recognized` | |
| Generated files are current, and generators never write history or fixtures. | `tests/generated.rs` (all tests) | |
| The demo is deterministic, and the 2.1.0 demo is a preserved input that is never regenerated. | `tests/generated.rs` › `the_demo_database_matches_its_generator`; `the_demo_keeps_the_records_of_the_2_1_0_demo`; `tests/public_data.rs` › `only_reviewed_synthetic_health_data_is_tracked` (pins the hash of `tests/fixtures/version-2-1-0/athlete.ath.sqlite`) | |
| SDNN and RMSSD never share a baseline. | See section 4.1. | |
| Append-only build history, naming, prose, impersonal content and sanitized evidence. | none | These are review rules. No test checks them. |

## 3. Comparison with 2.1.0

The reference is commit c6218e8 (2.1.0). The generators in `tools/reference/`
ran it from a separate checkout and recorded its outputs in `tests/golden/`
and `tests/vectors/`. The native tests compare against those files and never
load 2.1.0 code.

| Area | What is compared | Tests and size |
|---|---|---|
| Validator | Verdict, issue order, paths, codes, messages, and parsed output | `tests/model_vectors.rs`: 7,399 recorded cases in 140 files. Every case matches apart from the listed deliberate differences (lone surrogates, `__proto__`, the version, and `1e400`). |
| JSON Schema | The published files | `tests/published_schema.rs`: equal apart from the version |
| Stored rows | Replaying the demo history | `tests/store_replay.rs`: every athlete row equal byte for byte |
| Packages | Export bytes, restore, merge, and the 2.0.0 package | `tests/package_export.rs`, `tests/package_golden.rs` (19 golden files) |
| Provider imports | Rows, history, import keys, skip reasons, and refusals for WHOOP, Oura, Apple Health, Garmin, FIT, Strava, and mapped files | `tests/import_golden.rs`, `tests/import_apple.rs`, `tests/import_oura.rs`, `tests/import_garmin.rs`, `tests/import_mapping.rs` (67 golden files) |
| Calculations, prediction, views, and log planning | Values, receipts, grades, evidence, backtests, and plans | `tests/calculate_golden.rs`, `tests/prediction_golden.rs`, `tests/view_golden.rs`, `tests/log_golden.rs` |
| Maintenance | Check, repair, migration, and stats | `tests/check_repair.rs` (19 check scenarios), `tests/migration.rs` (1.0.0, 2.0.0, and 2.1.0 fixtures), `tests/stats.rs` |
| Command line | Output, exit status, and files, step by step | `tests/cli_golden.rs`: 39 scenarios, 295 steps |
| Help and probes | Text and parse results | `tests/cli_inventory.rs`: 33 help texts, 31 probes |

Each golden file names its normalization in its own `normalization` field. The
rules are field-specific: the scenario's working folder, the program version,
the SQL schema step each program writes, times and new UUIDv7 values created
during the run (numbered by first appearance), and package file hashes in a
manifest, whose bytes the package tests compare separately.

The deliberate differences are listed in
[`deliberate-changes.md`](deliberate-changes.md) (79 entries) and
[`cli-changes.md`](cli-changes.md) (6 entries). The same file lists the 2.1.0
behavior kept on purpose.

## 4. Migration, recovery, and rollback

- **Migration.** Synthetic 1.0.0, 2.0.0, and 2.1.0 databases migrate to SQL
  step 5 and keep their complete history. The migrated tables equal the 2.1.0
  migration's, and no record gains a `field_basis`. The backup is verified, a
  change after the backup is refused, and a failed step rolls back
  (`tests/migration.rs`).
- **Stopped processes.** A child process is killed at named points in a
  migration, a provider import, a repair, a package restore (including the
  rollback-journal restore of D221), an export, an attachment write, and an
  erasure. A new process then opens the file. It finds the change whole or
  absent, and finishes any pending file work (`src/stopped_writes.rs`,
  `src/package/tests.rs`).
- **Failed writes.** A failed migration, import, repair, restore, export,
  attachment write, or erasure writes nothing, and the WAL keeps committed data
  when a later write fails (`tests/wal_failed_write.rs`).
- **Rollback.** Going back to 2.1.0 means restoring the verified backup
  (`tests/migration.rs` › `rollback_means_restoring_the_verified_backup`). The
  2.1.0 program refuses a migrated database and reads the restored backup
  (`the_2_1_0_program_refuses_a_migrated_database`, run against c6218e8).
- **Packages.** Packages from 2.0.0, 2.1.0, and 2.1.1 restore
  (`tests/package_import.rs`).

## 5. Performance and maintenance

[`performance/report-2.1.1.md`](../../../performance/report-2.1.1.md) has the
full comparison. Every spec section 14 target passes on the measuring machine:

- The 3-million-record initial import takes 0.465 of the 2.1.0 median.
- The 3-million-record check takes about 0.19 of the 2.1.0 time. 2.1.0 needed a
  larger Node heap to finish it.
- Export is faster at both sizes, and all nine existing limits pass.
- Peak memory stays under 512 MiB for import, export, and check.

The same report describes the two maintenance exercises: an invented optional
field and an invented provider reader. Both were measured and reverted.

## 6. Clean installs

The GitHub checks of `ba969ab`: the CI workflow (run
[43708278665](https://github.com/mwhitham/ath/actions/runs/43708278665)), the
release workflow's validation without publishing (run
[43708278490](https://github.com/mwhitham/ath/actions/runs/43708278490)), and the
performance workflow (run
[43708278498](https://github.com/mwhitham/ath/actions/runs/43708278498)).
Every job passed. The release workflow's `publish` job and the
3-million-record job were skipped, because they run only for a tag or by hand.

| Job | Systems | Result |
|---|---|---|
| native: format, Clippy, every Rust test | Ubuntu, macOS, Windows | passed |
| native: Windows Credential Manager test | Windows | passed |
| release build | linux-x64 (Ubuntu 22.04), darwin-arm64 (macOS 15), win32-x64 (Windows Server 2022) | passed |
| archive smoke | linux-x64, darwin-arm64, win32-x64 | passed |
| npm client | Node 22, Node 24 | passed |
| npm-pack | Ubuntu | passed |
| npm smoke | Ubuntu 22.04, macOS 15, Windows Server 2022 | passed |
| npm-launcher-windows | Windows | passed |
| 100,000-record comparison, and the 2.1.0 refusal test | macOS 15 | passed |

- **Release build** builds each executable once and checks that it reports the
  Cargo and npm version.
- **Archive smoke** runs the release archive with Node, Cargo, `sqlite3`, and
  the source tree absent. It creates a record, imports synthetic provider data,
  corrects, calculates, exports, restores, repairs, migrates, and checks.
- **npm smoke** installs the packed tarball into an empty folder with no
  compiler.

## 7. Removal list

| Removed item | Status | Evidence |
|---|---|---|
| TypeScript database implementation (`src/*.ts`, `src/db/`, `src/v1/`, `src/program.ts`) | Gone | Git tracks no `.ts` file outside `packages/npm/` and `tools/`. What remains is legitimate. Comments name the old files as the source of a rule: `src/import/oura.rs:26`, `src/import/provider.rs:18` and `:85`, `src/import/candidate.rs:8`, `src/import/apple/mod.rs:5`. `packages/npm/src/schema.ts:4` and `packages/npm/src/extensions.ts:2` are the frozen Zod compatibility code (D209). `tools/performance/compare.ts:508`, `:570-571`, `:594` and `:883` run 2.1.0 TypeScript only inside the separate reference checkout (`cwd: reference`) for the performance comparison. |
| `crates/ath-core` | Gone | There is no `crates/` folder, and `Cargo.toml` defines one package. Remaining mentions are comments in `src/runtime/garmin_pull.rs:13`, `src/import/oura.rs:25`, `garmin.rs:24`, `units.rs:8`, `strava.rs:14`, `fit.rs:9`, `csv.rs:8`, `mapping.rs:35`, `src/import/apple/mod.rs:3` and `tools/performance/compare.ts:12`. `.github/workflows/release-performance.yml:65` and `:109` build `crates/ath-core` inside the 2.1.0 reference checkout for the old/new performance comparison (spec 14). This is legitimate. It is a release-evidence job, not a production operation. |
| `ath-core` executable bridge | Gone | There is no `runAthCore` and no spawn of `ath-core`. The only processes native code starts are the password-store tools and `stty` in `src/runtime/keyring.rs` (`:267`, `:273`, `:291`, `:303`, `:341`, `:346`, `:361`, `:675`, `:695`). npm starts only the native `ath` (`packages/npm/src/launcher.ts:540`, `packages/npm/src/client/worker.ts:128` and `:273`). |
| String action interpreter | Gone | `src/protocol/mod.rs:5` says the protocol exposes no SQL or action interpreter. `protocol.test.ts` asserts that no `sql`, `execute` or `action` operation exists. `src/import/staging.rs:23` `enum Action` is a typed match result (New, Matching, Changed and so on), not an interpreter. `Store::transaction` is crate-private (D223). |
| External `sqlite3` backup calls | Gone | Backups use the rusqlite backup API (`src/store/backup.rs:3`). Two legitimate mentions remain, and neither starts a process. `src/protocol/ops/helpers.rs:95-102` and `:245`, with `packages/npm/src/database.ts:294`, keep the public `sqlite3BackupDotCommand` export, which only formats a `.backup` text line (spec 11.2 keeps every export). `tools/performance/compare.ts:140`, `:754` and `:1330` call `sqlite3` in the benchmark tool only. |
| TypeScript SQL generator | Gone | The SQL files are embedded directly with `include_str!` at `src/store/migrate.rs:36`, `:42`, `:48`, `:54` and `:60`. There is no SQL in `packages/npm/src/`. |
| Rebuilds triggered by source modification time | Gone | No production code rebuilds at runtime. Two legitimate mentions remain. `build.rs:24`, `:32` and `:44` use `cargo:rerun-if-changed`, which is Cargo's normal compile-time asset tracking. `src/runtime/share.rs:44` and `:65` use modification time only to pick the newest backtest report. Tests: `tests/runtime_update.rs` › `a_source_checkout_refuses_and_never_builds`; `packages/npm/test/launcher.test.ts` › "reports a missing packed executable and runs nothing". |
| Legacy 0.x runtime schema and signal conversion; legacy import intermediates | Gone | Only refusals remain: `src/package/load.rs:119`, `src/cli/runtime.rs:25`, `:440` and `:461`, and the help text at `src/cli/examples/import.txt:34-35`. No signal or intermediate types remain. Test: `tests/package_import.rs` › `another_nonempty_database_conflicts_and_prerelease_json_is_refused`. |
| Duplicate Rust encoders | Gone | Typed-row encoding lives only in `src/store/encode.rs`. It is called from `src/store/write.rs:266` and `:371`, `src/package/restore.rs:530` and `src/store/repair.rs:216`. History rows are inserted in both `src/store/write.rs` and `src/package/restore.rs`. This matches spec 7's separate create and restore operations over one encoder. |
| Old build and publish scripts | Gone | No root `scripts/` is tracked. The npm package scripts are only `build`, `typecheck` and `test` (`packages/npm/package.json:71-74`). Packing and smoke testing use the new `packages/npm/scripts/pack.mjs` and `smoke.mjs`. `pnpm install` and `pnpm build` appear only for the reference checkout in `.github/workflows/release-performance.yml`, which is legitimate. |
| Root Node package files (`package.json`, `pnpm-lock.yaml`, root `tsconfig`) | Gone from Git | None is tracked. |
| Temporary `native/` source tree | Gone from Git | Nothing under `native/` is tracked. The source is at the root layout from spec 5 (`src/`, `Cargo.toml`). |

## 8. Generated files, public data, and documents

These ran in `cargo test` on Linux, macOS, and Windows for `ba969ab`:

- `tests/generated.rs`: the JSON Schema, the npm protocol module, the skill's
  command reference, and the demo database equal their committed copies.
- `tests/published_files.rs`: every published schema and SQL migration keeps
  its bytes.
- `tests/public_data.rs`: only the reviewed synthetic health-data files are
  tracked.
- `tests/documents.rs`: the specification, README, and skill name the current
  version and record count.

## Remaining limits

- No test fills the disk, and no test causes a Windows sharing violation.
- `tests/cli_golden.rs` and the pseudoterminal tests run on Unix only. Windows
  runs every other native test, the archive and npm smoke tests, the npm
  launcher job, and the Credential Manager test.
- The launcher's signal-forwarding tests run on POSIX only.
- Bounded memory is measured by the performance comparison, not asserted by a
  unit test. Every number in the report comes from one machine, and the
  3-million-record comparison needs the dedicated runner.
- The streamed-result test sends a little over 64 KiB.
- No complete Garmin account export fixture exists yet.
- On macOS, saving a key passes it to `security add-generic-password` on the
  command line, as 2.1.0 did.
