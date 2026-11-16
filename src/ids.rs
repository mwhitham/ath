//! Stable record IDs.

/// A new lowercase RFC 9562 UUIDv7. The time prefix keeps index order close to
/// creation order; it carries no meaning for the record.
pub fn new_record_id() -> String {
    uuid::Uuid::now_v7().hyphenated().to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_lowercase_v7() {
        let id = super::new_record_id();
        assert!(crate::model::is_record_id(&id), "{id}");
    }
}
