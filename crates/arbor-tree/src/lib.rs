//! # arbor-tree
//!
//! General-purpose acyclic trees, independent of any storage backend or application domain:
//!
//! 1. **In-memory forests** ([`FlatNode`], [`TreeNode`]): build and validate a forest from
//!    adjacency-list rows, iterate in pre-order or post-order, find nodes, collect subtrees, and
//!    compute the path from a root.
//! 2. **Cycle guards** ([`would_create_cycle`]) for re-parenting checks before a write.
//! 3. **Rollups** ([`TreeNode::rollup`], [`TreeNode::sum_rollup`],
//!    [`TreeNode::checked_sum_rollup`]): generic bottom-up folds and numeric subtree sums.
//! 4. **Inheritance & Precedence** ([`inherit_by`], [`resolve_by_rank`]): the attributes each node
//!    inherits from its ancestors, and multi-axis winning attribute resolution by rank, depth, and
//!    specificity.

pub mod error;
pub mod inherit;
pub mod node;

pub use error::TreeError;
pub use inherit::{RankedCandidate, inherit_by, resolve_by_rank};
pub use node::{CheckedAdd, FlatNode, PostOrderIter, PreOrderIter, TreeNode, would_create_cycle};
