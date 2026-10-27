//! Questions (architecture.md, "Lenses and questions"; DR-15): a named pipeline of one to three
//! query shapes, each step's input being the previous step's node set. A step with no IDs starts
//! from the previous step's nodes.
//!
//! The shapes are domain-free graph operations over a model; nothing here names a pack's kind,
//! relation or lens. This build serves `node`, `find`, `neighbors`, `path` and `why`, which the
//! code pack's questions use; a question with a step of another shape is refused with
//! [`QueryError::Unserved`] until that shape lands.
//!
//! - `neighbors` walks from each start node at the start node's own level: a start node stands
//!   for everything under it, and every edge is lifted to that level by roll-up's rule, so a
//!   container's neighbors are the blocks its files' edges reach. It follows its lenses' relations
//!   in their flow, or its `relations` forward, `depth` steps (1 when absent), `forward` (what the
//!   start reaches), `backward` (what reaches the start) or `both` (the default).
//! - `path` finds a shortest path, in hops, from `from` to `to` over its lens's relations in
//!   their flow, with edges lifted to the lens's level, so the hops inside one block are that
//!   block's own. Its first hop leaves a block under `from`, or the block `from` lifts to over an
//!   edge that leaves `from` itself, and its last reaches a block under `to`, or the block `to`
//!   lifts to over an edge that reaches `to` itself; no other ancestor stands for either. Each hop
//!   carries the edges it stands for, a sample of their evidence and their lowest confidence.
//! - `why` lists every edge between the two nodes, whatever its relation: from what is under
//!   `from` to what is under `to` first, then back, each with its evidence (and its `via_ref`
//!   node's), its confidence and its provenance.
//!
//! Each query reads the model once into a [`forest::Forest`] and an adjacency of lifted edges, so
//! its time is linear in the model's nodes and edges, up to a logarithmic factor.

pub mod forest;
pub mod lookup;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use planroom_contract::{
    EdgeId, Evidence, Flow, Kind, LensVocab, Level, NodeId, Provenance, Shape, ShapeStep, Vocabulary,
};
use serde_json::Value;

use crate::find;
use crate::model::Model;
use forest::Forest;

/// Evidence lines a hop carries at most, as a lifted edge's sample does.
pub const EVIDENCE_SAMPLE: usize = 3;

/// What a question's pipeline found: its last step's items, in the step's order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Answer {
    pub items: Vec<Item>,
}

impl Answer {
    /// The nodes the answer names, once each, in order: the next step's input.
    pub fn nodes(&self) -> Vec<NodeId> {
        let mut seen = HashSet::new();
        self.items.iter().filter(|i| seen.insert(i.node.clone())).map(|i| i.node.clone()).collect()
    }
}

/// One item of an answer: a node and why it answers.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub node: NodeId,
    pub reason: Reason,
}

/// Why an item answers its step.
#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    /// A `node` or `find` step named it.
    Named,
    /// The first block of a path.
    Start,
    /// The next block of a path, one hop after `from`.
    Hop(Hop),
    /// What a `neighbors` step reached.
    Neighbor(Neighbor),
    /// One edge between a `why` step's two nodes; the item's node is the edge's `from` end.
    Edge(EdgeWhy),
}

/// One hop of a path: every edge from the previous block to this one that the lens follows.
#[derive(Debug, Clone, PartialEq)]
pub struct Hop {
    pub from: NodeId,
    /// The relation of the hop's first edge.
    pub relation: Kind,
    /// Whether the hop runs against its edges' own direction, as a backward flow does.
    pub backward: bool,
    pub edges: Vec<EdgeId>,
    /// A sample of the edges' evidence, at most [`EVIDENCE_SAMPLE`].
    pub evidence: Vec<Evidence>,
    /// The lowest confidence among the edges.
    pub confidence: f32,
}

/// Which way a neighbor lies from the node a `neighbors` step started at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toward {
    /// The start reaches it, following the relations' flow.
    Forward,
    /// It reaches the start.
    Backward,
}

