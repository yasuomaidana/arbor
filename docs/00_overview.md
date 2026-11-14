# Architecture & Design Overview

`arbor` provides general-purpose, application-agnostic acyclic tree structures and recursive SQL traversal queries for Rust. It is split into two focused crates so that in-memory tree algorithms can be used without pulling in database dependencies:

1. [`arbor-tree`](../crates/arbor-tree): Database-free in-memory forest construction, cycle detection, pre-order and post-order iterators, generic bottom-up rollups, downward attribute inheritance, and multi-axis rank resolution.
2. [`arbor-diesel`](../crates/arbor-diesel): Typed `WITH RECURSIVE` descendant and ancestor Common Table Expression (CTE) queries for adjacency-list tables on Diesel (`sqlite`, `postgres`, and `libsql`), built on [`diesel-cte-ext`](https://crates.io/crates/diesel-cte-ext).

---

## Design Decisions

### 1. Adjacency List as the Canonical Input Representation

Hierarchies in relational databases and configuration files are most naturally represented as adjacency lists: each node carries its own `id` and an optional `parent_id` (`None` / `NULL` for root nodes).

- **In memory (`arbor-tree`):** [`FlatNode<ID, T>`] represents a single adjacency-list record with arbitrary payload `T`. [`TreeNode::build_forest`] converts a flat vector into a validated multi-root forest (`Vec<TreeNode<ID, T>>`), rejecting self-referential nodes, dangling parent references, and cycles (including detached cycle components).
- **In SQL (`arbor-diesel`):** [`Hierarchy`] describes an adjacency-list table via compile-time `&'static str` table and column names (`id`, `parent`) plus optional row filters (`scoped`, `scope_key_or_global`, `active_when_true`, `live_when_null`) and recursion bounds (`max_depth`, `max_breadth`).

### 2. Two-Layer Cycle Prevention

Tree mutations (such as moving a folder, category, or org-chart unit under a new parent) must preserve acyclicity:

1. **Pre-write cycle guards:**
   - In memory: `would_create_cycle(all_parent_pairs, node_id, new_parent_id)` walks the proposed parent's ancestor chain and returns `true` if `node_id` is encountered or if existing data contains a cycle.
   - Against the database: load `hierarchy.ancestors(new_parent_id)` and call `creates_cycle(node_id, &ancestor_rows)`.
2. **Read-time termination bounds:**
   - Every SQL traversal binds a `max_depth` limit (default `64`) in the recursive step (`WHERE c.depth < $max_depth`), guaranteeing termination even if cyclic rows exist in storage.
   - `reached_depth_limit(&hierarchy, &rows)` inspects the returned rows to report whether the depth ceiling was reached.

### 3. Single-Pass Loading and In-Memory Rollups

Recursive CTE queries load an entire subtree or ancestor chain in a single round-trip, avoiding `N + 1` per-level `SELECT` statements. Once rows are loaded and assembled via `TreeNode::build_forest`:

- `TreeNode::rollup` performs a generic post-order fold over each node's payload and its children's accumulated results.
- `TreeNode::sum_rollup` and `TreeNode::checked_sum_rollup` aggregate any numeric metric (file sizes, item counts, weights) across a subtree, with `checked_sum_rollup` using [`CheckedAdd`] to return `None` on arithmetic overflow.
- `inherit_by` propagates attributes from roots down to descendants without duplicates.
- `resolve_by_rank` deterministically selects a winning attribute across multiple candidate axes ordered by `(axis_rank ASC, depth ASC, specificity DESC)`.

---

## Further Reading

- [Recursive CTE Traversals (`arbor-diesel`)](cte_hierarchies.md)
- [Forest Rollups and Attribute Inheritance (`arbor-tree`)](tree_aggregation.md)
