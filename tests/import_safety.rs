//! Untrusted provider input, ported from `tests/import-safety.test.ts`, and the
//! `ath import` route order from `src/program.ts` and `src/shared-import.ts`.
//!
//! The attachment media-type check in that file belongs to attachment import,
//! not provider import, and is not covered here.
#![cfg_attr(not(unix), allow(unused))]

mod import_support;

use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::symlink;

use athleticstandard::import::{self, input, ImportOptions, ImportRoute, SharedImport};
use import_support::{new_store, TempDir};

fn replace_every(bytes: &[u8], before: &str, after: &str) -> Vec<u8> {
    assert_eq!(before.len(), after.len());
    let mut out = bytes.to_vec();
    let mut index = 0;
    while let Some(found) = out[index..].windows(before.len()).position(|window| window == before.as_bytes()) {
        let start = index + found;
        out[start..start + after.len()].copy_from_slice(after.as_bytes());
        index = start + after.len();
    }
    out
}

fn zip_with(path: &std::path::Path, name: &str, bytes: &[u8]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(bytes).unwrap();
    zip.finish().unwrap();
}

#[test]
fn rejects_archive_traversal_before_reading_an_entry() {
    let directory = TempDir::new();
    let safe = directory.path().join("safe.zip");
    zip_with(&safe, "aa/evil.csv", b"day,readiness_score\n2026-01-01,90\n");
    let unsafe_zip = directory.path().join("unsafe.zip");
    std::fs::write(&unsafe_zip, replace_every(&std::fs::read(&safe).unwrap(), "aa/evil.csv", "../evil.csv")).unwrap();
    let error = input::open_zip(&unsafe_zip).unwrap_err().to_string();
    assert_eq!(error, "invalid relative path: ../evil.csv");
    // Detection reads the same index, so the whole import is refused.
    let error = import::detect_export(&unsafe_zip).unwrap_err().to_string();
    assert_eq!(error, "invalid relative path: ../evil.csv");
    let error = import::classify(&unsafe_zip, false).unwrap_err().to_string();
    assert_eq!(error, "invalid relative path: ../evil.csv");
}

#[test]
fn rejects_absolute_and_backslash_archive_names() {
    let directory = TempDir::new();
    let safe = directory.path().join("safe.zip");
    zip_with(&safe, "aa/evil.csv", b"x\n");
    let absolute = directory.path().join("absolute.zip");
    std::fs::write(&absolute, replace_every(&std::fs::read(&safe).unwrap(), "aa/evil.csv", "/a/evil.csv")).unwrap();
    assert_eq!(input::open_zip(&absolute).unwrap_err().to_string(), "absolute path: /a/evil.csv");
    let drive = directory.path().join("drive.zip");
    std::fs::write(&drive, replace_every(&std::fs::read(&safe).unwrap(), "aa/evil.csv", "C:/evil.csv")).unwrap();
    assert_eq!(input::open_zip(&drive).unwrap_err().to_string(), "absolute path: C:/evil.csv");
    let backslash = directory.path().join("backslash.zip");
    std::fs::write(&backslash, replace_every(&std::fs::read(&safe).unwrap(), "aa/evil.csv", "aa\\evil.csv")).unwrap();
    assert_eq!(input::open_zip(&backslash).unwrap_err().to_string(), "invalid characters in fileName: aa\\evil.csv");
}

