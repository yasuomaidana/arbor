//! Recursive traversals on a real libSQL connection (own process: see `common/mod.rs`).
#![cfg(feature = "libsql")]

mod common;

traversal_tests!(
    libsql,
    diesel_libsql::LibSql,
    <diesel_libsql::LibSqlConnection as diesel::Connection>::establish(":memory:").expect("libsql")
);
