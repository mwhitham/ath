//! The native Apple Health import compared with 2.1.0.
//!
//! Each golden file was written by `native/tools/reference/import-golden-apple.ts`
//! from the 2.1.0 library, with the one deliberate change it names: a GPX time
//! without a UTC offset is not read in the computer's timezone. A scenario
//! replays the same steps and requires the same preview, write result or
//! refusal, and database: every record's stored JSON for every revision,
//! revision metadata, and import keys.

mod import_support;

use import_support::apple::run_scenario;

#[test]
fn the_fixture_folder_imports_repeats_and_takes_corrections_as_in_2_1_0() {
    run_scenario("watch-folder");
}

#[test]
fn zip_folder_and_bare_xml_containers_match_2_1_0() {
    run_scenario("watch-containers");
}

#[test]
fn a_repeated_record_and_its_ambiguous_correction_match_2_1_0() {
    run_scenario("watch-duplicate");
}

#[test]
fn a_deleted_record_named_again_is_a_conflict_as_in_2_1_0() {
    run_scenario("watch-deleted");
}

#[test]
fn inline_route_points_match_2_1_0() {
    run_scenario("watch-inline-route");
}

#[test]
fn file_reference_routes_match_2_1_0_in_a_folder_and_a_zip() {
    run_scenario("watch-file-route");
}

#[test]
fn route_files_outside_a_workout_match_2_1_0() {
    run_scenario("watch-route-mismatch");
}

#[test]
fn route_files_stay_inside_the_export_as_in_2_1_0() {
    run_scenario("watch-route-scope");
}

#[test]
fn invented_edge_cases_match_2_1_0() {
    run_scenario("watch-edges");
}

#[test]
fn refusals_match_2_1_0() {
    run_scenario("watch-refusals");
}

#[test]
fn records_2_1_0_could_not_write_are_refused_at_the_preview() {
    run_scenario("watch-invalid");
}
