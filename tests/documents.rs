//! The published documents agree with the format the code implements.

use athleticstandard::model::package::ATHLETIC_STANDARD_VERSION;
use athleticstandard::model::RecordType;

const DOCUMENTS: &[&str] = &["SPEC.md", "README.md", "skills/athletic-standard/references/format.md"];

fn read(relative: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)).unwrap()
}

#[test]
fn documents_state_the_record_type_count() {
    let count = RecordType::ALL.len();
    assert_eq!(count, 24);
    for file in DOCUMENTS {
        let text = read(file);
        assert!(text.contains(&count.to_string()), "{file} does not state {count}");
        assert!(!text.contains("23 record types") && !text.contains("one of 23"), "{file} names the old count");
    }
}

#[test]
fn documents_name_the_chunked_series_files() {
    for file in DOCUMENTS {
        let text = read(file);
        assert!(text.contains("<source-id>-<quantity>-000001.ath-series.json"), "{file}");
        assert!(!text.contains("<source-id>-<quantity>.ath-series.json"), "{file} names the unchunked file");
    }
}

#[test]
fn the_specification_and_skill_name_the_current_version() {
    let spec = read("SPEC.md");
    let title = spec.lines().next().unwrap();
    assert!(title.ends_with(&format!("v{ATHLETIC_STANDARD_VERSION}")), "{title}");
    let skill = read("skills/athletic-standard/SKILL.md");
    assert!(skill.contains(&format!("version: \"{ATHLETIC_STANDARD_VERSION}\"")), "SKILL.md frontmatter version");
    assert!(skill.contains(&format!("**{ATHLETIC_STANDARD_VERSION}**")), "SKILL.md names the version it expects");
}

/// Every relative Markdown link in the user-facing documents and the current
/// build-history folder points to a file that exists.
#[test]
fn relative_links_in_the_documents_resolve() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<std::path::PathBuf> =
        ["README.md", "CONTRIBUTING.md", "AGENTS.md", "SPEC.md"].iter().map(|name| root.join(name)).collect();
    for folder in ["docs", "build-history/v2.1.2", "build-history/v2.1.2/appendix"] {
        for entry in std::fs::read_dir(root.join(folder)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "md") {
                files.push(path);
            }
        }
    }
    let mut broken = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let mut rest = text.as_str();
        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];
            let Some(end) = after.find(')') else { break };
            let target = after[..end].split('#').next().unwrap_or("");
            rest = &after[end..];
            if target.is_empty() || target.contains("://") || target.starts_with("mailto:") || target.contains(' ') {
                continue;
            }
            if !file.parent().unwrap().join(target).exists() {
                broken.push(format!("{}: {target}", file.strip_prefix(root).unwrap().display()));
            }
        }
    }
    assert!(broken.is_empty(), "broken links:\n{}", broken.join("\n"));
}