/// A node a `neighbors` step reached.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbor {
    /// The start node it was reached from.
    pub start: NodeId,
    /// The node it was reached from: the start at depth 1.
    pub via: NodeId,
    pub toward: Toward,
    /// Hops from the start.
    pub depth: u32,
    /// The relation of the first edge between `via` and it.
    pub relation: Kind,
    pub edges: Vec<EdgeId>,
}

/// One edge as `why` gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeWhy {
    pub edge: EdgeId,
    pub relation: Kind,
    pub from: NodeId,
    pub to: NodeId,
    pub via: Option<String>,
    pub confidence: f32,
    /// The edge's evidence, then its `via_ref` node's.
    pub evidence: Vec<Evidence>,
    pub provenance: Provenance,
    /// The edge runs from the step's `to` side back to its `from` side.
    pub back: bool,
}

/// Why a question's pipeline can't run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// A step's shape this build doesn't serve yet.
    Unserved(Shape),
    /// A parameter whose value the shape can't read, such as a `direction` other than `forward`,
    /// `backward` or `both`, or a lens no loaded pack declares.
    Param(String),
    /// A parameter the shape needs and the step doesn't give, with no previous step to start from.
    Missing(String),
}

/// The relations a step follows: every one forward, or the named ones, each in its flow.
enum Relations {
    Every,
    Named(HashMap<Kind, Flow>),
}

impl Relations {
    fn flow(&self, kind: &Kind) -> Option<Flow> {
        match self {
            Relations::Every => Some(Flow::Forward),
            Relations::Named(named) => named.get(kind).copied(),
        }
    }
}

/// One step's arcs between lifted blocks: `next` in the relations' flow, `prev` against it.
#[derive(Default)]
struct Graph {
    next: HashMap<usize, Vec<Arc>>,
    prev: HashMap<usize, Vec<Arc>>,
    /// Every block an arc touches, by depth-first number, so the blocks under a node are one run.
    blocks: Vec<(u32, usize)>,
}

/// One lifted edge from the block it's listed under to `to`.
#[derive(Debug, Clone, Copy)]
struct Arc {
    to: usize,
    edge: usize,
    /// It runs against its edge's own direction.
    backward: bool,
}

/// The level a question resolves its arguments at: the level of the first lens one of its steps
/// names, else the default lens's (architecture.md, "IDs and aliases").
pub fn level_of(steps: &[ShapeStep], vocabulary: &Vocabulary) -> Level {
    let named = steps.iter().find_map(|s| names(s.params.get("lens")?).into_iter().next().map(str::to_owned));
    named
        .and_then(|id| vocabulary.lenses.iter().find(|l| l.id == id))
        .or_else(|| vocabulary.lenses.iter().find(|l| l.default))
        .map_or(Level::Container, |l| l.level)
}

/// Run a question's `steps` over `model`, with its arguments bound to node IDs by name (`X` for
/// `$X`); an optional argument left out is absent.
pub fn run(
    model: &Model,
    vocabulary: &Vocabulary,
    steps: &[ShapeStep],
    args: &BTreeMap<String, NodeId>,
) -> Result<Answer, QueryError> {
    let mut forest = Forest::new(&model.nodes);
    let mut answer: Option<Answer> = None;
    for step in steps {
        let input: Vec<usize> = answer
            .as_ref()
            .map(|a| a.nodes().iter().filter_map(|id| forest.id(&id.0)).collect())
            .unwrap_or_default();
        let had_input = answer.is_some();
        let cx = Step { model, vocabulary, params: &step.params, args, input: &input, had_input };
        answer = Some(match step.shape {
            Shape::Node => cx.node(&forest),
            Shape::Find => cx.find(&forest),
            Shape::Neighbors => cx.neighbors(&mut forest)?,
            Shape::Path => cx.path(&mut forest)?,
            Shape::Why => cx.why(&forest)?,
            other => return Err(QueryError::Unserved(other)),
        });
    }
    Ok(answer.unwrap_or_default())
}

/// One step as it runs: its parameters, the question's arguments and the previous step's nodes.
struct Step<'a> {
    model: &'a Model,
    vocabulary: &'a Vocabulary,
    params: &'a Value,
    args: &'a BTreeMap<String, NodeId>,
    input: &'a [usize],
    had_input: bool,
}

