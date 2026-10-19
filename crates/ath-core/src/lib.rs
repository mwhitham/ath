//! Athletic Standard Rust core (foundation scaffold).

pub const SQL_SCHEMA_VERSION: i32 = 1;

const MIGRATION_001_RAW: &str =
    include_str!("../../../spec/sql/migrations/001-initial.sql");

/// Published initial migration SQL (same file Node executes).
pub fn initial_migration_sql() -> &'static str {
    MIGRATION_001_RAW
}

/// Strip the generated comment header so sqlite can execute the body.
pub fn initial_migration_executable_sql() -> String {
    let lines: Vec<&str> = MIGRATION_001_RAW.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim_start().starts_with("CREATE "))
        .unwrap_or(0);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn embedded_migration_creates_metadata_table() {
        let sql = initial_migration_executable_sql();
        let connection = Connection::open_in_memory().expect("open in-memory db");
        connection.execute_batch(&sql).expect("apply migration");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'database_metadata'",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(count, 1);
    }
}
