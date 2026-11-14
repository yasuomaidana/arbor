# arbor-tree

Database-free acyclic trees and forest algorithms for Rust. `arbor-tree` operates on any adjacency-list hierarchy (`id` plus nullable `parent_id`): category taxonomies, file trees, org charts, and component trees.

## Capabilities

| Area | API | Feature |
|---|---|---|
| **Forests** | `FlatNode`, `TreeNode`, `TreeNode::build_forest` (rejects self-parents, missing parents, and cycles), `find_node`, `collect_descendant_ids`, `path_to` | always |
| **Iterators** | `PreOrderIter` (`pre_order_iter`), `PostOrderIter` (`post_order_iter`) | always |
| **Cycle guard** | `would_create_cycle(pairs, node_id, new_parent_id)` before a re-parenting mutation | always |
| **Rollups** | `rollup` (generic bottom-up fold), `sum_rollup` (generic over `Add + Sum`), `checked_sum_rollup` (generic over `CheckedAdd`, returns `None` on overflow) | always |
| **Inheritance & Precedence** | `inherit_by(forest, attrs)` and `resolve_by_rank(candidates)` (`RankedCandidate`) | always |
| **TypeScript bindings** | `ts-rs` derives on `FlatNode`, `TreeNode`, and `TreeError` | `to-typescript` |

## Quick Example

```rust
use arbor_tree::{FlatNode, TreeNode, inherit_by, would_create_cycle};

let flat = vec![
    FlatNode { id: "root", parent_id: None, data: 10_i64 },
    FlatNode { id: "child_a", parent_id: Some("root"), data: 25_i64 },
    FlatNode { id: "child_b", parent_id: Some("root"), data: 15_i64 },
];

let forest = TreeNode::build_forest(flat)?;
assert_eq!(forest[0].sum_rollup(&|v| *v), 50);
assert_eq!(forest[0].checked_sum_rollup(&|v| *v), Some(50));

let pairs = vec![("root", None), ("child_a", Some("root"))];
assert!(would_create_cycle(&pairs, &"root", &Some("child_a")));
# Ok::<(), arbor_tree::TreeError>(())
```
