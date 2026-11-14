# Changelog

All notable changes to `arbor-tree` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-14

### Added

- `FlatNode` and `TreeNode` in-memory representations and `TreeNode::build_forest` with self-parent, missing-parent, and cycle validation.
- `PreOrderIter` and `PostOrderIter` pull-based tree iterators, `find_node`, `collect_descendant_ids`, and `path_to`.
- `would_create_cycle` re-parenting cycle guard.
- Generic `rollup`, `sum_rollup`, and `checked_sum_rollup` (`CheckedAdd`) bottom-up aggregations.
- `inherit_by` downward attribute inheritance and `resolve_by_rank` (`RankedCandidate`) multi-axis precedence resolution.
