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

/// [`find_cycle`] over a `parent_of` that lends each parent's ID instead of copying it: the walk
/// copies the IDs of a loop only when it finds one.
pub(crate) fn find_cycle_by<'a>(
    child: &'a NodeId,
    new_parent: &'a NodeId,
    parent_of: impl Fn(&NodeId) -> Option<&'a NodeId>,
) -> Option<Vec<NodeId>> {
    // The first steps are checked against a short list; a longer walk keeps a set, so no chain
    // costs more than its length times a set lookup.
    const SHORT: usize = 16;
    let mut short: Vec<&NodeId> = Vec::new();
    let mut seen: BTreeSet<&NodeId> = BTreeSet::new();
    let mut at = new_parent;
    loop {
        if at == child {
            let mut path = vec![child.clone()];
            let mut step = new_parent;
            loop {
                path.push(step.clone());
                if step == child {
                    return Some(path);
                }
                step = parent_of(step).expect("the walk reached the child");
            }
        }
        let repeated = if short.len() < SHORT {
            let repeated = short.contains(&at);
            short.push(at);
            repeated
        } else {
            if seen.is_empty() {
                seen.extend(short.iter().copied());
            }
            !seen.insert(at)
        };
        if repeated {
            return None;
        }
        at = parent_of(at)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use proptest::prelude::*;
    use proptest::test_runner::Config;

    proptest! {
        // No I/O in the kernel's tests: a failing case is printed, never written to a file.
        #![proptest_config(Config { failure_persistence: None, ..Config::default() })]

        /// The walk that borrows IDs finds the loop the walk that copies them finds, short chains
        /// and long ones, with loops through the child, above it, or none.
        #[test]
        fn a_borrowing_walk_finds_what_a_copying_walk_finds(
            parents in prop::collection::btree_map(0u16..60, 0u16..64, 0..60),
            child in 0u16..64,
            new_parent in 0u16..64,
        ) {
            let ids: Vec<NodeId> = (0..64).map(|k| id(&k.to_string())).collect();
            let parents: BTreeMap<&NodeId, &NodeId> =
                parents.iter().map(|(k, p)| (&ids[*k as usize], &ids[*p as usize])).collect();
            let copied = find_cycle(&ids[child as usize], &ids[new_parent as usize], |n| parents.get(n).map(|p| (*p).clone()));
            let borrowed = find_cycle_by(&ids[child as usize], &ids[new_parent as usize], |n| parents.get(n).copied());
            prop_assert_eq!(copied, borrowed);
        }

        /// The same over one long chain, so the walk passes its short list.
        #[test]
        fn a_borrowing_walk_matches_on_a_long_chain(len in 1u16..80, child in 0u16..90, new_parent in 0u16..90, back in 0u16..90) {
            let ids: Vec<NodeId> = (0..90).map(|k| id(&k.to_string())).collect();
            // k's parent is k + 1 up to `len`, whose parent is `back`.
            let parent_of = |n: &NodeId| -> Option<usize> {
                let k: u16 = n.0.rsplit(':').next()?.parse().ok()?;
                match k.cmp(&len) {
                    std::cmp::Ordering::Less => Some(usize::from(k) + 1),
                    std::cmp::Ordering::Equal => Some(usize::from(back)),
                    std::cmp::Ordering::Greater => None,
                }
            };
            let copied = find_cycle(&ids[child as usize], &ids[new_parent as usize], |n| parent_of(n).map(|p| ids[p].clone()));
            let borrowed = find_cycle_by(&ids[child as usize], &ids[new_parent as usize], |n| parent_of(n).map(|p| &ids[p]));
            prop_assert_eq!(copied, borrowed);
        }
    }

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