impl Step<'_> {
    /// The nodes the node-ID parameter `key` names, with arguments filled in; the previous
    /// step's nodes when it names none, or when every argument it names was left out. A literal ID
    /// no node holds names nothing.
    fn ids(&self, forest: &Forest<'_>, key: &str) -> Option<Vec<usize>> {
        let values = self.params.get(key).map(names).unwrap_or_default();
        let mut bound = false;
        let mut found = Vec::new();
        for value in values {
            let Some(id) = fill(value, self.args) else { continue };
            bound = true;
            if let Some(i) = forest.id(&id)
                && !found.contains(&i)
            {
                found.push(i);
            }
        }
        if bound {
            Some(found)
        } else if self.had_input {
            Some(self.input.to_vec())
        } else {
            None
        }
    }

    fn node(&self, forest: &Forest<'_>) -> Answer {
        let ids = self.ids(forest, "id").unwrap_or_default();
        named(forest, ids)
    }

    fn find(&self, forest: &Forest<'_>) -> Answer {
        let text = self.params.get("text").and_then(Value::as_str).unwrap_or_default();
        let hits = find::find(&self.model.nodes, text);
        let mut ids: Vec<usize> = hits.iter().filter_map(|h| forest.id(&h.node.id.0)).collect();
        if self.had_input {
            let input: HashSet<usize> = self.input.iter().copied().collect();
            ids.retain(|i| input.contains(i));
        }
        named(forest, ids)
    }

    /// The relations the step follows: its lenses' in their flow, and its `relations` forward;
    /// every relation forward when it names neither.
    fn relations(&self) -> Result<Relations, QueryError> {
        let lenses = self.lenses()?;
        let listed = self.params.get("relations").map(names);
        if lenses.is_empty() && listed.is_none() {
            return Ok(Relations::Every);
        }
        let mut named = HashMap::new();
        for lens in lenses {
            for kind in &lens.relations {
                named.entry(kind.clone()).or_insert_with(|| lens.flow.get(kind).copied().unwrap_or_default());
            }
        }
        for kind in listed.unwrap_or_default() {
            named.entry(Kind(kind.to_owned())).or_insert(Flow::Forward);
        }
        Ok(Relations::Named(named))
    }

    /// The lenses the step names, in order.
    fn lenses(&self) -> Result<Vec<&LensVocab>, QueryError> {
        let ids = self.params.get("lens").map(names).unwrap_or_default();
        ids.into_iter()
            .map(|id| {
                self.vocabulary
                    .lenses
                    .iter()
                    .find(|l| l.id == id)
                    .ok_or_else(|| QueryError::Param("lens".into()))
            })
            .collect()
    }

    fn neighbors(&self, forest: &mut Forest<'_>) -> Result<Answer, QueryError> {
        let starts = self.ids(forest, "ids").ok_or_else(|| QueryError::Missing("ids".into()))?;
        let relations = self.relations()?;
        let depth = match self.params.get("depth") {
            None => 1,
            Some(v) => v.as_u64().filter(|d| *d >= 1).ok_or_else(|| QueryError::Param("depth".into()))?,
        };
        let directions: &[Toward] = match self.params.get("direction").map(|v| v.as_str()) {
            None | Some(Some("both")) => &[Toward::Backward, Toward::Forward],
            Some(Some("forward")) => &[Toward::Forward],
            Some(Some("backward")) => &[Toward::Backward],
            Some(_) => return Err(QueryError::Param("direction".into())),
        };
        // The start nodes by level, in order: one walk per level from all of them at once, so a
        // step that starts from many nodes still reads each lifted edge once per direction.
        let mut levels: Vec<(Level, Vec<usize>)> = Vec::new();
        for &start in &starts {
            let level = forest.node(start).level;
            match levels.iter_mut().find(|(l, _)| *l == level) {
                Some((_, group)) => group.push(start),
                None => levels.push((level, vec![start])),
            }
        }
        let mut items = Vec::new();
        let mut listed: HashSet<usize> = HashSet::new();
        for (level, group) in levels {
            forest.prepare(level);
            let graph = graph(forest, &self.model.edges, &relations, level);
            let held = Under::of(forest, &group);
            let inside = blocks_under(forest, &graph, &group);
            for &toward in directions {
                let arcs = match toward {
                    Toward::Forward => &graph.next,
                    Toward::Backward => &graph.prev,
                };
                let mut seen: HashSet<usize> = inside.iter().map(|(b, _)| *b).collect();
                let mut frontier: Vec<(usize, usize)> = inside.clone();
                for hop in 1..=depth {
                    let mut reached = Vec::new();
                    for &(at, start) in &frontier {
                        // Arcs are sorted by the block they reach, so each block's arcs are one run.
                        let list = arcs.get(&at).map_or(&[][..], Vec::as_slice);
                        for between in list.chunk_by(|x, y| x.to == y.to) {
                            let arc = between[0];
                            if held.under(forest, arc.to) || !seen.insert(arc.to) {
                                continue;
                            }
                            reached.push((arc.to, start));
                            if !listed.insert(arc.to) {
                                continue;
                            }
                            let via = if hop == 1 { start } else { at };
                            items.push(Item {
                                node: forest.node(arc.to).id.clone(),
                                reason: Reason::Neighbor(Neighbor {
                                    start: forest.node(start).id.clone(),
                                    via: forest.node(via).id.clone(),
                                    toward,
                                    depth: u32::try_from(hop).unwrap_or(u32::MAX),
                                    relation: self.model.edges[arc.edge].kind.clone(),
                                    edges: between
                                        .iter()
                                        .map(|a| self.model.edges[a.edge].id.clone())
                                        .collect(),
                                }),
                            });
                        }
                    }
                    if reached.is_empty() {
                        break;
                    }
                    frontier = reached;
                }
            }
        }
        Ok(Answer { items })
    }

    fn path(&self, forest: &mut Forest<'_>) -> Result<Answer, QueryError> {
        let from = self.ids(forest, "from").ok_or_else(|| QueryError::Missing("from".into()))?;
        let to = match self.params.get("to") {
            Some(_) => self.ids(forest, "to").unwrap_or_default(),
            None => return Err(QueryError::Missing("to".into())),
        };
        if from.is_empty() || to.is_empty() {
            return Ok(Answer::default());
        }
        let relations = self.relations()?;
        let level = match self.lenses()?.first() {
            Some(lens) => lens.level,
            None => from.iter().chain(&to).map(|i| forest.node(*i).level).min().unwrap_or(Level::Container),
        };
        forest.prepare(level);
        let graph = graph(forest, &self.model.edges, &relations, level);
        let (is_from, is_to) = (Ends::of(forest, &from, level), Ends::of(forest, &to, level));
        // One end holds the other: no path joins a node to what lies inside it.
        if from.iter().any(|f| is_to.under.under(forest, *f))
            || to.iter().any(|t| is_from.under.under(forest, *t))
        {
            return Ok(Answer::default());
        }
        let edges = &self.model.edges;
        // A path's first hop leaves a block under `from`, or the block `from` lifts to over an edge
        // that leaves `from` itself; its last reaches a block under `to`, or the block `to` lifts
        // to over an edge that reaches `to` itself.
        let leaves = |at: usize, arc: &Arc| is_from.leaves(forest, edges, at, arc);
        let reaches = |arc: &Arc| is_to.reaches(forest, edges, arc);
        let mut starts: Vec<usize> = graph
            .blocks
            .iter()
            .map(|b| b.1)
            .filter(|b| is_from.under.under(forest, *b) || is_from.blocks.contains(b))
            .collect();
        starts.sort_unstable();
        let mut came: HashMap<usize, Came> = HashMap::new();
        let mut queue: VecDeque<usize> = VecDeque::new();
        // The last hop: the block it leaves, the block it reaches, and whether it is also the first.
        let mut end: Option<(usize, usize, bool)> = None;
        'first: for &start in &starts {
            for arc in graph.next.get(&start).into_iter().flatten().filter(|a| leaves(start, a)) {
                if reaches(arc) {
                    end = Some((start, arc.to, true));
                    break 'first;
                }
                if is_from.under.under(forest, arc.to) || came.contains_key(&arc.to) {
                    continue;
                }
                came.insert(arc.to, Came::First(start));
                queue.push_back(arc.to);
            }
        }
        if end.is_none() {
            'search: while let Some(at) = queue.pop_front() {
                for arc in graph.next.get(&at).into_iter().flatten() {
                    if reaches(arc) {
                        end = Some((at, arc.to, false));
                        break 'search;
                    }
                    if is_from.under.under(forest, arc.to) || came.contains_key(&arc.to) {
                        continue;
                    }
                    came.insert(arc.to, Came::Hop(at));
                    queue.push_back(arc.to);
                }
            }
        }
        let Some((last, end, first)) = end else { return Ok(Answer::default()) };
        // The chain of blocks back to the start, the end first.
        let mut chain = vec![end, last];
        // A last hop that is also the first leaves a start; any other leaves a block a hop reached.
        if !first {
            while let Some(Came::Hop(prev)) = came.get(chain.last().expect("a chain is never empty")) {
                chain.push(*prev);
            }
            if let Some(Came::First(start)) = came.get(chain.last().expect("a chain is never empty")) {
                chain.push(*start);
            }
        }
        chain.reverse();
        let hops = chain.len() - 1;
        let mut items = vec![Item { node: forest.node(chain[0]).id.clone(), reason: Reason::Start }];
        for (k, pair) in chain.windows(2).enumerate() {
            let arcs: Vec<&Arc> = graph.next[&pair[0]]
                .iter()
                .filter(|a| a.to == pair[1])
                .filter(|a| k > 0 || leaves(pair[0], a))
                .filter(|a| k + 1 < hops || reaches(a))
                .collect();
            items.push(Item {
                node: forest.node(pair[1]).id.clone(),
                reason: Reason::Hop(self.hop(forest, pair[0], &arcs)),
            });
        }
        Ok(Answer { items })
    }

    /// One hop from `from` over `arcs`, which all lead to the same block.
    fn hop(&self, forest: &Forest<'_>, from: usize, arcs: &[&Arc]) -> Hop {
        let edges: Vec<&planroom_contract::Edge> = arcs.iter().map(|a| &self.model.edges[a.edge]).collect();
        let mut evidence = Vec::new();
        for e in edges.iter().flat_map(|e| &e.evidence) {
            if evidence.len() < EVIDENCE_SAMPLE && !evidence.contains(e) {
                evidence.push(e.clone());
            }
        }
        Hop {
            from: forest.node(from).id.clone(),
            relation: edges[0].kind.clone(),
            backward: arcs[0].backward,
            edges: edges.iter().map(|e| e.id.clone()).collect(),
            evidence,
            confidence: edges.iter().map(|e| e.confidence).fold(f32::INFINITY, f32::min),
        }
    }

    fn why(&self, forest: &Forest<'_>) -> Result<Answer, QueryError> {
        let from = self.ids(forest, "from").ok_or_else(|| QueryError::Missing("from".into()))?;
        let to = match self.params.get("to") {
            Some(_) => self.ids(forest, "to").unwrap_or_default(),
            None => return Err(QueryError::Missing("to".into())),
        };
        let a = Under::of(forest, &from);
        let b = Under::of(forest, &to);
        let mut there = Vec::new();
        let mut back = Vec::new();
        for edge in &self.model.edges {
            let Some((u, v)) = forest.ends(edge) else { continue };
            if a.under(forest, u) && b.under(forest, v) {
                there.push((edge, u, v, false));
            } else if b.under(forest, u) && a.under(forest, v) {
                back.push((edge, u, v, true));
            }
        }
        let key =
            |e: &(&planroom_contract::Edge, usize, usize, bool)| (e.0.kind.clone(), e.1, e.2, e.0.id.clone());
        there.sort_by_key(key);
        back.sort_by_key(key);
        let items = there
            .into_iter()
            .chain(back)
            .map(|(edge, u, v, back)| {
                let mut evidence = edge.evidence.clone();
                let via = edge.via_ref.as_ref().and_then(|r| forest.end(r)).map(|i| forest.node(i));
                for e in via.into_iter().flat_map(|n| &n.evidence) {
                    if !evidence.contains(e) {
                        evidence.push(e.clone());
                    }
                }
                Item {
                    node: forest.node(u).id.clone(),
                    reason: Reason::Edge(EdgeWhy {
                        edge: edge.id.clone(),
                        relation: edge.kind.clone(),
                        from: forest.node(u).id.clone(),
                        to: forest.node(v).id.clone(),
                        via: edge.via.clone(),
                        confidence: edge.confidence,
                        evidence,
                        provenance: edge.provenance.clone(),
                        back,
                    }),
                }
            })
            .collect();
        Ok(Answer { items })
    }
}

