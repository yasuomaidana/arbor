# arbor

Generic, application-agnostic acyclic trees and typed recursive-CTE SQL traversals for Rust.

`arbor` works on any adjacency-list hierarchy (a node `id` plus a nullable `parent_id`): category taxonomies, file and directory trees, organizational charts, and nested component hierarchies.

## Crates

| Crate | Description | Dependencies |
|---|---|---|
| [`arbor-tree`](crates/arbor-tree) | Database-free in-memory forests (`FlatNode`, `TreeNode`), cycle guards (`would_create_cycle`), pre/post-order iterators, generic bottom-up rollups (`rollup`, `sum_rollup`, `checked_sum_rollup`), downward attribute inheritance (`inherit_by`), and multi-axis rank resolution (`resolve_by_rank`). | `serde`, `thiserror`, optional `ts-rs` |
| [`arbor-diesel`](crates/arbor-diesel) | Typed `WITH RECURSIVE` descendant and ancestor traversals (`Hierarchy`, `Scope`, `ScopeKey`, `Direction`, `Traversal`) on Diesel, built on `diesel-cte-ext`. | `arbor-tree`, `diesel`, `diesel-cte-ext`, optional `diesel-libsql` |

## Feature Table

| Crate | Feature | Default | What it enables |
|---|---|---|---|
| `arbor-tree` | `to-typescript` | No | `ts-rs` TypeScript type export derives on `FlatNode`, `TreeNode`, and `TreeError` |
| `arbor-diesel` | `sqlite` | No | Recursive CTE execution on `diesel::sqlite::Sqlite` |
| `arbor-diesel` | `postgres` | No | Recursive CTE execution on `diesel::pg::Pg` |
| `arbor-diesel` | `libsql` | No | Recursive CTE execution on `diesel_libsql::LibSql` |

## Quick Start

### In-memory trees (`arbor-tree`)

```rust
use arbor_tree::{FlatNode, TreeNode, inherit_by, would_create_cycle};

let flat_nodes = vec![
    FlatNode { id: "root", parent_id: None, data: 100_u64 },
    FlatNode { id: "docs", parent_id: Some("root"), data: 250_u64 },
    FlatNode { id: "readme", parent_id: Some("docs"), data: 50_u64 },
];

let forest = TreeNode::build_forest(flat_nodes)?;
assert_eq!(forest[0].checked_sum_rollup(&|size| *size), Some(400));
assert_eq!(forest[0].path_to(&"readme"), Some(vec!["root", "docs", "readme"]));

let pairs = vec![("root", None), ("docs", Some("root")), ("readme", Some("docs"))];
assert!(would_create_cycle(&pairs, &"root", &Some("readme")));
# Ok::<(), arbor_tree::TreeError>(())
```

### Recursive SQL traversals (`arbor-diesel`)

```rust,ignore
use arbor_diesel::sql::{Hierarchy, creates_cycle, reached_depth_limit};
use diesel::RunQueryDsl;
use diesel::sql_types::Text;

const CATEGORIES: Hierarchy = Hierarchy::new("category_nodes", "id", "parent_id")
    .scoped("domain", "catalog")
    .scope_key_or_global("scope_key", Some("scope_a"))
    .active_when_true("is_active")
    .live_when_null("deleted_at");

let subtree: Vec<(String, Option<String>, i32)> = CATEGORIES
    .descendants::<diesel::sqlite::Sqlite, Text, _>("electronics".to_string())
    .load(&mut conn)?;

let ancestors: Vec<(String, Option<String>, i32)> = CATEGORIES
    .ancestors::<diesel::sqlite::Sqlite, Text, _>("all_in_one".to_string())
    .load(&mut conn)?;

assert!(!creates_cycle(&"engineering".to_string(), &ancestors));
assert!(!reached_depth_limit(&CATEGORIES, &subtree));
```

## Implemented vs planned

| Capability | Status | Notes |
|---|---|---|
| Adjacency-list forest construction & cycle detection (`FlatNode`, `TreeNode::build_forest`) | **Implemented** | Rejects self-parents, missing parents, and attached or detached cycles |
| Pre-order & post-order pull iterators (`PreOrderIter`, `PostOrderIter`) | **Implemented** | Zero-allocation borrowing iterators |
| Re-parenting cycle guard (`would_create_cycle`, `creates_cycle`) | **Implemented** | Available both in memory and over SQL ancestor query results |
| Generic bottom-up rollups (`rollup`, `sum_rollup`, `checked_sum_rollup`) | **Implemented** | Generic over numeric types (`Add + Sum` and `CheckedAdd`) |
| Downward attribute inheritance & multi-axis precedence (`inherit_by`, `resolve_by_rank`) | **Implemented** | Deterministic resolution by `(axis_rank ASC, depth ASC, specificity DESC)` |
| Typed `WITH RECURSIVE` SQL traversals (`Hierarchy::descendants`, `Hierarchy::ancestors`) | **Implemented** | Supports discriminator scope, scope key or global visibility, active flag, soft-delete, max depth, and max breadth |
| SQLite & libSQL recursive CTE integration tests | **Implemented** | Tested in isolated binaries (`sql_sqlite`, `sql_libsql`) |
| PostgreSQL recursive CTE support | **Implemented** | Compile-checked against `diesel::pg::Pg` |
| Published crates.io dependency for `diesel-libsql` | **Planned** | Currently pinned to the git fork with `cte-ext` until upstream release |
| Live PostgreSQL integration test harness | **Planned** | Currently compile-checked; runtime execution covers SQLite and libSQL |

## Documentation

- [Architecture & Design Overview](docs/00_overview.md)
- [Recursive CTE Traversals](docs/cte_hierarchies.md)
- [Forest Rollups and Attribute Inheritance](docs/tree_aggregation.md)

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
