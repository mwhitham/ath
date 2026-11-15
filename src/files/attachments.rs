//! Where an attachment's bytes live, and how a file is checked against its
//! record. This is the one place these rules are written; ingestion, export,
//! restore, erasure, cleanup, and `ath check` all use it.
//!
//! Attachment files sit in `attachments/` beside the database file. A file is
//! stored as `<record ID><extension>`. Its size, SHA-256, and leading bytes must
//! match the record.

use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::durable;
use crate::error::{Error, Result};
use crate::model::AttachmentData;

/// The folder next to the database that holds attachment files.
pub const ATTACHMENT_DIRECTORY: &str = "attachments";

/// The attachment folder for a database file.
pub fn attachment_directory(database_path: &Path) -> PathBuf {
    durable::parent_of(database_path).join(ATTACHMENT_DIRECTORY)
}

/// `attachments/<stored name>`, relative to the database folder.
pub fn relative_attachment_path(stored_name: &str) -> String {
    format!("{ATTACHMENT_DIRECTORY}/{stored_name}")
}

/// The stored file name: the record ID plus an extension. A known media type
/// sets the extension. Otherwise a short alphanumeric extension of the
/// original file name is kept in lowercase, and any other extension is dropped.
pub fn stored_attachment_name(record_id: &str, data: &AttachmentData) -> String {
    let known = match data.media_type.as_str() {
        "image/jpeg" => Some(".jpg"),
        "image/png" => Some(".png"),
        "image/webp" => Some(".webp"),
        "application/pdf" => Some(".pdf"),
        _ => None,
    };
    let extension = match known {
        Some(extension) => extension.to_string(),
        None => {
            let input = posix_extname(&data.file_name);
            let tail = input.strip_prefix('.').unwrap_or_default();
            let short = (1..=10).contains(&tail.len()) && tail.bytes().all(|byte| byte.is_ascii_alphanumeric());
            if input.starts_with('.') && short {
                input.to_ascii_lowercase()
            } else {
                String::new()
            }
        }
    };
    format!("{record_id}{extension}")
}

/// `path.posix.extname`: the last `.` and what follows it in the last path
/// segment, except for a segment that starts with its only dot. Slashes and
/// dots are ASCII, so working on UTF-8 bytes gives the same split as UTF-16.
/// The rule is the same on every operating system, so a stored name never
/// depends on where the record was written.
pub fn posix_extname(path: &str) -> &str {
    let bytes = path.as_bytes();
    let mut start_dot: Option<usize> = None;
    let mut start_part = 0usize;
    let mut end: Option<usize> = None;
    let mut matched_slash = true;
    // 0: dot is the first character of the segment; 1: another dot follows; -1: other text.
    let mut pre_dot_state = 0i8;
    for index in (0..bytes.len()).rev() {
        let byte = bytes[index];
        if byte == b'/' {
            if !matched_slash {
                start_part = index + 1;
                break;
            }
            continue;
        }
        if end.is_none() {
            matched_slash = false;
            end = Some(index + 1);
        }
        if byte == b'.' {
            if start_dot.is_none() {
                start_dot = Some(index);
            } else if pre_dot_state != 1 {
                pre_dot_state = 1;
            }
        } else if start_dot.is_some() {
            pre_dot_state = -1;
        }
    }
    let (Some(start_dot), Some(end)) = (start_dot, end) else {
        return "";
    };
    if pre_dot_state == 0 || (pre_dot_state == 1 && start_dot == end - 1 && start_dot == start_part + 1) {
        return "";
    }
    &path[start_dot..end]
}

/// Node's `"ascii"` decoding clears the high bit of each byte. The 2.1.0
/// signature checks for WebP and PDF compared text decoded that way.
fn ascii_at(bytes: &[u8], range: std::ops::Range<usize>, expected: &[u8]) -> bool {
    bytes.get(range).is_some_and(|slice| slice.iter().map(|byte| byte & 0x7f).eq(expected.iter().copied()))
}

const PNG: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const JPEG: [u8; 3] = [0xff, 0xd8, 0xff];

fn is_webp(bytes: &[u8]) -> bool {
    ascii_at(bytes, 0..4, b"RIFF") && ascii_at(bytes, 8..12, b"WEBP")
}

fn is_pdf(bytes: &[u8]) -> bool {
    ascii_at(bytes, 0..5, b"%PDF-")
}

/// The media type that a file's leading bytes show: PNG, JPEG, WebP, or PDF.
pub fn sniff_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&PNG) {
        Some("image/png")
    } else if bytes.starts_with(&JPEG) {
        Some("image/jpeg")
    } else if is_webp(bytes) {
        Some("image/webp")
    } else if is_pdf(bytes) {
        Some("application/pdf")
    } else {
        None
    }
}

/// True when leading bytes fit the declared media type. PNG, JPEG, WebP, and
/// PDF must start with their signature, any other image type is refused, and
/// a non-image type other than PDF is accepted as stated.
pub fn bytes_match_media_type(media_type: &str, leading: &[u8]) -> bool {
    match media_type.to_lowercase().as_str() {
        "image/png" => leading.starts_with(&PNG),
        "image/jpeg" => leading.starts_with(&JPEG),
        "image/webp" => is_webp(leading),
        "application/pdf" => is_pdf(leading),
        other => !other.starts_with("image/"),
    }
}