/// Each node once, as named items.
fn named(forest: &Forest<'_>, ids: Vec<usize>) -> Answer {
    let mut seen = HashSet::new();
    Answer {
        items: ids
            .into_iter()
            .filter(|i| seen.insert(*i))
            .map(|i| Item { node: forest.node(i).id.clone(), reason: Reason::Named })
            .collect(),
    }
}

/// The arcs of `edges` in `relations`, lifted to `level`, whose blocks [`Forest::prepare`] filled.
fn graph(
    forest: &Forest<'_>,
    edges: &[planroom_contract::Edge],
    relations: &Relations,
    level: Level,
) -> Graph {
    let mut graph = Graph::default();
    for (i, edge) in edges.iter().enumerate() {
        let Some(flow) = relations.flow(&edge.kind) else { continue };
        let Some((u, v)) = forest.ends(edge) else { continue };
        let Some((a, b)) = forest.lift(u, v, level) else { continue };
        let (tail, head, backward) = match flow {
            Flow::Forward => (a, b, false),
            Flow::Backward => (b, a, true),
        };
        graph.next.entry(tail).or_default().push(Arc { to: head, edge: i, backward });
        graph.prev.entry(head).or_default().push(Arc { to: tail, edge: i, backward });
    }
    // Arcs in the order of the node they reach, then by relation and edge ID, so every walk is
    // reproducible.
    for list in graph.next.values_mut().chain(graph.prev.values_mut()) {
        list.sort_by(|x, y| {
            (x.to, &edges[x.edge].kind, &edges[x.edge].id).cmp(&(
                y.to,
                &edges[y.edge].kind,
                &edges[y.edge].id,
            ))
        });
    }
    let mut blocks: Vec<(u32, usize)> =
        graph.next.keys().chain(graph.prev.keys()).map(|b| (forest.range(*b).0, *b)).collect();
    blocks.sort_unstable();
    blocks.dedup();
    graph.blocks = blocks;
    graph
}

