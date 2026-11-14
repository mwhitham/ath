//! Where an attachment's bytes live next to the database, and how a stored
//! file is checked against its record.
//!
//! Attachment files sit in `attachments/` beside the database file. The file
//! name is the record ID plus an extension chosen from the media type, or from
//! a short alphanumeric extension of the supplied file name.

use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::AttachmentData;

/// The attachments folder for a database file.
pub fn attachments_directory(database_path: &Path) -> PathBuf {
    database_path.parent().map(Path::to_path_buf).unwrap_or_default().join("attachments")
}

/// Node's `path.extname` for a POSIX file name.
fn extname(file_name: &str) -> &str {
    let trimmed = file_name.trim_end_matches('/');
    let base = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if base.chars().all(|c| c == '.') {
        return "";
    }
    match base.rfind('.') {
        Some(0) | None => "",
        Some(index) => &base[index..],
    }
}

fn short_extension(extension: &str) -> bool {
    let Some(rest) = extension.strip_prefix('.') else { return false };
    (1..=10).contains(&rest.len()) && rest.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// The stored file name: `<record_id><extension>`.
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
            let supplied = extname(&data.file_name);
            if short_extension(supplied) {
                supplied.to_ascii_lowercase()
            } else {
                String::new()
            }
        }
    };
    format!("{record_id}{extension}")
}

/// True when the leading bytes fit the declared media type. Image types that
/// are not recognized are refused; other types are accepted.
pub fn bytes_match_media_type(media_type: &str, leading: &[u8]) -> bool {
    // Node's "ascii" decoding clears the high bit of each byte.
    let ascii = |range: std::ops::Range<usize>, expected: &[u8]| {
        leading.get(range).is_some_and(|bytes| bytes.iter().map(|byte| byte & 0x7f).eq(expected.iter().copied()))
    };
    let media_type = media_type.to_lowercase();
    match media_type.as_str() {
        "image/png" => leading.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => leading.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => ascii(0..4, b"RIFF") && ascii(8..12, b"WEBP"),
        "application/pdf" => ascii(0..5, b"%PDF-"),
        other => !other.starts_with("image/"),
    }
}

/// The size, SHA-256, and leading bytes of a stored file, read in pieces.
pub struct FileFacts {
    pub byte_count: u64,
    pub sha256: String,
    pub leading: Vec<u8>,
}

pub fn read_file_facts(path: &Path) -> std::io::Result<FileFacts> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut leading = Vec::with_capacity(16);
    let mut byte_count = 0u64;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if leading.len() < 16 {
            let take = (16 - leading.len()).min(read);
            leading.extend_from_slice(&buffer[..take]);
        }
        hasher.update(&buffer[..read]);
        byte_count += read as u64;
    }
    let sha256 = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(FileFacts { byte_count, sha256, leading })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_follows_node_extname() {
        assert_eq!(extname("photo.JPG"), ".JPG");
        assert_eq!(extname(".bashrc"), "");
        assert_eq!(extname("archive.tar.gz"), ".gz");
        assert_eq!(extname("noext"), "");
        assert_eq!(extname("a."), ".");
        assert_eq!(extname(".."), "");
        assert!(short_extension(".gz"));
        assert!(!short_extension("."));
        assert!(!short_extension(".toolongextension"));
    }

    #[test]
    fn media_type_signatures() {
        assert!(bytes_match_media_type("image/png", &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0]));
        assert!(!bytes_match_media_type("image/png", b"%PDF-1.7"));
        assert!(bytes_match_media_type("IMAGE/JPEG", &[0xff, 0xd8, 0xff, 0xe0]));
        assert!(bytes_match_media_type("image/webp", b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!bytes_match_media_type("image/gif", b"GIF89a"));
        assert!(bytes_match_media_type("text/plain", b"hello"));
    }
}
