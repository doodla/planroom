//! The lookup tables the fold keeps by event ID and command key: std's hash map, whose SipHash is
//! keyed at random for each table, so no ID a writer chooses can steer lookups into collisions.
//! Nothing iterates one to build output, so their order never reaches what a fold returns;
//! `Debug` lists the entries sorted, so it doesn't depend on that order either.

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

/// A map for lookups by key.
#[derive(Clone, PartialEq)]
pub(crate) struct Lookup<K: Hash + Eq, V>(HashMap<K, V>);

impl<K: Hash + Eq, V> Default for Lookup<K, V> {
    fn default() -> Self {
        Lookup(HashMap::new())
    }
}

impl<K: Hash + Eq, V> Lookup<K, V> {
    pub(crate) fn get<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.0.get(key)
    }

    pub(crate) fn get_mut<Q: Hash + Eq + ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        self.0.get_mut(key)
    }

    pub(crate) fn contains_key<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.0.contains_key(key)
    }

    /// Insert `value` under `key`, and return the value it replaced.
    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    pub(crate) fn entry(&mut self, key: K) -> std::collections::hash_map::Entry<'_, K, V> {
        self.0.entry(key)
    }

    pub(crate) fn reserve(&mut self, more: usize) {
        self.0.reserve(more);
    }
}

impl<K: Hash + Eq + Ord + fmt::Debug, V: fmt::Debug> fmt::Debug for Lookup<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut entries: Vec<(&K, &V)> = self.0.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        f.debug_map().entries(entries).finish()
    }
}

impl<K: Hash + Eq, V> FromIterator<(K, V)> for Lookup<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Lookup(iter.into_iter().collect())
    }
}

/// A set for membership tests.
#[derive(Clone, PartialEq)]
pub(crate) struct LookupSet<K: Hash + Eq>(HashSet<K>);

impl<K: Hash + Eq> Default for LookupSet<K> {
    fn default() -> Self {
        LookupSet(HashSet::new())
    }
}

impl<K: Hash + Eq> LookupSet<K> {
    pub(crate) fn contains<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.0.contains(key)
    }

    /// The members, in no order: only for a caller that collects them into an ordered set or
    /// tests each one.
    pub(crate) fn unordered(&self) -> impl Iterator<Item = &K> {
        self.0.iter()
    }
}

impl<K: Hash + Eq + Ord + fmt::Debug> fmt::Debug for LookupSet<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut entries: Vec<&K> = self.0.iter().collect();
        entries.sort();
        f.debug_set().entries(entries).finish()
    }
}

impl<K: Hash + Eq> FromIterator<K> for LookupSet<K> {
    fn from_iter<I: IntoIterator<Item = K>>(iter: I) -> Self {
        LookupSet(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_lists_a_lookup_sorted_whatever_order_it_holds() {
        let keys = ["ev-9", "ev-1", "ev-5", "ev-3", "ev-7", "ev-2"];
        let map: Lookup<String, usize> = keys.iter().enumerate().map(|(i, k)| ((*k).to_owned(), i)).collect();
        assert_eq!(
            format!("{map:?}"),
            r#"{"ev-1": 1, "ev-2": 5, "ev-3": 3, "ev-5": 2, "ev-7": 4, "ev-9": 0}"#
        );
        let set: LookupSet<String> = keys.iter().map(|k| (*k).to_owned()).collect();
        assert_eq!(format!("{set:?}"), r#"{"ev-1", "ev-2", "ev-3", "ev-5", "ev-7", "ev-9"}"#);
        assert!(set.contains("ev-5") && !set.contains("ev-4"));
        assert_eq!(map.get("ev-7"), Some(&4));
    }
}
