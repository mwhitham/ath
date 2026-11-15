//! `ath stats` against the 2.1.0 results on every fixture.

mod maintenance_support;

use athleticstandard::store::stats::{database_stats, render_database_stats};
use athleticstandard::store::{OpenOptions, Store};
use maintenance_support::*;

#[test]
fn stats_match_2_1_0() {
    let golden = golden("stats.json");
    for (name, fixture) in [("demo", demo_fixture()), ("npm100", npm100_fixture()), ("v200", version_200_fixture())] {
        let path = copy_fixture(&fixture, "ath-stats-", false);
        let before = file_hash(&path);
        let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
        let stats = database_stats(&store).unwrap();
        drop(store);
        assert_eq!(file_hash(&path), before, "{name}: stats changed the file");
        let expected = &golden[name];
        // Compare the JSON text the command prints, not only the values.
        let ours = athleticstandard::json::to_pretty(&athleticstandard::json::to_value(&stats));
        let theirs = athleticstandard::json::to_pretty(&expected["stats"]);
        assert_eq!(ours, theirs, "{name}: stats JSON differs");
        assert_eq!(render_database_stats(&stats), expected["text"].as_str().unwrap(), "{name}: rendering differs");
    }
}
