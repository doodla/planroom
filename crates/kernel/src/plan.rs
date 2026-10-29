//! Plans as kernel vocabulary (architecture.md, "IDs and aliases" and "Planning: stages, ghosts
//! and lint (R5)"; DR-45): a project is the `kernel.project` node `plan:<slug>:project`, a plan
//! item the `kernel.item` node `plan:<slug>:<key>` whose `type` prop names an item type a stage
//! declares, and what an item touches a `kernel.touches` edge from it. Pure functions over the
//! fold; the item types and stages arrive as data, and nothing here names one.
//!
//! A project's slug is its text as lowercase words, at most 48 characters, used only when
//! redaction would leave the text as it is, since an ID is never masked; otherwise, and for an
//! empty slug or the kernel's own `group` prefix, it is `p-` and 8 hex characters of the text's
//! sha256. An item's project is read from its ID's prefix, so a ghost needs no chain walk.

use std::collections::{BTreeMap, BTreeSet};

use planroom_contract::{
    Edit, EventId, Kind, Layer, LevelId, Node, NodeId, Op, PlanProposal, Provenance, Ref, Seq, StageDecl,
    Target,
};
use serde_json::{Value, json};

use crate::fold::{Folded, Ghost, build_node};
use crate::keys;

/// The kernel's project kind.
pub const PROJECT_KIND: &str = "kernel.project";
/// The kernel's plan item kind.
pub const ITEM_KIND: &str = "kernel.item";
/// The kernel's one relation: what a plan item touches.
pub const TOUCHES: &str = "kernel.touches";
/// The namespace of every project and plan item.
pub const NAMESPACE: &str = "plan";
/// The prop that names an item's type.
pub const TYPE: &str = "type";
/// The key of a project's own node, which no item takes.
pub const PROJECT_KEY: &str = "project";
/// The longest slug a project's text gives.
const SLUG_CHARS: usize = 48;
/// The longest item key (DR-94's bound on a declared ID).
pub const KEY_MAX: usize = 64;
/// The kernel's own ID prefix for hand groups: a `plan:group:*` item would share their space.
const RESERVED: &str = "group";

/// The slug a project named `text` takes.
pub fn project_slug(text: &str) -> String {
    let slug = keys::slug(text).map(|s| {
        let cut: String = s.chars().take(SLUG_CHARS).collect();
        cut.trim_matches('-').to_owned()
    });
    match slug {
        Some(slug)
            if !slug.is_empty()
                && slug != RESERVED
                && crate::redact::text(text) == text
                && crate::redact::text(&slug) == slug =>
        {
            slug
        }
        _ => format!("p-{}", &keys::sha256_hex(&[text])[..8]),
    }
}

/// The node of the project whose slug is `slug`.
pub fn project_id(slug: &str) -> NodeId {
    NodeId(format!("{NAMESPACE}:{slug}:{PROJECT_KEY}"))
}

/// The node of the item `key` in the project whose slug is `slug`.
pub fn item_id(slug: &str, key: &str) -> NodeId {
    NodeId(format!("{NAMESPACE}:{slug}:{key}"))
}

/// The slug of the project a plan ID belongs to, read from its prefix.
pub fn slug_of(id: &NodeId) -> Option<&str> {
    let rest = id.0.strip_prefix(NAMESPACE)?.strip_prefix(':')?;
    let (slug, key) = rest.split_once(':')?;
    (!slug.is_empty() && !key.is_empty()).then_some(slug)
}

/// Whether `id` is a project's own node.
pub fn is_project(id: &NodeId) -> bool {
    slug_of(id).is_some() && id.0.ends_with(&format!(":{PROJECT_KEY}"))
}

/// Whether `id` is an item of the project `project`, or the project itself.
pub fn in_project(id: &NodeId, project: &NodeId) -> bool {
    slug_of(id).is_some() && slug_of(id) == slug_of(project)
}

