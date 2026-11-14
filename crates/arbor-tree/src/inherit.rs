//! Attribute inheritance down a forest and multi-axis precedence resolution.

use std::collections::HashMap;
use std::hash::Hash;

use crate::node::TreeNode;

/// Returns, for every node in `forest`, its own attributes plus those of all its ancestors.
///
/// `attributes` extracts a node's own attributes from its data. The result for each node lists
/// inherited attributes from the root downwards, then the node's own, without duplicates
/// (the first occurrence wins, so a root-level attribute keeps its position).
///
/// ```
/// use arbor_tree::{FlatNode, TreeNode, inherit_by};
///
/// // Root division -> team: the child node inherits its parent's attributes.
/// let forest = TreeNode::build_forest(vec![
///     FlatNode { id: "engineering", parent_id: None, data: vec!["internal"] },
///     FlatNode { id: "platform", parent_id: Some("engineering"), data: vec!["on-call"] },
/// ])?;
/// let effective = inherit_by(&forest, |labels: &Vec<&str>| labels.as_slice());
/// assert_eq!(effective[&"platform"], vec!["internal", "on-call"]);
/// # Ok::<(), arbor_tree::TreeError>(())
/// ```
pub fn inherit_by<ID, T, A, F>(forest: &[TreeNode<ID, T>], attributes: F) -> HashMap<ID, Vec<A>>
where
    ID: Clone + Eq + Hash,
    A: Clone + PartialEq,
    F: Fn(&T) -> &[A],
{
    let mut effective = HashMap::new();
    for root in forest {
        walk(root, &[], &attributes, &mut effective);
    }
    effective
}

fn walk<ID, T, A, F>(
    node: &TreeNode<ID, T>,
    inherited: &[A],
    attributes: &F,
    out: &mut HashMap<ID, Vec<A>>,
) where
    ID: Clone + Eq + Hash,
    A: Clone + PartialEq,
    F: Fn(&T) -> &[A],
{
    let mut own = inherited.to_vec();
    for attribute in attributes(&node.data) {
        if !own.contains(attribute) {
            own.push(attribute.clone());
        }
    }
    for child in &node.children {
        walk(child, &own, attributes, out);
    }
    out.insert(node.id.clone(), own);
}

/// A candidate for multi-axis attribute resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedCandidate<V> {
    /// Axis rank: 0 is highest precedence, then 1, 2, ...
    pub axis_rank: u32,
    /// Distance from target entity (0 = direct, 1 = parent, etc.). Lower is closer.
    pub depth: u32,
    /// Specificity score (higher = more specific rule).
    pub specificity: u32,
    /// The resolved attribute or value.
    pub candidate: V,
}

impl<V> RankedCandidate<V> {
    const fn precedence_key(&self) -> (u32, u32, std::cmp::Reverse<u32>) {
        (
            self.axis_rank,
            self.depth,
            std::cmp::Reverse(self.specificity),
        )
    }
}

impl<V: Eq> PartialOrd for RankedCandidate<V> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<V: Eq> Ord for RankedCandidate<V> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.precedence_key().cmp(&other.precedence_key())
    }
}

/// Generic multi-axis precedence resolution.
///
/// Given candidate attributes for keys (e.g. entities or targets), deterministically
/// picks the winning attribute by `(axis_rank ASC, depth ASC, specificity DESC)`.
pub fn resolve_by_rank<K, V>(
    candidates: impl IntoIterator<Item = (K, RankedCandidate<V>)>,
) -> HashMap<K, V>
where
    K: Eq + Hash,
    V: Clone,
{
    let mut best: HashMap<K, RankedCandidate<V>> = HashMap::new();
    for (k, c) in candidates {
        match best.entry(k) {
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(c);
            }
            std::collections::hash_map::Entry::Occupied(mut e) => {
                if c.precedence_key() < e.get().precedence_key() {
                    e.insert(c);
                }
            }
        }
    }
    best.into_iter().map(|(k, c)| (k, c.candidate)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::FlatNode;
    use rstest::rstest;

    #[rstest]
    fn inherits_through_every_level_without_duplicates() {
        let forest = TreeNode::build_forest(vec![
            FlatNode {
                id: "media",
                parent_id: None,
                data: vec!["digital"],
            },
            FlatNode {
                id: "audio",
                parent_id: Some("media"),
                data: vec!["streamable"],
            },
            FlatNode {
                id: "podcast",
                parent_id: Some("audio"),
                data: vec!["streamable", "spoken"],
            },
            FlatNode {
                id: "archive",
                parent_id: None,
                data: vec![],
            },
        ])
        .expect("valid forest");

        let effective = inherit_by(&forest, |data: &Vec<&str>| data.as_slice());

        assert_eq!(effective[&"media"], vec!["digital"]);
        assert_eq!(
            effective[&"podcast"],
            vec!["digital", "streamable", "spoken"]
        );
        assert!(effective[&"archive"].is_empty());
    }

    #[rstest]
    #[case::axis_rank_wins((1, 0, 5, "default_rule"), (0, 0, 1, "override_rule"), "override_rule")]
    #[case::closer_depth_wins((1, 2, 10, "ancestor_rule"), (1, 0, 1, "direct_rule"), "direct_rule")]
    #[case::higher_specificity_wins((1, 0, 2, "low_specificity"), (1, 0, 5, "high_specificity"), "high_specificity")]
    fn test_resolve_by_rank_precedence(
        #[case] first: (u32, u32, u32, &'static str),
        #[case] second: (u32, u32, u32, &'static str),
        #[case] expected: &'static str,
    ) {
        let to_candidate = |(axis_rank, depth, specificity, candidate)| RankedCandidate {
            axis_rank,
            depth,
            specificity,
            candidate,
        };
        let candidates = vec![
            ("node", to_candidate(first)),
            ("node", to_candidate(second)),
            ("node", to_candidate(first)),
        ];
        let resolved = resolve_by_rank(candidates);
        assert_eq!(resolved.get("node"), Some(&expected));
        assert!(to_candidate(second) < to_candidate(first));
    }
}
