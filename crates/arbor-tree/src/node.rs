//! In-memory generic acyclic tree structures and algorithms.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::Sum;
use std::ops::Add;
#[cfg(feature = "to-typescript")]
use ts_rs::TS;

use crate::error::TreeError;

/// Numeric addition that returns `None` on overflow.
pub trait CheckedAdd: Sized {
    /// Adds `self` and `rhs`, returning `None` if overflow occurs.
    fn checked_add(self, rhs: Self) -> Option<Self>;
}

macro_rules! impl_checked_add {
    ($($t:ty),+ $(,)?) => {
        $(
            impl CheckedAdd for $t {
                #[inline]
                fn checked_add(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_add(self, rhs)
                }
            }
        )+
    };
}

impl_checked_add!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

/// Flat representation of a hierarchical entity before tree construction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "to-typescript", derive(TS))]
#[cfg_attr(feature = "to-typescript", ts(export, export_to = "tree/"))]
pub struct FlatNode<ID, T> {
    pub id: ID,
    pub parent_id: Option<ID>,
    pub data: T,
}

/// A node in a multi-way acyclic tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "to-typescript", derive(TS))]
#[cfg_attr(feature = "to-typescript", ts(export, export_to = "tree/"))]
pub struct TreeNode<ID, T> {
    pub id: ID,
    pub parent_id: Option<ID>,
    pub data: T,
    pub depth: usize,
    pub children: Vec<TreeNode<ID, T>>,
}

