//! The parent tree a query walks, read once: which node holds which, each node's block at a level,
//! and the ends an edge lifts to, all in constant or logarithmic time per lookup, so no query
//! costs more than a pass over the model's nodes and edges, however deep the tree.
//!
//! Holding is by a depth-first numbering of the parent tree: a node holds another when the other's
//! number falls inside its own subtree's range. Climbing uses jump pointers (each node keeps its
//! parent and one ancestor further up, picked so that any climb takes a logarithmic number of
//! steps), so finding the ancestor at a given depth never walks a deep chain one parent at a time.
//! A parent cycle, which only bad facts or an older store can hold, is cut where the numbering
//! first meets it, so every node has one root.
//!
//! Levels are compared by their rank on the ladder the forest is built with: each node's rank is
//! read once, and a level's blocks are kept under its rank.

use std::collections::HashMap;

use planroom_contract::{Edge, Node};

use crate::ladder::Ladder;
use crate::resolve::Index;

/// The parent tree of a model's nodes, numbered for holding and climbing.
pub struct Forest<'a> {
    pub nodes: &'a [Node],
    index: Index<'a>,
    by_id: HashMap<&'a str, usize>,
    parent: Vec<Option<usize>>,
    jump: Vec<usize>,
    depth: Vec<u32>,
    /// Depth-first entry number, and one past the last number in the node's subtree.
    enter: Vec<u32>,
    leave: Vec<u32>,
    /// Each node's level's rank on the ladder.
    rank: Vec<u8>,
    /// Each node's block at a level, by the level's rank, filled on first use.
    blocks: HashMap<u8, Vec<usize>>,
    /// The nodes in depth-first order: every parent before its children.
    order: Vec<usize>,
}

impl<'a> Forest<'a> {
    /// The forest of `nodes`, whose levels `ladder` ranks.
    pub fn new(nodes: &'a [Node], ladder: &Ladder<'_>) -> Forest<'a> {
        let by_id: HashMap<&str, usize> =
            nodes.iter().enumerate().map(|(i, n)| (n.id.0.as_str(), i)).collect();
        let n = nodes.len();
        let named: Vec<Option<usize>> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                node.parent.as_ref().and_then(|p| by_id.get(p.0.as_str()).copied()).filter(|p| *p != i)
            })
            .collect();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, p) in named.iter().enumerate() {
            if let Some(p) = p {
                children[*p].push(i);
            }
        }
        let mut forest = Forest {
            nodes,
            index: Index::new(nodes),
            by_id,
            parent: vec![None; n],
            jump: (0..n).collect(),
            depth: vec![0; n],
            enter: vec![0; n],
            leave: vec![0; n],
            rank: nodes.iter().map(|n| ladder.rank(&n.level)).collect(),
            blocks: HashMap::new(),
            order: Vec::with_capacity(n),
        };
        let mut seen = vec![false; n];
        // Roots first, in the model's order; then any node a parent cycle left unreached, as the
        // root of what remains of its cycle.
        let roots = (0..n).filter(|i| named[*i].is_none()).chain(0..n);
        let mut clock = 0u32;
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for root in roots {
            if seen[root] {
                continue;
            }
            seen[root] = true;
            forest.place(root, None, clock);
            clock += 1;
            stack.push((root, 0));
            while let Some((at, next)) = stack.last_mut() {
                let at = *at;
                if let Some(&child) = children[at].get(*next) {
                    *next += 1;
                    if !seen[child] {
                        seen[child] = true;
                        forest.place(child, Some(at), clock);
                        clock += 1;
                        stack.push((child, 0));
                    }
                } else {
                    forest.leave[at] = clock;
                    stack.pop();
                }
            }
        }
        forest
    }

    /// Number `i` as `parent`'s child, with its jump pointer.
    fn place(&mut self, i: usize, parent: Option<usize>, clock: u32) {
        self.enter[i] = clock;
        self.order.push(i);
        let Some(p) = parent else { return };
        self.parent[i] = Some(p);
        self.depth[i] = self.depth[p] + 1;
        let (j, jj) = (self.jump[p], self.jump[self.jump[p]]);
        self.jump[i] = if self.depth[p] - self.depth[j] == self.depth[j] - self.depth[jj] { jj } else { p };
    }

    /// The node with this ID.
    pub fn id(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    /// The node an edge end names: by ID, or by an alias exactly one node holds.
    pub fn end(&self, end: &planroom_contract::Ref) -> Option<usize> {
        self.index.get(end).and_then(|n| self.id(&n.id.0))
    }

    pub fn node(&self, i: usize) -> &'a Node {
        &self.nodes[i]
    }

    /// The rank of the node's level on the forest's ladder.
    pub fn rank(&self, i: usize) -> u8 {
        self.rank[i]
    }

    /// Whether `a` is `b` or one of its ancestors.
    pub fn holds(&self, a: usize, b: usize) -> bool {
        self.enter[a] <= self.enter[b] && self.enter[b] < self.leave[a]
    }

    /// The node's parent in the tree, which a parent cycle may have cut.
    pub fn parent(&self, i: usize) -> Option<usize> {
        self.parent[i]
    }

    /// The node's subtree as a range of depth-first numbers: a node holds exactly those whose
    /// number falls in its range.
    pub fn range(&self, i: usize) -> (u32, u32) {
        (self.enter[i], self.leave[i])
    }

    /// Whether `a` holds `b` or `b` holds `a`.
    pub fn related(&self, a: usize, b: usize) -> bool {
        self.holds(a, b) || self.holds(b, a)
    }

    /// Each node's block at the level of rank `level`: its nearest ancestor at or above the level, itself included,
    /// or the top of its chain when none is (architecture.md, "Levels and roll-ups", step 1).
    fn blocks_at(&mut self, level: u8) -> &[usize] {
        let (rank, order, parent) = (&self.rank, &self.order, &self.parent);
        self.blocks.entry(level).or_insert_with(|| {
            let mut block = vec![0; rank.len()];
            for &i in order {
                block[i] = match parent[i] {
                    Some(p) if rank[i] > level => block[p],
                    _ => i,
                };
            }
            block
        })
    }

    /// Fill the blocks of `level`, so [`Forest::lift`] reads them without a mutable borrow.
    pub fn prepare(&mut self, level: u8) {
        self.blocks_at(level);
    }

    /// The node's block at `level`: the block an edge end there lifts to before roll-up's step 3.
    /// [`Forest::prepare`] fills the level's blocks first.
    pub fn block(&self, i: usize, level: u8) -> usize {
        self.blocks.get(&level).expect("the level's blocks are prepared")[i]
    }

    /// The ends an edge from `u` to `v` lifts to at `level`, by roll-up's rule: each end's block
    /// at the level; none when both share it or either end holds the other, since the edge then
    /// lies inside one node; and an end whose block also holds the other end steps down to its
    /// child on the way to its own end, so no lifted edge joins a node to its own ancestor.
    /// [`Forest::prepare`] fills the level's blocks first.
    pub fn lift(&self, u: usize, v: usize, level: u8) -> Option<(usize, usize)> {
        let block = self.blocks.get(&level).expect("the level's blocks are prepared");
        let (mut a, mut b) = (block[u], block[v]);
        if a == b || self.related(u, v) {
            return None;
        }
        let (a_holds, b_holds) = (self.holds(a, v), self.holds(b, u));
        if a_holds && b_holds {
            return None;
        }
        if a_holds {
            a = self.highest_apart(u, v);
        }
        if b_holds {
            b = self.highest_apart(v, u);
        }
        Some((a, b))
    }

    /// The highest ancestor of `u`, itself included, that doesn't hold `v`; `u` doesn't hold `v`.
    fn highest_apart(&self, u: usize, v: usize) -> usize {
        // Holding `v` is true from the lowest node holding both upward, so climb through jump
        // pointers while the node above doesn't hold it.
        let mut at = u;
        while let Some(p) = self.parent[at] {
            if self.holds(p, v) {
                return at;
            }
            let j = self.jump[at];
            at = if !self.holds(j, v) { j } else { p };
        }
        at
    }

    /// Whether `edge`'s two ends resolve, as node positions.
    pub fn ends(&self, edge: &Edge) -> Option<(usize, usize)> {
        Some((self.end(&edge.from)?, self.end(&edge.to)?))
    }
}