#[test]
fn rejects_archive_links_and_extreme_expansion_ratios() {
    let directory = TempDir::new();
    let linked = directory.path().join("linked.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&linked).unwrap());
    zip.add_symlink("export.xml", "target", zip::write::SimpleFileOptions::default()).unwrap();
    zip.finish().unwrap();
    let error = input::open_zip(&linked).unwrap_err().to_string();
    assert_eq!(error, "archive entry is a symbolic link: export.xml");

    let expanded = directory.path().join("expanded.zip");
    zip_with(&expanded, "export.xml", &vec![0_u8; 8 * 1024 * 1024]);
    let error = input::open_zip(&expanded).unwrap_err().to_string();
    assert_eq!(error, "archive entry exceeds the 1000:1 expansion limit: export.xml");
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_inside_provider_directories() {
    let directory = TempDir::new();
    let outside = directory.path().join("outside.csv");
    std::fs::write(
        &outside,
        "Cycle start time,Cycle end time,Cycle timezone\n2026-01-01 00:00:00,2026-01-01 08:00:00,UTC+00:00\n",
    )
    .unwrap();
    let provider = directory.path().join("provider");
    std::fs::create_dir(&provider).unwrap();
    symlink(&outside, provider.join("physiological_cycles.csv")).unwrap();
    let export = import::detect_export(&provider).unwrap();
    assert_eq!(export.format, import::ExportFormat::Whoop);
    let error = import::detect::read_csv_bundle(&export).unwrap_err().to_string();
    assert!(error.contains("symbolic link"), "{error}");

    let store_directory = TempDir::new();
    let store = new_store(store_directory.path());
    let error = import::preview(&store, &provider, &ImportOptions::default()).unwrap_err().to_string();
    assert_eq!(
        error,
        format!("refusing symbolic link in provider export: {}", provider.join("physiological_cycles.csv").display())
    );
    assert_eq!(store.database_revision().unwrap(), 0);
}

#[test]
fn a_zip_entry_is_measured_while_it_is_read() {
    let directory = TempDir::new();
    let path = directory.path().join("data.zip");
    zip_with(&path, "folder/physiological_cycles.csv", b"Cycle start time\n");
    let zip = input::open_zip(&path).unwrap();
    assert_eq!(zip.read_text("folder/physiological_cycles.csv").unwrap(), "Cycle start time\n");
    let length = zip.read_entry("folder/physiological_cycles.csv", |reader| {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes.len())
    });
    assert_eq!(length.unwrap(), 17);
    assert!(zip.read_text("missing.csv").is_err());
}

#[test]
fn routes_follow_the_2_1_0_order() {
    let directory = TempDir::new();
    let root = directory.path();
    let whoop = root.join("whoop");
    import_support::write_directory(&whoop, &import_support::fixture_files());
    assert_eq!(import::classify(&whoop, false).unwrap(), ImportRoute::Provider);
    assert_eq!(import::classify(&whoop, true).unwrap(), ImportRoute::Mapped);

    std::fs::write(root.join("ride.gpx"), "<gpx/>").unwrap();
    assert_eq!(import::classify(&root.join("ride.gpx"), false).unwrap(), ImportRoute::Shared(SharedImport::Garmin));
    std::fs::write(root.join("activity.json"), r#"{"activityName":"x"}"#).unwrap();
    assert_eq!(
        import::classify(&root.join("activity.json"), false).unwrap(),
        ImportRoute::Shared(SharedImport::Garmin)
    );
    std::fs::write(root.join("package.json"), r#"{"athleticstandard_version":"2.1.0"}"#).unwrap();
    assert_eq!(import::classify(&root.join("package.json"), false).unwrap(), ImportRoute::Package);

    let strava = root.join("strava");
    std::fs::create_dir(&strava).unwrap();
    std::fs::write(strava.join("activities.csv"), "x\n").unwrap();
    assert_eq!(import::classify(&strava, false).unwrap(), ImportRoute::Shared(SharedImport::Strava));

    // An Apple export's routes are part of the Apple export.
    let apple = root.join("apple");
    std::fs::create_dir_all(apple.join("workout-routes")).unwrap();
    std::fs::write(apple.join("export.xml"), "<HealthData/>").unwrap();
    std::fs::write(apple.join("workout-routes/route.gpx"), "<gpx/>").unwrap();
    std::fs::write(apple.join("route.gpx"), "<gpx/>").unwrap();
    assert_eq!(import::classify(&apple, false).unwrap(), ImportRoute::Provider);

    let package = root.join("restored");
    std::fs::create_dir(&package).unwrap();
    std::fs::write(package.join("athlete.ath.json"), "{}").unwrap();
    assert_eq!(import::classify(&package, false).unwrap(), ImportRoute::Package);

    // A symbolic link is never a shared import. Creating one needs Unix or
    // extra Windows privileges, so this case runs on Unix.
    #[cfg(unix)]
    {
        symlink(root.join("ride.gpx"), root.join("linked.gpx")).unwrap();
        assert_eq!(import::classify(&root.join("linked.gpx"), false).unwrap(), ImportRoute::Provider);
    }
}

/// Bits in the order deflate packs them: each value starts at its least
/// significant bit, and each Huffman code at its most significant bit.
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    current: u8,
    count: u32,
}

impl Bits {
    fn value(&mut self, value: u32, width: u32) {
        for index in 0..width {
            self.current |= (((value >> index) & 1) as u8) << self.count;
            self.count += 1;
            if self.count == 8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.count = 0;
            }
        }
    }

    fn code(&mut self, code: u32, width: u32) {
        for index in (0..width).rev() {
            self.value((code >> index) & 1, 1);
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.bytes.push(self.current);
        }
        self.bytes
    }
}

/// One deflate block with the fixed codes: `text`, then copies of its last
/// byte until at least `at_least` bytes come out. Each copy of 258 bytes
/// takes 13 bits, so the stream stays small. Returns the stream and the exact
/// number of bytes it expands to.
fn deflate_growing(text: &[u8], at_least: u64) -> (Vec<u8>, u64) {
    let mut bits = Bits::default();
    bits.value(1, 1); // the last block
    bits.value(1, 2); // fixed Huffman codes
    for &byte in text {
        assert!(byte < 144, "the fixed 8-bit literal codes cover bytes 0 to 143");
        bits.code(0x30 + u32::from(byte), 8);
    }
    let mut expanded = text.len() as u64;
    while expanded < at_least {
        bits.code(0xc5, 8); // length 258 (symbol 285)
        bits.code(0, 5); // distance 1
        expanded += 258;
    }
    bits.code(0, 7); // end of block (symbol 256)
    (bits.finish(), expanded)
}

/// A zip with one deflated entry whose local and central headers declare
/// `declared` expanded bytes, whatever `data` really expands to. The CRC is
/// never reached, because the reader stops first.
fn zip_declaring(name: &str, data: &[u8], declared: u32) -> Vec<u8> {
    let date: u16 = ((2026 - 1980) << 9) | (8 << 5) | 9;
    let compressed = u32::try_from(data.len()).unwrap();
    let name_length = u16::try_from(name.len()).unwrap();
    let mut out = Vec::new();
    // Local file header.
    out.extend(0x0403_4b50_u32.to_le_bytes());
    for field in [20_u16, 0, 8, 0, date] {
        out.extend(field.to_le_bytes()); // version, flags, deflate, time, date
    }
    for field in [0_u32, compressed, declared] {
        out.extend(field.to_le_bytes()); // CRC, compressed size, expanded size
    }
    out.extend(name_length.to_le_bytes());
    out.extend(0_u16.to_le_bytes());
    out.extend(name.as_bytes());
    out.extend(data);
    // Central directory.
    let central = u32::try_from(out.len()).unwrap();
    out.extend(0x0201_4b50_u32.to_le_bytes());
    for field in [20_u16, 20, 0, 8, 0, date] {
        out.extend(field.to_le_bytes()); // made by, needed, flags, deflate, time, date
    }
    for field in [0_u32, compressed, declared] {
        out.extend(field.to_le_bytes());
    }
    for field in [name_length, 0, 0, 0, 0] {
        out.extend(field.to_le_bytes()); // name, extra, comment, disk, internal attributes
    }
    for field in [0_u32, 0] {
        out.extend(field.to_le_bytes()); // external attributes, local header offset
    }
    out.extend(name.as_bytes());
    let central_size = u32::try_from(out.len()).unwrap() - central;
    // End of central directory.
    out.extend(0x0605_4b50_u32.to_le_bytes());
    for field in [0_u16, 0, 1, 1] {
        out.extend(field.to_le_bytes());
    }
    out.extend(central_size.to_le_bytes());
    out.extend(central.to_le_bytes());
    out.extend(0_u16.to_le_bytes());
    out
}

/// The count after "got at least" in the measured reader's refusal.
fn counted(error: &str, declared: usize) -> u64 {
    let prefix = format!("too many bytes in the stream. expected {declared}. got at least ");
    let rest = error.strip_prefix(&prefix).unwrap_or_else(|| panic!("{error}"));
    rest.parse().unwrap()
}

#[test]
fn an_entry_that_expands_past_its_declared_size_is_refused_while_it_is_read() {
    use athleticstandard::input_policy::MAX_ZIP_MEMORY_ENTRY_BYTES;
    use std::io::Read;

    let directory = TempDir::new();
    let name = "physiological_cycles.csv";
    let text = b"Cycle start time,Cycle end time,Cycle timezone\n";
    // The headers claim only the first line. The data expands past the
    // limit for an entry read into memory, yet the archive stays small, so
    // the size and expansion-ratio checks on the headers pass.
    let (data, expanded) = deflate_growing(text, MAX_ZIP_MEMORY_ENTRY_BYTES + 1);
    assert!(expanded > MAX_ZIP_MEMORY_ENTRY_BYTES);
    assert!(data.len() < 512 * 1024, "{} compressed bytes", data.len());
    let path = directory.path().join("whoop.zip");
    std::fs::write(&path, zip_declaring(name, &data, text.len() as u32)).unwrap();

    // The archive library alone, which trusts the header, gives every byte.
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut entry = archive.by_index(0).unwrap();
    assert_eq!(entry.size(), text.len() as u64);
    let mut buffer = vec![0_u8; 1 << 20];
    let mut total = 0_u64;
    while let Ok(count) = entry.read(&mut buffer) {
        if count == 0 {
            break;
        }
        total += count as u64;
    }
    assert_eq!(total, expanded);
    drop(entry);

    // The checked index accepts the headers. Reading counts the real bytes
    // and stops just past the declared size, long before the expansion ends.
    let zip = input::open_zip(&path).unwrap();
    assert_eq!(zip.entries[0].size, text.len() as u64);
    let error = zip.read_text(name).unwrap_err().to_string();
    assert!(counted(&error, text.len()) < 1 << 20, "{error}");
    let error = zip
        .read_entry(name, |reader| {
            std::io::copy(reader, &mut std::io::sink())?;
            Ok(())
        })
        .unwrap_err()
        .to_string();
    assert!(counted(&error, text.len()) < 1 << 20, "{error}");

    // The import is refused while it reads the entry, and writes nothing.
    let store_directory = TempDir::new();
    let store = new_store(store_directory.path());
    let error = import::preview(&store, &path, &ImportOptions::default()).unwrap_err().to_string();
    assert!(counted(&error, text.len()) < 1 << 20, "{error}");
    assert_eq!(store.database_revision().unwrap(), 0);
}