impl<ID, T> TreeNode<ID, T>
where
    ID: Clone + Eq + Hash + Debug + ToString,
{
    /// Construct a forest (list of root trees) from a flat list of nodes, validating acyclicity.
    pub fn build_forest(
        flat_nodes: Vec<FlatNode<ID, T>>,
    ) -> Result<Vec<TreeNode<ID, T>>, TreeError> {
        let all_ids: HashSet<ID> = flat_nodes.iter().map(|n| n.id.clone()).collect();
        Self::validate_flat_nodes(&flat_nodes, &all_ids)?;

        let mut children_map: HashMap<Option<ID>, Vec<FlatNode<ID, T>>> = HashMap::new();
        for node in flat_nodes {
            children_map
                .entry(node.parent_id.clone())
                .or_default()
                .push(node);
        }

        let mut visited = HashSet::new();
        let mut roots = Vec::new();

        if let Some(root_nodes) = children_map.remove(&None) {
            for root_flat in root_nodes {
                let tree = Self::build_subtree(root_flat, &mut children_map, 0, &mut visited)?;
                roots.push(tree);
            }
        }

        if visited.len() < all_ids.len()
            && let Some(first) = all_ids.difference(&visited).next()
        {
            return Err(TreeError::CycleDetected {
                node_id: first.to_string(),
                parent_id: "cycle_component".to_string(),
            });
        }

        Ok(roots)
    }

    fn validate_flat_nodes(
        flat_nodes: &[FlatNode<ID, T>],
        all_ids: &HashSet<ID>,
    ) -> Result<(), TreeError> {
        for node in flat_nodes {
            if let Some(parent) = &node.parent_id
                && parent == &node.id
            {
                return Err(TreeError::SelfReferential {
                    node_id: node.id.to_string(),
                });
            }
        }

        for node in flat_nodes {
            if let Some(parent) = &node.parent_id
                && !all_ids.contains(parent)
            {
                return Err(TreeError::ParentNotFound {
                    parent_id: parent.to_string(),
                });
            }
        }

        Ok(())
    }

    fn build_subtree(
        current: FlatNode<ID, T>,
        children_map: &mut HashMap<Option<ID>, Vec<FlatNode<ID, T>>>,
        depth: usize,
        visited: &mut HashSet<ID>,
    ) -> Result<TreeNode<ID, T>, TreeError> {
        if !visited.insert(current.id.clone()) {
            return Err(TreeError::CycleDetected {
                node_id: current.id.to_string(),
                parent_id: current
                    .parent_id
                    .as_ref()
                    .map(|p| p.to_string())
                    .unwrap_or_default(),
            });
        }

        let mut children = Vec::new();
        if let Some(child_flats) = children_map.remove(&Some(current.id.clone())) {
            for child_flat in child_flats {
                let child_tree = Self::build_subtree(child_flat, children_map, depth + 1, visited)?;
                children.push(child_tree);
            }
        }

        Ok(TreeNode {
            id: current.id,
            parent_id: current.parent_id,
            data: current.data,
            depth,
            children,
        })
    }

    /// Returns a pre-order (root-first) pull-based iterator over references to nodes in this subtree.
    pub fn pre_order_iter(&self) -> PreOrderIter<'_, ID, T> {
        PreOrderIter { stack: vec![self] }
    }

    /// Returns a post-order (children-first) pull-based iterator over references to nodes in this subtree.
    pub fn post_order_iter(&self) -> PostOrderIter<'_, ID, T> {
        PostOrderIter {
            stack: vec![(self, 0)],
        }
    }

    /// Collect all descendant IDs in preorder traversal.
    pub fn collect_descendant_ids(&self) -> Vec<ID> {
        let mut result = Vec::new();
        self.collect_ids_recursive(&mut result);
        result
    }

    fn collect_ids_recursive(&self, acc: &mut Vec<ID>) {
        acc.push(self.id.clone());
        for child in &self.children {
            child.collect_ids_recursive(acc);
        }
    }

    /// Search for a node by ID in this subtree.
    pub fn find_node(&self, target_id: &ID) -> Option<&TreeNode<ID, T>> {
        if &self.id == target_id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_node(target_id) {
                return Some(found);
            }
        }
        None
    }

    /// Bottom-up aggregation rollup (e.g. summing file sizes or subtree item counts).
    pub fn rollup<R, F>(&self, rollup_fn: &F) -> R
    where
        F: Fn(&T, Vec<R>) -> R,
    {
        let child_results: Vec<R> = self.children.iter().map(|c| c.rollup(rollup_fn)).collect();
        rollup_fn(&self.data, child_results)
    }

    /// Bottom-up numeric sum rollup over a subtree.
    pub fn sum_rollup<N, F>(&self, value_fn: &F) -> N
    where
        N: Add<Output = N> + Sum,
        F: Fn(&T) -> N,
    {
        self.rollup(&|data, child_sums: Vec<N>| value_fn(data) + child_sums.into_iter().sum::<N>())
    }

    /// Like [`sum_rollup`](Self::sum_rollup), but returns `None` instead of overflowing.
    pub fn checked_sum_rollup<N, F>(&self, value_fn: &F) -> Option<N>
    where
        N: CheckedAdd,
        F: Fn(&T) -> N,
    {
        self.rollup(&|data, child_sums: Vec<Option<N>>| {
            child_sums
                .into_iter()
                .try_fold(value_fn(data), |acc, child| acc.checked_add(child?))
        })
    }

    /// The IDs from this node down to `target_id` (both inclusive), or `None` if `target_id`
    /// is not in this subtree.
    pub fn path_to(&self, target_id: &ID) -> Option<Vec<ID>> {
        if &self.id == target_id {
            return Some(vec![self.id.clone()]);
        }
        self.children.iter().find_map(|child| {
            child.path_to(target_id).map(|mut path| {
                path.insert(0, self.id.clone());
                path
            })
        })
    }
}

/// Zero-allocation pull-based pre-order iterator over references to nodes in a `TreeNode`.
pub struct PreOrderIter<'a, ID, T> {
    stack: Vec<&'a TreeNode<ID, T>>,
}

impl<'a, ID, T> Iterator for PreOrderIter<'a, ID, T> {
    type Item = &'a TreeNode<ID, T>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        for child in node.children.iter().rev() {
            self.stack.push(child);
        }
        Some(node)
    }
}

/// Zero-allocation pull-based post-order iterator over references to nodes in a `TreeNode`.
pub struct PostOrderIter<'a, ID, T> {
    stack: Vec<(&'a TreeNode<ID, T>, usize)>,
}

