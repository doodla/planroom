//! The command catalogue's undo classes (architecture.md, One command catalogue; DR-55, DR-81):
//! every leaf says how its effect is undone, and `share` alone can't be.

use std::collections::BTreeMap;

use planroom_contract::{Command, Undo};

/// Every leaf this build serves, with its undo class. A new leaf joins this list with its class;
/// a new `not_undoable` one needs a decision record first (DR-55).
const SNAPSHOT: &[(&str, &str)] = &[
    ("doctor", "query"),
    ("edit", "edit"),
    ("find", "query"),
    ("focus", "query"),
    ("import", "edit"),
    ("map", "query"),
    ("open", "query"),
    ("person init", "query"),
    ("scan", "query"),
    ("schema", "query"),
    ("serve", "query"),
    ("share", "not_undoable"),
    ("version", "query"),
    ("watch", "query"),
];

fn class(undo: Undo) -> &'static str {
    match undo {
        Undo::Edit => "edit",
        Undo::Compensate => "compensate",
        Undo::Query => "query",
        Undo::NotUndoable(_) => "not_undoable",
    }
}

#[test]
fn undo_classes_snapshot() {
    let found: BTreeMap<String, &str> = Command::SPECS.iter().map(|s| (s.name(), class(s.undo))).collect();
    let want: BTreeMap<String, &str> = SNAPSHOT.iter().map(|(leaf, c)| ((*leaf).to_owned(), *c)).collect();
    assert_eq!(found, want, "every leaf's undo class, as the catalogue declares it");

    // `share` is the only leaf Planroom can't take back, and it says why.
    let not_undoable: Vec<(String, &str)> = Command::SPECS
        .iter()
        .filter_map(|s| match s.undo {
            Undo::NotUndoable(why) => Some((s.name(), why)),
            _ => None,
        })
        .collect();
    assert_eq!(not_undoable.len(), 1);
    assert_eq!(not_undoable[0].0, "share");
    assert!(!not_undoable[0].1.is_empty(), "a not-undoable leaf gives its reason");

    // An edit-class leaf that writes user-authored state needs a base, so its undo can carry one.
    let edit = Command::SPECS.iter().find(|s| s.name() == "edit").unwrap();
    assert!(edit.needs_base);
    // The listing `schema --commands` gives agrees.
    for leaf in Command::listing() {
        assert_eq!(Some(leaf.undo.as_str()), want.get(&leaf.name).copied(), "{}", leaf.name);
    }
}
