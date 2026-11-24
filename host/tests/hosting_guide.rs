//! The configuration examples in docs/hosting.md are files the host accepts.

use ath_host::config::{Config, Limits, StorageConfig};

/// Every JSON block in the guide that names a storage backend.
fn configuration_examples() -> Vec<String> {
    let guide = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/hosting.md")).unwrap();
    let mut examples = Vec::new();
    let mut rest = guide.as_str();
    while let Some(start) = rest.find("```json\n") {
        let after = &rest[start + "```json\n".len()..];
        let end = after.find("```").expect("a closed code block");
        if after[..end].contains("\"storage\"") {
            examples.push(after[..end].to_string());
        }
        rest = &after[end..];
    }
    examples
}

#[test]
fn the_hosting_guide_examples_are_valid_configurations() {
    let examples = configuration_examples();
    assert_eq!(examples.len(), 2, "one directory example and one remote example");
    let folder = std::env::temp_dir().join(format!("ath-host-guide-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&folder).unwrap();
    let mut kinds = Vec::new();
    for (index, example) in examples.iter().enumerate() {
        let path = folder.join(format!("example-{index}.json"));
        std::fs::write(&path, example).unwrap();
        let config = Config::read(&path).unwrap_or_else(|error| panic!("example {index}: {error}"));
        kinds.push(match config.storage {
            StorageConfig::Directory => "directory",
            StorageConfig::Libsql { .. } => "libsql",
            StorageConfig::LocalServer { .. } => "local_server",
        });
        if let StorageConfig::Directory = config.storage {
            // The directory example spells out the defaults the guide's table lists.
            let defaults = Limits::default();
            assert_eq!(config.limits.body_bytes, defaults.body_bytes);
            assert_eq!(config.limits.page_size, defaults.page_size);
            assert_eq!(config.limits.working_copy_idle_seconds, defaults.working_copy_idle_seconds);
        }
    }
    std::fs::remove_dir_all(&folder).unwrap();
    assert_eq!(kinds, vec!["directory", "libsql"]);
}

/// Section 7 of the guide lists every operation the host serves (D266).
#[test]
fn the_hosting_guide_lists_every_operation() {
    let guide = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/hosting.md")).unwrap();
    for route in ath_host::operations::routes() {
        let start = format!("`{} {}", route.operation.method, route.operation.path);
        let listed = guide
            .match_indices(&start)
            .any(|(index, _)| matches!(guide[index + start.len()..].chars().next(), Some('`') | Some('?')));
        assert!(listed, "docs/hosting.md does not list {start}`");
    }
}
