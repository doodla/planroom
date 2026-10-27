//! The pure parent-cycle check (DR-61): whether giving a node a new parent closes a loop.
//!
//! `store::commit` refuses such a change through [`crate::cas`], and the fold breaks any loop an
//! older store or an import holds, so the tree always ends.

use std::collections::BTreeSet;

use planroom_contract::NodeId;

/// The loop that giving `child` the parent `new_parent` would close, as
/// `[child, new_parent, ..., child]`, or `None` when the new parent's ancestor chain never reaches
/// `child`. `parent_of` gives each node's current parent. A loop above `new_parent` that doesn't
/// pass through `child` isn't this change's, so it returns `None` for it.
pub fn find_cycle(
    child: &NodeId,
    new_parent: &NodeId,
    parent_of: impl Fn(&NodeId) -> Option<NodeId>,
) -> Option<Vec<NodeId>> {
    let mut path = vec![child.clone()];
    let mut seen = BTreeSet::new();
    let mut at = new_parent.clone();
    loop {
        path.push(at.clone());
        if &at == child {
            return Some(path);
        }
        if !seen.insert(at.clone()) {
            return None;
        }
        at = parent_of(&at)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn id(s: &str) -> NodeId {
        NodeId(format!("r:x:{s}"))
    }

    #[test]
    fn a_move_under_a_descendant_names_the_loop() {
        let parents = BTreeMap::from([(id("b"), id("a")), (id("c"), id("b"))]);
        let cycle = find_cycle(&id("a"), &id("c"), |n| parents.get(n).cloned());
        assert_eq!(cycle, Some(vec![id("a"), id("c"), id("b"), id("a")]));
        assert_eq!(find_cycle(&id("c"), &id("a"), |n| parents.get(n).cloned()), None);
        assert_eq!(find_cycle(&id("a"), &id("a"), |_| None), Some(vec![id("a"), id("a")]));
    }

    #[test]
    fn a_loop_that_skips_the_child_is_not_its_cycle() {
        let parents = BTreeMap::from([(id("b"), id("c")), (id("c"), id("b"))]);
        assert_eq!(find_cycle(&id("a"), &id("b"), |n| parents.get(n).cloned()), None);
    }
}
