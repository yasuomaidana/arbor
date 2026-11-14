//! Recursive traversals on a real SQLite connection.
#![cfg(feature = "sqlite")]

mod common;

traversal_tests!(
    sqlite,
    diesel::sqlite::Sqlite,
    <diesel::sqlite::SqliteConnection as diesel::Connection>::establish(":memory:")
        .expect("sqlite")
);
