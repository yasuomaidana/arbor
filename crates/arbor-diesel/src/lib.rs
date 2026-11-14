//! # arbor-diesel
//!
//! Typed `WITH RECURSIVE` descendant and ancestor traversals over adjacency-list tables for
//! Diesel, built on [`arbor_tree`] and [`diesel_cte_ext`].
//!
//! Enable one or more backend features (`sqlite`, `postgres`, `libsql`) to execute traversals
//! against a database connection.

pub use arbor_tree;
#[cfg(feature = "libsql")]
pub use diesel_libsql;

pub mod sql;

pub use sql::{
    Body, DEFAULT_MAX_DEPTH, Direction, Hierarchy, Scope, ScopeKey, Seed, Step, Traversal,
    TraversalBackend, creates_cycle, reached_depth_limit,
};
