//! A map through one lens (architecture.md, "Lenses and questions"; DR-15, DR-46): the relations
//! it lifts, the kinds it is about, and each block's tone from the kind that declares it.
//!
//! A lens is data from the loaded packs' vocabulary, so nothing here names a pack's kind,
//! relation or lens. A lens lifts only its relations. A lens whose `kinds` is non-empty shows the
//! nodes of its kinds, edges or none, plus the nodes its relations touch, each with its parent
//! chain, and lists its kinds' nodes first. A lens whose `kinds` is empty shows every block. The
//! rest rank by the lifted edges they carry, most first, ties in the view's own order. With no
//! lens (`--lens all`), a view is about every kind of every loaded pack and lifts every relation,
//! ranked the same way.

use std::collections::{HashMap, HashSet};

use planroom_contract::{Kind, LensVocab, MapView, NodeId, Tone, Vocabulary};

/// The lens with ID `id`, as a loaded pack declares it.
pub fn named<'a>(vocabulary: &'a Vocabulary, id: &str) -> Option<&'a LensVocab> {
    vocabulary.lenses.iter().find(|l| l.id == id)
}

/// The one default lens: the first the vocabulary marks default. The loader keeps at most one
/// default, the code pack's when two packs claim one.
pub fn default(vocabulary: &Vocabulary) -> Option<&LensVocab> {
    vocabulary.lenses.iter().find(|l| l.default)
}

/// The tone of `kind`, as its pack declares it: a kind's block color.
pub fn tone(vocabulary: &Vocabulary, kind: &Kind) -> Option<Tone> {
    vocabulary.kinds.iter().find(|k| &k.kind == kind).and_then(|k| k.tone)
}

/// `view` through `lens`, or through every relation and kind when `lens` is `None`: the lifted
/// edges of the lens's relations only; with kinds, only their blocks and the blocks those edges
/// touch, each with the ancestors the view holds; the nodes of its kinds first, then by the
/// lifted edges each carries; every block with its kind's tone; and the lens's yarn, none with no lens.
pub fn apply(mut view: MapView, vocabulary: &Vocabulary, lens: Option<&LensVocab>) -> MapView {
    for node in &mut view.nodes {
        node.tone = tone(vocabulary, &node.kind);
    }
    view.yarn = lens.and_then(|l| l.yarn);
    if let Some(lens) = lens {
        view.edges.retain(|e| lens.relations.contains(&e.kind));
        if !lens.kinds.is_empty() {
            let keep = shown(&view, lens);
            view.nodes.retain(|n| keep.contains(&n.id));
        }
    }
    let mut carried: HashMap<&str, u32> = HashMap::new();
    for edge in &view.edges {
        *carried.entry(edge.from.0.as_str()).or_default() += edge.count;
        *carried.entry(edge.to.0.as_str()).or_default() += edge.count;
    }
    let about = |kind: &Kind| lens.is_some_and(|l| l.kinds.contains(kind));
    // A stable sort, so equal blocks keep the view's own order.
    let mut ranked: Vec<(bool, u32, usize)> = view
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (!about(&n.kind), u32::MAX - carried.get(n.id.0.as_str()).copied().unwrap_or(0), i))
        .collect();
    ranked.sort();
    let mut nodes: Vec<Option<_>> = view.nodes.into_iter().map(Some).collect();
    view.nodes = ranked.into_iter().filter_map(|(.., i)| nodes[i].take()).collect();
    view
}

