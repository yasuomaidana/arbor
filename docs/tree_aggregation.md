# Forest Rollups and Attribute Inheritance (`arbor-tree`)

Load an adjacency-list hierarchy once (for example with `arbor-diesel` or a batch query), build a validated forest with `TreeNode::build_forest`, and run aggregations or inheritance passes in memory.

## 1. Building and Validating a Forest

`TreeNode::build_forest(Vec<FlatNode<ID, T>>)` constructs the root `TreeNode`s and validates structural invariants:

- **Self-parenting rejected:** returns `Err(TreeError::SelfReferential)` if any node has `parent_id == Some(id)`.
- **Missing parent rejected:** returns `Err(TreeError::ParentNotFound)` if any `parent_id` does not exist in the input slice.
- **Cycles rejected:** returns `Err(TreeError::CycleDetected)` if any cycle is reachable from a root or exists as a detached cyclic component.

Each resulting `TreeNode<ID, T>` stores its `id`, `parent_id`, `data`, 0-based `depth`, and `children`.

## 2. Bottom-Up Rollups

- `rollup(&|data, child_results| ...)` performs a general post-order fold, passing a node's own payload `&T` and the `Vec<R>` produced by its direct children.
- `sum_rollup(&|data| value)` computes `value(node)` plus the sum across all descendants, generic over any numeric type `N: Add<Output = N> + Sum` (`i32`, `i64`, `u64`, `usize`, `f64`, etc.).
- `checked_sum_rollup(&|data| value)` computes the same subtree sum for any integer type implementing `CheckedAdd`, returning `None` if any addition overflows.

Example: computing the total byte size of a directory subtree:

```rust
use arbor_tree::{FlatNode, TreeNode};

let forest = TreeNode::build_forest(vec![
    FlatNode { id: "root", parent_id: None, data: 0_u64 },
    FlatNode { id: "src", parent_id: Some("root"), data: 4096_u64 },
    FlatNode { id: "lib.rs", parent_id: Some("src"), data: 12800_u64 },
])?;

assert_eq!(forest[0].checked_sum_rollup(&|bytes| *bytes), Some(16896_u64));
# Ok::<(), arbor_tree::TreeError>(())
```

## 3. Downward Inheritance and Multi-Axis Precedence

- `inherit_by(&forest, |data| slice)` returns a `HashMap<ID, Vec<A>>` containing each node's inherited ancestor attributes (ordered from root down) followed by its own attributes, deduplicated by first occurrence.
- `resolve_by_rank(candidates)` resolves competing `RankedCandidate<V>` entries per target key using deterministic tuple ordering `(axis_rank ASC, depth ASC, specificity DESC)`:
  1. Lower `axis_rank` wins (e.g., an explicit node override axis beats a category default axis).
  2. Closer `depth` wins (`0` = direct match beats `1` = parent match).
  3. Higher `specificity` wins when axis rank and depth are tied.

## 4. Traversal Iterators and Lookups

- `pre_order_iter()` and `post_order_iter()` return pull-based iterators over `&TreeNode<ID, T>` references without copying node payloads.
- `find_node(&target_id)` searches a subtree for `target_id`.
- `collect_descendant_ids()` returns all IDs in the subtree in pre-order.
- `path_to(&target_id)` returns the `Vec<ID>` path from the subtree root down to `target_id` (both inclusive), or `None` if `target_id` is not present.