/// The blocks of `graph` that the start nodes hold, each with the nearest start that holds it,
/// and the start nodes themselves: where a walk from them begins. One sweep in depth-first order
/// over the starts and the blocks, so nested starts cost nothing extra.
fn blocks_under(forest: &Forest<'_>, graph: &Graph, starts: &[usize]) -> Vec<(usize, usize)> {
    let mut opens: Vec<(u32, usize)> = starts.iter().map(|s| (forest.range(*s).0, *s)).collect();
    opens.sort_unstable();
    opens.dedup();
    let mut out: Vec<(usize, usize)> = opens.iter().map(|(_, s)| (*s, *s)).collect();
    let mut open: Vec<usize> = Vec::new();
    let mut next = 0;
    for &(at, block) in &graph.blocks {
        while next < opens.len() && opens[next].0 <= at {
            while open.last().is_some_and(|s| forest.range(*s).1 <= opens[next].0) {
                open.pop();
            }
            open.push(opens[next].1);
            next += 1;
        }
        while open.last().is_some_and(|s| forest.range(*s).1 <= at) {
            open.pop();
        }
        if let Some(&start) = open.last() {
            out.push((block, start));
        }
    }
    let mut seen = HashSet::new();
    out.retain(|(b, _)| seen.insert(*b));
    out
}