#[cfg(test)]
mod tests {
    use crate::ladder::testing::*;
    use planroom_contract::{Kind, Layer, LevelId, NodeId, Provenance, Tier};

    use super::*;

    fn node(id: &str, level: LevelId, parent: Option<&str>) -> Node {
        Node {
            id: NodeId(id.into()),
            kind: Kind("t.x".into()),
            level,
            parent: parent.map(|p| NodeId(p.into())),
            name: id.into(),
            aliases: vec![],
            props: Default::default(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "t/x".into(), tier: Tier::Universal },
            evidence: vec![],
        }
    }

    #[test]
    fn holding_and_lifting_follow_the_parent_tree() {
        let nodes = vec![
            node("r:repo:r", realm(), None),
            node("r:dir:a", site(), Some("r:repo:r")),
            node("r:file:a/x", room(), Some("r:dir:a")),
            node("r:dir:b", site(), Some("r:repo:r")),
            node("r:file:b/y", room(), Some("r:dir:b")),
        ];
        let vocabulary = vocabulary();
        let ladder = Ladder::of(&vocabulary);
        let (region, site) = (ladder.rank(&region()), ladder.rank(&site()));
        let mut f = Forest::new(&nodes, &ladder);
        f.prepare(region);
        f.prepare(site);
        let i = |id: &str| f.id(id).unwrap();
        assert!(f.holds(i("r:repo:r"), i("r:file:b/y")));
        assert!(!f.holds(i("r:dir:a"), i("r:file:b/y")));
        let (x, y) = (i("r:file:a/x"), i("r:file:b/y"));
        // Both files' block at the region level is the realm node, so the edge lies inside it.
        assert_eq!(f.lift(x, y, region), None);
        assert_eq!(f.lift(x, y, site), Some((i("r:dir:a"), i("r:dir:b"))));
        // A file and the repo that holds it never make a lifted edge.
        assert_eq!(f.lift(x, i("r:repo:r"), site), None);
    }

    #[test]
    fn a_deep_chain_climbs_in_few_steps_and_a_cycle_still_ends() {
        let mut nodes = vec![node("c:dir:0", site(), None)];
        for d in 1..20_000 {
            nodes.push(node(&format!("c:dir:{d}"), site(), Some(&format!("c:dir:{}", d - 1))));
        }
        nodes.push(node("c:file:side", room(), Some("c:dir:3")));
        let vocabulary = vocabulary();
        let ladder = Ladder::of(&vocabulary);
        let f = Forest::new(&nodes, &ladder);
        let (deep, side) = (f.id("c:dir:19999").unwrap(), f.id("c:file:side").unwrap());
        assert_eq!(f.highest_apart(deep, side), f.id("c:dir:4").unwrap());
        // A two-node parent cycle: each names the other.
        let cycle = vec![node("c:dir:p", site(), Some("c:dir:q")), node("c:dir:q", site(), Some("c:dir:p"))];
        let f = Forest::new(&cycle, &ladder);
        assert!(f.holds(0, 1) ^ f.holds(1, 0));
    }
}