/// Whether `key` may key an item: `[a-z0-9][a-z0-9_-]{0,63}`, never `project`, and left as it is
/// by redaction, since it becomes an ID.
pub fn key_ok(key: &str) -> bool {
    let b = key.as_bytes();
    let word = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    !b.is_empty()
        && b.len() <= KEY_MAX
        && word(b[0])
        && b.iter().all(|&c| word(c) || c == b'_' || c == b'-')
        && key != PROJECT_KEY
        && crate::redact::text(key) == key
}

/// The `put` of a new project named `name`.
pub fn project_put(id: &NodeId, name: &str, base: Seq) -> Edit {
    let fields = BTreeMap::from([
        ("kind".to_owned(), json!(PROJECT_KIND)),
        ("level".to_owned(), json!(LevelId::INTENT)),
        ("name".to_owned(), json!(name)),
    ]);
    Edit {
        op: Op::Put,
        target: Target::Node(id.clone()),
        layer: Layer::Intended,
        fields,
        base,
        over: BTreeMap::new(),
    }
}

/// A project as a plan reads it: from the model, or from its pending ghost `put`.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub id: NodeId,
    pub name: String,
    /// In the model; else only a pending ghost holds it.
    pub live: bool,
    /// The pending ghost `put`s of it, in `seq` order.
    pub pending: Vec<EventId>,
}

/// Every project, live or pending as a ghost, by ID.
pub fn projects(folded: &Folded) -> Vec<Project> {
    let mut out: BTreeMap<NodeId, Project> = BTreeMap::new();
    for node in folded.model.nodes.iter().filter(|n| n.kind.0 == PROJECT_KIND && is_project(&n.id)) {
        out.insert(
            node.id.clone(),
            Project { id: node.id.clone(), name: node.name.clone(), live: true, pending: vec![] },
        );
    }
    for ghost in &folded.overlay.ghosts {
        let Some(node) = ghost_node(ghost) else { continue };
        if node.kind.0 != PROJECT_KIND || !is_project(&node.id) {
            continue;
        }
        let entry = out.entry(node.id.clone()).or_insert_with(|| Project {
            id: node.id.clone(),
            name: node.name.clone(),
            live: false,
            pending: vec![],
        });
        if !entry.live {
            entry.name = node.name.clone();
        }
        entry.pending.push(ghost.event.clone());
    }
    out.into_values().collect()
}

/// The node a pending ghost `put` would write.
pub fn ghost_node(ghost: &Ghost) -> Option<Node> {
    let (Op::Put, Target::Node(id)) = (&ghost.edit.op, &ghost.edit.target) else { return None };
    let provenance = Provenance::Authored { actor: ghost.writer.actor.clone(), event: ghost.event.clone() };
    build_node(id, &ghost.edit, provenance)
}

/// One plan item as the plan reads it: the model's, or a pending ghost `put`'s.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: NodeId,
    pub name: String,
    /// Its `type` prop.
    pub item_type: String,
    pub parent: Option<NodeId>,
    /// In the model; else only a pending ghost holds it.
    pub live: bool,
    /// Its `kernel.touches` targets: the model's, then pending ones, each once.
    pub touches: Vec<NodeId>,
    /// Its pending ghost edits, those on its touches included, in `seq` order.
    pub pending: Vec<EventId>,
}