/// What a set of nodes holds: a node is under the set when one of its members holds it. The
/// members' subtree ranges, merged, answer that in logarithmic time, however large the set.
struct Under {
    ranges: Vec<(u32, u32)>,
}

impl Under {
    fn of(forest: &Forest<'_>, nodes: &[usize]) -> Under {
        let mut ranges: Vec<(u32, u32)> = nodes.iter().map(|n| forest.range(*n)).collect();
        ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
        for (start, end) in ranges {
            match merged.last_mut() {
                Some(last) if start < last.1 => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        Under { ranges: merged }
    }

    fn under(&self, forest: &Forest<'_>, i: usize) -> bool {
        let at = forest.range(i).0;
        let k = self.ranges.partition_point(|r| r.0 <= at);
        k > 0 && at < self.ranges[k - 1].1
    }
}

/// How a block on a path was reached: by a first hop from a start block, or a hop from another.
enum Came {
    First(usize),
    Hop(usize),
}

/// One end of a path at its level: what lies under the nodes it names, and the blocks they lift
/// to. A block under the end stands for it whole; the block it lifts to only over an edge whose own
/// end lies under it, since that block holds more than the end. No other ancestor stands for it,
/// so an edge from elsewhere in `from`'s repo is no path from `from`, as `why` finds no edge there.
struct Ends {
    under: Under,
    blocks: HashSet<usize>,
}

impl Ends {
    fn of(forest: &Forest<'_>, nodes: &[usize], level: Level) -> Ends {
        let blocks = nodes.iter().map(|n| forest.block(*n, level)).collect();
        Ends { under: Under::of(forest, nodes), blocks }
    }

    /// The node of the arc's own edge that the arc leaves (`head` false) or reaches, in its flow.
    fn end(forest: &Forest<'_>, edges: &[planroom_contract::Edge], arc: &Arc, head: bool) -> Option<usize> {
        let (u, v) = forest.ends(&edges[arc.edge])?;
        Some(if head != arc.backward { v } else { u })
    }

    /// Whether the arc, listed under the block `at`, leaves this end.
    fn leaves(&self, forest: &Forest<'_>, edges: &[planroom_contract::Edge], at: usize, arc: &Arc) -> bool {
        self.under.under(forest, at)
            || (self.blocks.contains(&at)
                && Self::end(forest, edges, arc, false).is_some_and(|n| self.under.under(forest, n)))
    }

    /// Whether the arc reaches this end.
    fn reaches(&self, forest: &Forest<'_>, edges: &[planroom_contract::Edge], arc: &Arc) -> bool {
        self.under.under(forest, arc.to)
            || (self.blocks.contains(&arc.to)
                && Self::end(forest, edges, arc, true).is_some_and(|n| self.under.under(forest, n)))
    }
}

/// The names a parameter holds: a string, or a list of strings.
fn names(value: &Value) -> Vec<&str> {
    match value {
        Value::String(s) => vec![s.as_str()],
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        Value::Object(map) => map.get("in").map(names).unwrap_or_default(),
        _ => vec![],
    }
}

/// `value` with each `$NAME` argument filled in, or none when it names an argument left out.
fn fill(value: &str, args: &BTreeMap<String, NodeId>) -> Option<String> {
    if !value.contains('$') {
        return Some(value.to_owned());
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let len = after.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(after.len());
        if len == 0 {
            out.push('$');
        } else {
            out.push_str(&args.get(&after[..len])?.0);
        }
        rest = &after[len..];
    }
    out.push_str(rest);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_argument_fills_a_whole_id_or_a_part_of_one() {
        let args = BTreeMap::from([("X".to_owned(), NodeId("a:box:web".into()))]);
        assert_eq!(fill("$X", &args).as_deref(), Some("a:box:web"));
        assert_eq!(fill("b:part:$X", &args).as_deref(), Some("b:part:a:box:web"));
        assert_eq!(fill("b:part:k", &args).as_deref(), Some("b:part:k"));
        assert_eq!(fill("$Y", &args), None);
    }
}