/// Refuse bytes that do not match the record's media type.
pub fn check_attachment_bytes(record_id: &str, data: &AttachmentData, bytes: &[u8]) -> Result<()> {
    if !bytes_match_media_type(&data.media_type, bytes) {
        return Err(Error::refused(format!(
            "attachment bytes do not match supported media type {}: {record_id}",
            data.media_type
        )));
    }
    Ok(())
}

/// Leading bytes enough for every signature check.
pub const LEADING_BYTES: usize = 16;

/// The size, SHA-256, and leading bytes of a stored file, read in pieces.
pub struct FileFacts {
    pub byte_count: u64,
    pub sha256: String,
    pub leading: Vec<u8>,
}

/// Read a file's facts without holding it in memory.
pub fn read_file_facts(path: &Path) -> std::io::Result<FileFacts> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut leading = Vec::with_capacity(LEADING_BYTES);
    let mut byte_count = 0u64;
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if leading.len() < LEADING_BYTES {
            let take = (LEADING_BYTES - leading.len()).min(read);
            leading.extend_from_slice(&buffer[..take]);
        }
        hasher.update(&buffer[..read]);
        byte_count += read as u64;
    }
    Ok(FileFacts { byte_count, sha256: durable::hex(&hasher.finalize()), leading })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(file_name: &str, media_type: &str) -> AttachmentData {
        AttachmentData {
            file_name: file_name.into(),
            media_type: media_type.into(),
            byte_count: 0,
            sha256: "0".repeat(64),
            description: None,
        }
    }

    #[test]
    fn extname_follows_node_posix_rules() {
        for (input, expected) in [
            ("index.html", ".html"),
            ("index.coffee.md", ".md"),
            ("archive.tar.gz", ".gz"),
            ("photo.JPG", ".JPG"),
            ("index.", "."),
            ("index", ""),
            (".index", ""),
            (".index.md", ".md"),
            ("..", ""),
            ("a/b.c/d", ""),
            ("a/.b", ""),
            ("a.png/", ".png"),
            ("..png", ".png"),
            ("ä.Tiff", ".Tiff"),
        ] {
            assert_eq!(posix_extname(input), expected, "{input}");
        }
    }

    #[test]
    fn stored_names_use_known_or_short_extensions() {
        let id = "01890f3e-e7d3-7cc0-88c8-00000000000e";
        assert_eq!(stored_attachment_name(id, &data("x.PNG", "image/png")), format!("{id}.png"));
        assert_eq!(stored_attachment_name(id, &data("x.HEIC", "image/heic")), format!("{id}.heic"));
        assert_eq!(stored_attachment_name(id, &data("x.12345678901", "text/plain")), id);
        assert_eq!(stored_attachment_name(id, &data("x.t-t", "text/plain")), id);
        assert_eq!(stored_attachment_name(id, &data("notes", "text/plain")), id);
        assert_eq!(stored_attachment_name(id, &data("x.", "text/plain")), id);
        assert_eq!(stored_attachment_name(id, &data("x.jpeg", "IMAGE/JPEG")), format!("{id}.jpeg"));
    }

    #[test]
    fn media_types_are_sniffed_and_checked_as_2_1_0_did() {
        let png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0];
        assert_eq!(sniff_media_type(&png), Some("image/png"));
        assert_eq!(sniff_media_type(&[0xff, 0xd8, 0xff, 0]), Some("image/jpeg"));
        assert_eq!(sniff_media_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_media_type(b"%PDF-1.7"), Some("application/pdf"));
        assert_eq!(sniff_media_type(b"GIF89a"), None);
        assert_eq!(sniff_media_type(b"RIFF"), None);
        // Node's ascii decoding ignores the high bit.
        assert_eq!(sniff_media_type(&[b'%' | 0x80, b'P', b'D', b'F', b'-']), Some("application/pdf"));
        assert!(bytes_match_media_type("image/png", &png));
        assert!(bytes_match_media_type("Image/PNG", &png));
        assert!(!bytes_match_media_type("image/png", b"%PDF-1.7"));
        assert!(!bytes_match_media_type("image/jpeg", &png));
        assert!(!bytes_match_media_type("image/gif", b"GIF89a"));
        assert!(bytes_match_media_type("text/plain", b"hello"));
        let id = "01890f3e-e7d3-7cc0-88c8-00000000000e";
        let error = check_attachment_bytes(id, &data("a.pdf", "application/pdf"), b"nope").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("attachment bytes do not match supported media type application/pdf: {id}")
        );
    }

    #[test]
    fn file_facts_are_read_in_pieces() {
        let directory = crate::files::make_temporary_directory("ath-facts-").unwrap();
        let path = directory.join("file");
        let bytes: Vec<u8> = (0..200_000u32).map(|index| (index % 251) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        let facts = read_file_facts(&path).unwrap();
        assert_eq!(facts.byte_count, bytes.len() as u64);
        assert_eq!(facts.sha256, durable::sha256_hex(&bytes));
        assert_eq!(facts.leading, bytes[..LEADING_BYTES]);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
