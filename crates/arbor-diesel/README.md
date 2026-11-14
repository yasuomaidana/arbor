# arbor-diesel

Typed `WITH RECURSIVE` descendant and ancestor traversals for adjacency-list hierarchies on [Diesel](https://diesel.rs), built on [`arbor-tree`](../arbor-tree) and [`diesel-cte-ext`](https://crates.io/crates/diesel-cte-ext).

## Features

| Feature | Backend | Description |
|---|---|---|
| `sqlite` | `diesel::sqlite::Sqlite` | Recursive CTE traversal on SQLite connections |
| `postgres` | `diesel::pg::Pg` | Recursive CTE traversal on PostgreSQL connections |
| `libsql` | `diesel_libsql::LibSql` | Recursive CTE traversal on libSQL connections |

> **TODO (pre-publish):** `diesel-libsql` is currently specified as a git dependency (`branch = "develop"`) for its `cte-ext` feature (upstream PRs `yackey-labs/diesel-libsql#5` and `#6`). Before publishing `arbor-diesel` to crates.io, `diesel-libsql` must be switched to a published crates.io release.

## Quick Example

```rust,ignore
use arbor_diesel::sql::{Hierarchy, creates_cycle, reached_depth_limit};
use diesel::RunQueryDsl;
use diesel::sql_types::Text;

const CATALOG_NODES: Hierarchy = Hierarchy::new("category_nodes", "id", "parent_id")
    .scoped("domain", "catalog")
    .scope_key_or_global("scope_key", Some("org_a"))
    .active_when_true("is_active")
    .live_when_null("deleted_at");

let subtree: Vec<(String, Option<String>, i32)> = CATALOG_NODES
    .descendants::<diesel::sqlite::Sqlite, Text, _>("electronics".to_string())
    .load(&mut conn)?;

let ancestors: Vec<(String, Option<String>, i32)> = CATALOG_NODES
    .ancestors::<diesel::sqlite::Sqlite, Text, _>("ultrabooks".to_string())
    .load(&mut conn)?;

assert!(!creates_cycle(&"engineering".to_string(), &ancestors));
assert!(!reached_depth_limit(&CATALOG_NODES, &subtree));
```

## Safety & Execution Guarantees

1. **Injection-safe.** Table and column names are `&'static str` constants emitted through the backend's identifier quoting. Start IDs, discriminator values, and scope keys are bound parameters.
2. **Bounded termination.** Every traversal stops at `max_depth` (default `64`), preventing infinite recursion on cyclic data. `reached_depth_limit` detects when a result set reached the configured depth ceiling.
3. **Prepared-statement cache isolation.** Because the generated SQL depends on the runtime `Hierarchy` descriptor, traversal query fragments opt out of Diesel's type-keyed prepared-statement cache so queries across different tables or filters never collide.
4. **One SQLite flavor per process.** Plain `diesel::sqlite` and `diesel_libsql` resolve to the same `sqlite3_*` symbols; run their integration tests in separate binaries.