/// The blocks a lens with kinds shows: the nodes of its kinds, the nodes its lifted edges touch,
/// and each one's ancestors that the view holds, so a block never shows without its parent chain.
fn shown(view: &MapView, lens: &LensVocab) -> HashSet<NodeId> {
    let parent: HashMap<&NodeId, &NodeId> =
        view.nodes.iter().filter_map(|n| n.parent.as_ref().map(|p| (&n.id, p))).collect();
    let held: HashSet<&NodeId> = view.nodes.iter().map(|n| &n.id).collect();
    let seeds = view
        .nodes
        .iter()
        .filter(|n| lens.kinds.contains(&n.kind))
        .map(|n| &n.id)
        .chain(view.edges.iter().flat_map(|e| [&e.from, &e.to]));
    let mut keep: HashSet<NodeId> = HashSet::new();
    for seed in seeds {
        let mut at = Some(seed);
        // A block already kept has its chain kept too, so the walk stops there, and a parent
        // cycle ends as well.
        while let Some(id) = at.filter(|id| held.contains(id)) {
            if !keep.insert(id.clone()) {
                break;
            }
            at = parent.get(id).copied();
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use crate::ladder::testing::*;
    use std::collections::BTreeMap;

    use planroom_contract::{KindVocab, LevelId, LiftedEdge, MapNode, NodeId, Yarn};

    use super::*;

    fn kind(id: &str, level: LevelId, tone: Option<Tone>) -> KindVocab {
        KindVocab {
            kind: Kind(id.into()),
            level,
            id_prefix: id.rsplit('.').next().unwrap().into(),
            label: id.into(),
            props: serde_json::Value::Null,
            tone,
        }
    }

    fn lens(id: &str, relations: &[&str], kinds: &[&str], default: bool) -> LensVocab {
        LensVocab {
            id: id.into(),
            label: id.into(),
            relations: relations.iter().map(|r| Kind((*r).into())).collect(),
            flow: BTreeMap::new(),
            kinds: kinds.iter().map(|k| Kind((*k).into())).collect(),
            level: region(),
            default,
            yarn: None,
        }
    }

    fn vocabulary() -> Vocabulary {
        Vocabulary {
            kinds: vec![
                kind("p.app", region(), Some(Tone::Service)),
                kind("p.vault", region(), Some(Tone::Store)),
                kind("p.flag", region(), None),
            ],
            relations: vec![],
            lenses: vec![
                lens("calls", &["p.call"], &[], true),
                lens("flags", &["p.reads"], &["p.flag"], false),
            ],
            suggestions: vec![],
            levels: crate::ladder::testing::vocabulary().levels,
        }
    }

    fn block(id: &str, kind: &str) -> MapNode {
        MapNode {
            id: NodeId(id.into()),
            kind: Kind(kind.into()),
            tone: None,
            name: id.into(),
            level: region(),
            parent: None,
            counts: BTreeMap::new(),
            position: None,
            ghost: false,
            claimed_by: None,
        }
    }

    fn edge(kind: &str, from: &str, to: &str) -> LiftedEdge {
        LiftedEdge {
            kind: Kind(kind.into()),
            from: NodeId(from.into()),
            to: NodeId(to.into()),
            count: 0,
            evidence_sample: vec![],
            children: vec![],
        }
    }

    fn view() -> MapView {
        MapView {
            level: region(),
            lens: String::new(),
            focus: None,
            nodes: vec![
                block("a:x:api", "p.app"),
                block("a:x:vault", "p.vault"),
                block("a:x:gift", "p.flag"),
            ],
            edges: vec![edge("p.call", "a:x:api", "a:x:vault"), edge("p.reads", "a:x:api", "a:x:gift")],
            hidden: 0,
            empty: None,
            layout: None,
            levels: vec![],
            yarn: None,
        }
    }

    fn ids(view: &MapView) -> Vec<&str> {
        view.nodes.iter().map(|n| n.id.0.as_str()).collect()
    }

    #[test]
    fn no_lens_keeps_every_block_and_edge_and_sets_each_tone() {
        let vocabulary = vocabulary();
        let all = apply(view(), &vocabulary, None);
        assert_eq!(ids(&all), ["a:x:api", "a:x:vault", "a:x:gift"]);
        assert_eq!(all.edges.len(), 2);
        let tones: Vec<Option<Tone>> = all.nodes.iter().map(|n| n.tone).collect();
        assert_eq!(tones, [Some(Tone::Service), Some(Tone::Store), None]);
    }

    #[test]
    fn a_lens_lifts_only_its_relations_and_hides_no_block() {
        let vocabulary = vocabulary();
        let calls = apply(view(), &vocabulary, default(&vocabulary));
        assert_eq!(ids(&calls), ["a:x:api", "a:x:vault", "a:x:gift"]);
        let kinds: Vec<&str> = calls.edges.iter().map(|e| e.kind.0.as_str()).collect();
        assert_eq!(kinds, ["p.call"]);
    }

    #[test]
    fn blocks_rank_by_the_lifted_edges_they_carry() {
        let vocabulary = vocabulary();
        let mut busy = view();
        busy.edges[0].count = 1;
        busy.edges[1].count = 4;
        // Through every relation, the block with the most lifted edges comes first, and equal
        // blocks keep their order; through `calls`, only its edges count.
        assert_eq!(ids(&apply(busy.clone(), &vocabulary, None)), ["a:x:api", "a:x:gift", "a:x:vault"]);
        assert_eq!(
            ids(&apply(busy, &vocabulary, default(&vocabulary))),
            ["a:x:api", "a:x:vault", "a:x:gift"]
        );
    }

    #[test]
    fn a_lens_with_kinds_lists_their_blocks_first_edges_or_none() {
        let vocabulary = vocabulary();
        let flags = apply(view(), &vocabulary, named(&vocabulary, "flags"));
        assert_eq!(ids(&flags), ["a:x:gift", "a:x:api"]);
        let mut edgeless = view();
        edgeless.edges.clear();
        assert_eq!(ids(&apply(edgeless, &vocabulary, named(&vocabulary, "flags"))), ["a:x:gift"]);
        assert!(named(&vocabulary, "nowhere").is_none());
        assert_eq!(default(&vocabulary).map(|l| l.id.as_str()), Some("calls"));
    }

    #[test]
    fn a_lens_with_kinds_shows_them_and_what_its_relations_touch_each_with_its_parent_chain() {
        let vocabulary = vocabulary();
        let mut nested = view();
        // A repo holds the app, a folder in it holds the flag, and a second repo holds the vault
        // and a second app that only `p.call` reaches.
        let mut repo = block("a:repo:a", "p.repo");
        repo.level = realm();
        let mut other = block("b:repo:b", "p.repo");
        other.level = realm();
        let mut folder = block("a:dir:flags", "p.folder");
        folder.parent = Some(repo.id.clone());
        nested.nodes[0].parent = Some(repo.id.clone());
        nested.nodes[1].parent = Some(other.id.clone());
        nested.nodes[2].parent = Some(folder.id.clone());
        let mut lone = block("b:x:lone", "p.app");
        lone.parent = Some(other.id.clone());
        nested.edges.push(edge("p.call", "b:x:lone", "a:x:vault"));
        nested.nodes.extend([repo, other, folder, lone]);

        let flags = apply(nested.clone(), &vocabulary, named(&vocabulary, "flags"));
        // The flag, the app `p.reads` touches, and the repo and folder above them; never the
        // vault, the second app or their repo, which only `p.call` reaches.
        assert_eq!(ids(&flags), ["a:x:gift", "a:x:api", "a:repo:a", "a:dir:flags"]);
        let kinds: Vec<&str> = flags.edges.iter().map(|e| e.kind.0.as_str()).collect();
        assert_eq!(kinds, ["p.reads"]);

        // A lens whose `kinds` is empty hides no block.
        let calls = apply(nested.clone(), &vocabulary, default(&vocabulary));
        assert_eq!(calls.nodes.len(), nested.nodes.len());
    }

    #[test]
    fn a_lens_puts_its_yarn_on_the_view_and_no_lens_puts_none() {
        let vocabulary = vocabulary();
        let mut dyed = lens("dyed", &["p.call"], &[], false);
        dyed.yarn = Some(Yarn::Plum);
        let shown = apply(view(), &vocabulary, Some(&dyed));
        assert_eq!(shown.yarn, Some(Yarn::Plum));

        // A lens that names none draws none, and so does no lens, whatever the input held.
        let plain = lens("plain", &["p.call"], &[], false);
        let mut held = view();
        held.yarn = Some(Yarn::Teal);
        assert_eq!(apply(held.clone(), &vocabulary, Some(&plain)).yarn, None);
        assert_eq!(apply(held, &vocabulary, None).yarn, None);
    }
}