impl<'a, ID, T> Iterator for PostOrderIter<'a, ID, T> {
    type Item = &'a TreeNode<ID, T>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (node, next_child) = self.stack.pop()?;
            if next_child < node.children.len() {
                self.stack.push((node, next_child + 1));
                self.stack.push((&node.children[next_child], 0));
            } else {
                return Some(node);
            }
        }
    }
}

/// Helper function to verify if adding/moving a `node_id` under `new_parent_id` would create a cycle.
pub fn would_create_cycle<ID>(
    all_parent_pairs: &[(ID, Option<ID>)],
    node_id: &ID,
    new_parent_id: &Option<ID>,
) -> bool
where
    ID: Clone + Eq + Hash,
{
    let Some(target_parent) = new_parent_id else {
        return false; // Moving to root can never create a cycle
    };

    if node_id == target_parent {
        return true; // Self-parenting is an instant cycle
    }

    let mut parent_lookup: HashMap<ID, Option<ID>> = HashMap::new();
    for (id, parent) in all_parent_pairs {
        parent_lookup.insert(id.clone(), parent.clone());
    }

    let mut curr = Some(target_parent.clone());
    let mut visited = HashSet::new();

    while let Some(parent) = curr {
        if &parent == node_id {
            return true;
        }
        if !visited.insert(parent.clone()) {
            return true;
        }
        curr = parent_lookup.get(&parent).cloned().flatten();
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_build_tree_and_sum_rollup() {
        // Hierarchy:
        // root_dir (0)
        //   ├── src_dir (100)
        //   │     ├── lib_rs (50)
        //   │     └── node_rs (150)
        //   └── docs_dir (300)

        struct FileNodeData {
            #[allow(dead_code)]
            label: &'static str,
            size_bytes: i64,
        }

        let flat_nodes = vec![
            FlatNode {
                id: "root_dir",
                parent_id: None,
                data: FileNodeData {
                    label: "root",
                    size_bytes: 0,
                },
            },
            FlatNode {
                id: "src_dir",
                parent_id: Some("root_dir"),
                data: FileNodeData {
                    label: "src",
                    size_bytes: 100,
                },
            },
            FlatNode {
                id: "lib_rs",
                parent_id: Some("src_dir"),
                data: FileNodeData {
                    label: "lib.rs",
                    size_bytes: 50,
                },
            },
            FlatNode {
                id: "node_rs",
                parent_id: Some("src_dir"),
                data: FileNodeData {
                    label: "node.rs",
                    size_bytes: 150,
                },
            },
            FlatNode {
                id: "docs_dir",
                parent_id: Some("root_dir"),
                data: FileNodeData {
                    label: "docs",
                    size_bytes: 300,
                },
            },
        ];

        let forest = TreeNode::build_forest(flat_nodes).unwrap();
        assert_eq!(forest.len(), 1);

        let root = &forest[0];
        assert_eq!(root.id, "root_dir");
        assert_eq!(root.children.len(), 2);

        // Sum rollup: total size = 0 + 100 + 50 + 150 + 300 = 600
        let total = root.sum_rollup(&|node| node.size_bytes);
        assert_eq!(total, 600);

        // Generic over unsigned and floating-point numeric types as well
        let total_u64: u64 = root.sum_rollup(&|node| node.size_bytes as u64);
        assert_eq!(total_u64, 600_u64);

        // Find node in subtree
        assert_eq!(
            root.find_node(&"lib_rs").map(|n| n.data.size_bytes),
            Some(50)
        );
        assert!(root.find_node(&"nonexistent").is_none());

        // Descendant IDs
        let ids = root.collect_descendant_ids();
        assert_eq!(
            ids,
            vec!["root_dir", "src_dir", "lib_rs", "node_rs", "docs_dir"]
        );
    }

    #[rstest]
    fn test_pre_and_post_order_iter() {
        let flat_nodes = vec![
            FlatNode {
                id: "root",
                parent_id: None,
                data: (),
            },
            FlatNode {
                id: "a",
                parent_id: Some("root"),
                data: (),
            },
            FlatNode {
                id: "a1",
                parent_id: Some("a"),
                data: (),
            },
            FlatNode {
                id: "a2",
                parent_id: Some("a"),
                data: (),
            },
            FlatNode {
                id: "b",
                parent_id: Some("root"),
                data: (),
            },
        ];

        let forest = TreeNode::build_forest(flat_nodes).unwrap();
        let root = &forest[0];

        let pre_order: Vec<&str> = root.pre_order_iter().map(|n| n.id).collect();
        assert_eq!(pre_order, vec!["root", "a", "a1", "a2", "b"]);

        let post_order: Vec<&str> = root.post_order_iter().map(|n| n.id).collect();
        assert_eq!(post_order, vec!["a1", "a2", "a", "b", "root"]);
    }

    #[rstest]
    #[case::detached_cycle(
        vec![("A", Some("B")), ("B", Some("A"))],
        TreeError::CycleDetected { node_id: "A".into(), parent_id: "cycle_component".into() }
    )]
    #[case::self_referential(
        vec![("A", Some("A"))],
        TreeError::SelfReferential { node_id: "A".into() }
    )]
    #[case::parent_not_found(
        vec![("A", Some("missing"))],
        TreeError::ParentNotFound { parent_id: "missing".into() }
    )]
    fn test_invalid_forest_errors(
        #[case] pairs: Vec<(&'static str, Option<&'static str>)>,
        #[case] expected_err: TreeError,
    ) {
        let flat_nodes = pairs
            .into_iter()
            .map(|(id, parent_id)| FlatNode {
                id,
                parent_id,
                data: (),
            })
            .collect();
        let err = TreeNode::build_forest(flat_nodes).unwrap_err();
        if matches!(err, TreeError::CycleDetected { .. }) {
            assert!(matches!(expected_err, TreeError::CycleDetected { .. }));
        } else {
            assert_eq!(err, expected_err);
        }
    }

    #[rstest]
    #[case("root", Some("child"), true)]
    #[case("child", Some("root"), false)]
    #[case("child", None, false)]
    #[case("root", Some("root"), true)]
    #[case("outside", Some("cycle_a"), true)]
    fn test_would_create_cycle(
        #[case] node_id: &str,
        #[case] new_parent: Option<&str>,
        #[case] expected: bool,
    ) {
        let tree_pairs = vec![
            ("root", None),
            ("parent", Some("root")),
            ("child", Some("parent")),
            ("cycle_a", Some("cycle_b")),
            ("cycle_b", Some("cycle_a")),
        ];
        assert_eq!(
            would_create_cycle(&tree_pairs, &node_id, &new_parent),
            expected
        );
    }

    #[rstest]
    #[case(i64::MAX, 1_i64, None)]
    #[case(2_i64, 3_i64, Some(5_i64))]
    fn test_checked_sum_rollup_detects_overflow(
        #[case] root_val: i64,
        #[case] leaf_val: i64,
        #[case] expected: Option<i64>,
    ) {
        let forest = TreeNode::build_forest(vec![
            FlatNode {
                id: "root",
                parent_id: None,
                data: root_val,
            },
            FlatNode {
                id: "leaf",
                parent_id: Some("root"),
                data: leaf_val,
            },
        ])
        .unwrap();
        assert_eq!(forest[0].checked_sum_rollup(&|value| *value), expected);
    }

    #[rstest]
    fn test_path_to() {
        let forest = TreeNode::build_forest(vec![
            FlatNode {
                id: "computing",
                parent_id: None,
                data: (),
            },
            FlatNode {
                id: "laptops",
                parent_id: Some("computing"),
                data: (),
            },
            FlatNode {
                id: "ultrabooks",
                parent_id: Some("laptops"),
                data: (),
            },
            FlatNode {
                id: "desktops",
                parent_id: Some("computing"),
                data: (),
            },
        ])
        .unwrap();
        assert_eq!(
            forest[0].path_to(&"ultrabooks"),
            Some(vec!["computing", "laptops", "ultrabooks"])
        );
        assert_eq!(forest[0].path_to(&"missing"), None);
    }
}
