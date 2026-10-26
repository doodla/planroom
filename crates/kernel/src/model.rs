//! What the fold reads and what it returns (architecture.md, "The fold"): the facts of the
//! active snapshots, and the model at one `seq`.

use planroom_contract::{Edge, Hint, Node, Seq};

/// The facts a fold reads: the nodes, edges and hints of the active snapshots and the workspace
/// link snapshot, as the extractors and linkers wrote them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub hints: Vec<Hint>,
}

/// The model at one `seq`: every node once, sorted by ID, and the edges between them. Views,
/// questions and diffs read it; nothing stores it (DR-2).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    /// The `seq` the model was folded at.
    pub at: Seq,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}