/// The items of `project`: the model's, plus the overlay's pending ghost `put` and `set` edits on
/// the project's `plan:<slug>:*` nodes and on their `kernel.touches` edges, by ID.
pub fn items_of(folded: &Folded, project: &NodeId) -> Vec<Item> {
    let mine = |id: &NodeId| in_project(id, project) && !is_project(id);
    let mut items: BTreeMap<NodeId, Item> = BTreeMap::new();
    for node in folded.model.nodes.iter().filter(|n| n.kind.0 == ITEM_KIND && mine(&n.id)) {
        items.insert(node.id.clone(), item_of(node, true));
    }
    for edge in folded.model.edges.iter().filter(|e| e.kind.0 == TOUCHES) {
        if let (Ref::Id(from), Ref::Id(to)) = (&edge.from, &edge.to)
            && let Some(item) = items.get_mut(from)
            && !item.touches.contains(to)
        {
            item.touches.push(to.clone());
        }
    }
    for ghost in &folded.overlay.ghosts {
        match &ghost.edit.target {
            Target::Node(id) if mine(id) => {
                if let Some(node) = ghost_node(ghost).filter(|n| n.kind.0 == ITEM_KIND) {
                    let entry = items.entry(id.clone()).or_insert_with(|| item_of(&node, false));
                    if !entry.live {
                        let pending = std::mem::take(&mut entry.pending);
                        let touches = std::mem::take(&mut entry.touches);
                        *entry = Item { pending, touches, ..item_of(&node, false) };
                    }
                }
                if let Some(item) = items.get_mut(id) {
                    item.pending.push(ghost.event.clone());
                }
            }
            Target::Edge(_) if ghost.edit.op == Op::Put => {
                let field = |name: &str| -> Option<NodeId> {
                    match serde_json::from_value::<Ref>(ghost.edit.fields.get(name)?.clone()).ok()? {
                        Ref::Id(id) => Some(id),
                        Ref::Alias(_) => None,
                    }
                };
                let kind = ghost.edit.fields.get("kind").and_then(Value::as_str);
                if kind != Some(TOUCHES) {
                    continue;
                }
                if let (Some(from), Some(to)) = (field("from"), field("to"))
                    && let Some(item) = items.get_mut(&from)
                {
                    if !item.touches.contains(&to) {
                        item.touches.push(to);
                    }
                    item.pending.push(ghost.event.clone());
                }
            }
            _ => {}
        }
    }
    items.into_values().collect()
}

fn item_of(node: &Node, live: bool) -> Item {
    Item {
        id: node.id.clone(),
        name: node.name.clone(),
        item_type: node.props.get(TYPE).and_then(Value::as_str).unwrap_or_default().to_owned(),
        parent: node.parent.clone(),
        live,
        touches: vec![],
        pending: vec![],
    }
}

/// The first stage that writes `item_type`.
pub fn stage_of<'a>(stages: &'a [StageDecl], item_type: &str) -> Option<&'a StageDecl> {
    stages.iter().find(|s| s.items.iter().any(|t| t.id == item_type))
}

/// The first stage, in order, with no live item of the types it writes.
pub fn next_stage<'a>(stages: &'a [StageDecl], items: &[Item]) -> Option<&'a StageDecl> {
    stages.iter().find(|s| !items.iter().any(|i| i.live && s.items.iter().any(|t| t.id == i.item_type)))
}

/// Items live first, then ghosts, each by the first stage that writes its type, then by ID.
pub fn order(items: &mut [Item], stages: &[StageDecl]) {
    let rank = |item: &Item| stages.iter().position(|s| s.items.iter().any(|t| t.id == item.item_type));
    items.sort_by(|a, b| {
        (!a.live, rank(a).unwrap_or(usize::MAX), &a.id.0).cmp(&(
            !b.live,
            rank(b).unwrap_or(usize::MAX),
            &b.id.0,
        ))
    });
}

