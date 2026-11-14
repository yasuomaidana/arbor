# Recursive CTE Traversals (`arbor-diesel`)

## 1. Hierarchy Descriptor Model

A `Hierarchy` describes one adjacency-list table using compile-time `&'static str` identifiers:

| Field | Meaning |
|---|---|
| `table`, `id`, `parent` | Table name, primary-key column, and nullable parent column (`&'static str`) |
| `scope: Option<Scope>` | Discriminator equality filter applied at every level, e.g. `domain = 'catalog'` for a table storing multiple category trees |
| `scope_key: Option<ScopeKey>` | Scope-key visibility filter (`column IS NULL OR column = $scope_key`, or `column IS NULL` when `None`) |
| `active_column: Option<&str>` | Boolean flag column; only rows where `column IS TRUE` are traversed |
| `live_column: Option<&str>` | Soft-delete timestamp column; only rows where `column IS NULL` are traversed |
| `max_depth: i32` | Recursion depth ceiling (default `64`; the seed row has depth `0`) |
| `max_breadth: Option<i32>` | Optional per-parent child limit using `ROW_NUMBER() OVER (PARTITION BY parent ORDER BY id)` |

Construct a `Hierarchy` using `const` builder methods:

```rust,ignore
use arbor_diesel::sql::Hierarchy;

const CATALOG: Hierarchy = Hierarchy::new("category_nodes", "id", "parent_id")
    .scoped("domain", "catalog")
    .scope_key_or_global("scope_key", Some("scope_a"))
    .active_when_true("is_active")
    .live_when_null("deleted_at")
    .max_depth(32)
    .max_breadth(100);
```

## 2. Generated SQL Structure

Calling `descendants::<DB, ST, K>(start)` emits backend-quoted identifiers and bound parameters (`?` or `$n`):

```sql
WITH RECURSIVE "arbor_tree" ("node_id", "parent_id", "depth") AS (
    SELECT "id", "parent_id", 0 FROM "category_nodes"
     WHERE "id" = ? AND "domain" = ? AND "deleted_at" IS NULL
    UNION ALL
    SELECT t."id", t."parent_id", c."depth" + 1
      FROM "category_nodes" t INNER JOIN "arbor_tree" c ON t."parent_id" = c."node_id"
     WHERE c."depth" < ? AND t."domain" = ? AND t."deleted_at" IS NULL
)
SELECT "node_id", "parent_id", "depth" FROM "arbor_tree" ORDER BY "depth"
```

Calling `ancestors::<DB, ST, K>(start)` differs only in the recursive join condition: `ON t."id" = c."parent_id"`. Both queries load as `Vec<(K, Option<K>, i32)>`, where `ST` is the key's Diesel SQL type (such as `diesel::sql_types::Text`) and `K` is the corresponding Rust key type.

The `WITH RECURSIVE` envelope is assembled by `diesel_cte_ext::builders::with_recursive`; `arbor-diesel` supplies the typed `Seed`, `Step`, and `Body` query fragments. Because the SQL depends on the runtime `Hierarchy` configuration, `Seed` and `Step` mark themselves unsafe to cache in Diesel's type-indexed prepared-statement cache.

## 3. Cycle Safety in Two Layers

1. **Before writing:** call `arbor_tree::would_create_cycle(pairs, node, new_parent)` in memory, or load `hierarchy.ancestors(new_parent)` and check `arbor_diesel::creates_cycle(node, &rows)`.
2. **While reading:** `max_depth` guarantees termination even if cyclic data exists in the table, and `reached_depth_limit(&hierarchy, &rows)` reports whether any row reached `max_depth`.

Database schemas should also enforce `CHECK (parent_id <> id)` and `ON DELETE RESTRICT` on the parent foreign key.

## 4. Supported Backends

| Feature | Backend Type | Status |
|---|---|---|
| `sqlite` | `diesel::sqlite::Sqlite` | Integration-tested (`tests/sql_sqlite.rs`) |
| `libsql` | `diesel_libsql::LibSql` | Integration-tested (`tests/sql_libsql.rs`) |
| `postgres` | `diesel::pg::Pg` | Compile-checked (uses identical standard SQL) |

Plain `diesel::sqlite` and `diesel_libsql` connections link against the same `sqlite3_*` symbol namespace and must not be opened in the same OS process; integration tests isolate each backend in its own test binary.
