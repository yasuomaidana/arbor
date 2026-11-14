//! Error types for hierarchical and acyclic tree operations.

use serde::{Deserialize, Serialize};
use thiserror::Error;
#[cfg(feature = "to-typescript")]
use ts_rs::TS;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "to-typescript", derive(TS))]
#[cfg_attr(feature = "to-typescript", ts(export, export_to = "tree/"))]
pub enum TreeError {
    #[error(
        "Cycle detected in hierarchy: node {node_id} is an ancestor of proposed parent {parent_id}"
    )]
    CycleDetected { node_id: String, parent_id: String },

    #[error("Node not found: {node_id}")]
    NodeNotFound { node_id: String },

    #[error("Parent node not found: {parent_id}")]
    ParentNotFound { parent_id: String },

    #[error("Invalid hierarchy: {message}")]
    InvalidHierarchy { message: String },

    #[error("Self-referential node: node {node_id} cannot have itself as parent")]
    SelfReferential { node_id: String },

    #[error("Database error in tree query: {message}")]
    Database { message: String },
}