/// The edits that write `proposal` into `project` at `base`, with each item's parent and touch
/// targets already resolved by key; `parents` holds only the parents the proposal names. A key is
/// existing only when its node is in the model: an existing item gets one `set` of the fields that
/// differ, and none when nothing differs, keeping its parent when the proposal names none; any
/// other key, one only pending ghosts hold included, gets a whole `put` built from the proposal
/// alone, under the project when it names no parent. Each touch target the model doesn't already join to its item gets one edge `put`. A
/// proposal never retracts.
pub fn edits_for(
    proposal: &PlanProposal,
    project: &NodeId,
    parents: &BTreeMap<String, NodeId>,
    touches: &BTreeMap<String, Vec<NodeId>>,
    folded: &Folded,
    base: Seq,
) -> Vec<Edit> {
    let Some(slug) = slug_of(project) else { return vec![] };
    let touches_kind = Kind(TOUCHES.to_owned());
    let joined: BTreeSet<&str> = folded.model.edges.iter().map(|e| e.id.0.as_str()).collect();
    let mut out = Vec::new();
    for item in &proposal.items {
        let id = item_id(slug, &item.key);
        let named = parents.get(&item.key);
        let parent = named.cloned().unwrap_or_else(|| project.clone());
        let existing = folded.model.nodes.iter().find(|n| n.id == id);
        let edit = match existing {
            Some(node) => {
                let mut fields = BTreeMap::new();
                if node.name != item.name {
                    fields.insert("name".to_owned(), json!(item.name));
                }
                // A parent the proposal leaves out is no field that differs: the item stays put.
                if let Some(parent) = named.filter(|p| node.parent.as_ref() != Some(*p)) {
                    fields.insert("parent".to_owned(), json!(parent));
                }
                if node.props.get(TYPE).and_then(Value::as_str) != Some(item.item_type.as_str()) {
                    fields.insert(format!("props.{TYPE}"), json!(item.item_type));
                }
                for (prop, value) in &item.props {
                    if node.props.get(prop) != Some(value) {
                        fields.insert(format!("props.{prop}"), value.clone());
                    }
                }
                (!fields.is_empty()).then(|| Edit {
                    op: Op::Set,
                    target: Target::Node(id.clone()),
                    layer: Layer::Intended,
                    fields,
                    base,
                    over: BTreeMap::new(),
                })
            }
            None => {
                let mut props = serde_json::Map::new();
                props.insert(TYPE.to_owned(), json!(item.item_type));
                for (prop, value) in &item.props {
                    props.insert(prop.clone(), value.clone());
                }
                let fields = BTreeMap::from([
                    ("kind".to_owned(), json!(ITEM_KIND)),
                    ("level".to_owned(), json!(LevelId::INTENT)),
                    ("name".to_owned(), json!(item.name)),
                    ("parent".to_owned(), json!(parent)),
                    ("props".to_owned(), Value::Object(props)),
                ]);
                Some(Edit {
                    op: Op::Put,
                    target: Target::Node(id.clone()),
                    layer: Layer::Intended,
                    fields,
                    base,
                    over: BTreeMap::new(),
                })
            }
        };
        out.extend(edit);
        for target in touches.get(&item.key).into_iter().flatten() {
            let edge = keys::edge_id(&touches_kind, &id, target);
            if joined.contains(edge.0.as_str()) || out.iter().any(|e| e.target == Target::Edge(edge.clone()))
            {
                continue;
            }
            let fields = BTreeMap::from([
                ("kind".to_owned(), json!(TOUCHES)),
                ("from".to_owned(), json!({"id": id})),
                ("to".to_owned(), json!({"id": target})),
            ]);
            out.push(Edit {
                op: Op::Put,
                target: Target::Edge(edge),
                layer: Layer::Intended,
                fields,
                base,
                over: BTreeMap::new(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_slug_is_its_text_unless_redaction_would_change_it() {
        assert_eq!(project_slug("Add retries to the worker"), "add-retries-to-the-worker");
        assert_eq!(project_id("x").0, "plan:x:project");
        let long = "word ".repeat(30);
        let slug = project_slug(&long);
        assert!(slug.len() <= SLUG_CHARS && !slug.ends_with('-'), "{slug}");
        for odd in ["db_PASSWORD=hunter2 rotation", "!!!", "Group", ""] {
            let slug = project_slug(odd);
            assert!(slug.starts_with("p-") && slug.len() == 10, "{odd}: {slug}");
        }
        assert_eq!(slug_of(&NodeId("plan:abc:k1".into())), Some("abc"));
        assert_eq!(slug_of(&NodeId("orders:file:a".into())), None);
        assert!(is_project(&NodeId("plan:abc:project".into())));
    }

    #[test]
    fn an_item_key_is_a_short_lowercase_word() {
        for ok in ["a", "next-1", "retry_policy", &"k".repeat(KEY_MAX)] {
            assert!(key_ok(ok), "{ok}");
        }
        for bad in ["", "project", "-a", "_a", "A", "a b", "a:b", &"k".repeat(KEY_MAX + 1)] {
            assert!(!key_ok(bad), "{bad}");
        }
    }
}
