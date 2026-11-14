# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-14

### Added

- `arbor-tree`: database-free acyclic tree structures (`FlatNode`, `TreeNode`), forest validation (`build_forest`), pre-order and post-order iterators, path and subtree lookups, re-parenting cycle guard (`would_create_cycle`), generic bottom-up rollups (`rollup`, `sum_rollup`, `checked_sum_rollup` via `CheckedAdd`), downward attribute inheritance (`inherit_by`), and multi-axis rank resolution (`resolve_by_rank`).
- `arbor-diesel`: typed `WITH RECURSIVE` descendant and ancestor CTE traversals (`Hierarchy`, `Scope`, `ScopeKey`, `Direction`, `Traversal`) built on `diesel` and `diesel-cte-ext`, supporting SQLite, PostgreSQL, and libSQL backends with depth and breadth bounds, soft-delete and active-flag filters, and statement-cache safety.
