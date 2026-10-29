//! The universal `git` built-in: a repo's history at the snapshot's commit, read with one
//! `git log` (DR-16), as the people who wrote it, who owns the repo, and which files change
//! together.
//!
//! - **The bound.** The history is the commit's own ancestry, which on the tracked branch is that
//!   branch's history, newest first, up to its last [`BOUND`] commits; older history is reported
//!   as `partial_scan`'s `history before <sha>` (DR-47). Its facts are a function of the commit
//!   alone, as every snapshot's are (DR-85): no branch or tag is read. A shallow clone's boundary
//!   cuts the ancestry short as well, and is reported the same way; the boundaries the read
//!   reached key the snapshot, as a partial clone's missing blobs do.
//! - **People.** A person node per author, keyed by the first 12 hex characters of the sha256 of
//!   the lowercased email, named by the author's name, under the repo. The email itself is never
//!   stored: the key can't be read back, and the name is repo text the map masks.
//! - **Ownership.** The repo is owned by its top author: the most commits, then the smaller key.
//! - **Co-change.** Pairs of the snapshot's files that changed together in at least
//!   [`MIN_TOGETHER`] commits, the top [`TOP_PAIRS`] by count, then by path. A commit that touches
//!   more than [`WIDE`] files, such as a reformat or a vendored import, says nothing about which
//!   files belong together, so it's left out of co-change.
//!
//! Evidence: each fact cites the newest commit it was read from, as a `Ref` with no ref name,
//! which ingest checks with `git cat-file`.

use std::collections::{BTreeMap, HashMap};

use planroom_contract::{
    Edge, Evidence, FACTS_FORMAT, FactBatch, Kind, Layer, Node, NodeId, Provenance, Ref, Tier,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::git::Git;
use crate::ids::{edge_id, hex, node_id};
use crate::{ExtractError, Extractor, KindRef, RepoPath, RepoView};

pub const NAME: &str = "code/git";

/// The most commits one scan reads of a repo's history (DR-47).
pub const BOUND: usize = 5_000;

/// The fewest commits a pair of files must change together in to count as co-changing.
pub const MIN_TOGETHER: u32 = 2;

/// The most co-change pairs one repo keeps.
pub const TOP_PAIRS: usize = 50;

/// A commit touching more files than this is left out of co-change.
pub const WIDE: usize = 100;

/// The most pairs the count holds at once. Past it, the pairs seen least are dropped, rarest
/// first, so memory stays bounded however the history runs; the input order is fixed, so the
/// result is the same on every machine (DR-47).
const PAIR_CAP: usize = 1_000_000;

/// The kinds this built-in emits, which the pack that declares it names in `options.kinds`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kinds {
    /// The repo's own node, which the `files` built-in emits: the people's parent and the owned.
    pub repo: KindRef,
    /// A file node, which the `files` built-in emits: the ends of a co-change edge.
    pub file: KindRef,
    pub person: KindRef,
}

/// The relations this built-in emits, qualified, as the pack names them in `options.relations`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relations {
    pub co_change: String,
    pub owned_by: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    kinds: Kinds,
    relations: Relations,
}

pub struct History;

impl Extractor for History {
    fn name(&self) -> &'static str {
        NAME
    }

    fn run(&self, repo: &RepoView, files: &[RepoPath], opts: &Value) -> Result<FactBatch, ExtractError> {
        let opts: Options = serde_json::from_value(opts.clone())
            .map_err(|e| ExtractError::InvalidOptions(format!("{NAME}: {e}")))?;
        let read = read(&Git::new(&repo.root), &repo.commit, files, BOUND)?;
        Ok(facts(repo, &opts, &read))
    }

    fn reads_history(&self) -> bool {
        true
    }

    /// Where the read stopped short of the commit's whole history: the bound's cut, then each
    /// shallow clone boundary it reached ([`Git::shallow_cut`]).
    fn uncovered(&self, repo: &RepoView) -> Result<Vec<String>, ExtractError> {
        let git = Git::new(&repo.root);
        let cut = git.history_cut(&repo.commit, BOUND).map_err(ExtractError::Git)?;
        let shallow = git.shallow_cut(&repo.commit, BOUND).map_err(ExtractError::Git)?;
        Ok(cut.into_iter().chain(shallow).map(|sha| format!("history before {sha}")).collect())
    }
}

/// One author over the history read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    /// The first 12 hex characters of the sha256 of the lowercased email: the person's key.
    pub key: String,
    /// The name on the author's newest commit.
    pub name: String,
    pub commits: u32,
    /// The author's newest commit.
    pub newest: String,
}

/// Two files that changed together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    /// The two paths, in sorted order.
    pub files: (String, String),
    pub count: u32,
    /// The newest commit that changed both.
    pub newest: String,
}

/// What one history read found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Read {
    /// By key.
    pub authors: BTreeMap<String, Author>,
    /// The top pairs, most commits first, then by path.
    pub pairs: Vec<Pair>,
    /// How many commits were read: at most the bound.
    pub commits: usize,
}

impl Read {
    /// The top author: the most commits, then the smaller key.
    pub fn owner(&self) -> Option<&Author> {
        self.authors.values().max_by(|a, b| a.commits.cmp(&b.commits).then_with(|| b.key.cmp(&a.key)))
    }
}

/// A person's key: the first 12 hex characters of the sha256 of the lowercased email.
pub fn person_key(email: &str) -> String {
    hex(&Sha256::digest(email.to_lowercase().as_bytes()))[..12].to_owned()
}

/// Read the newest `bound` commits of `commit`'s history: its authors, and the co-change of
/// `files`, the snapshot's own, so every edge joins two of its nodes.
pub fn read(git: &Git, commit: &str, files: &[RepoPath], bound: usize) -> Result<Read, ExtractError> {
    let index: HashMap<&[u8], u32> =
        files.iter().enumerate().map(|(i, f)| (f.0.as_bytes(), i as u32)).collect();
    let mut shas: Vec<String> = Vec::new();
    let mut authors: BTreeMap<String, Author> = BTreeMap::new();
    let mut counts = PairCounts::default();
    git.log(commit, bound, WIDE + 1, |c| {
        let at = shas.len() as u32;
        let key = person_key(&c.email);
        let author = authors.entry(key.clone()).or_insert_with(|| Author {
            key,
            name: c.name.clone(),
            commits: 0,
            newest: c.sha.clone(),
        });
        author.commits += 1;
        if c.touched <= WIDE {
            let mut ids: Vec<u32> = c.files.iter().filter_map(|f| index.get(f.as_slice()).copied()).collect();
            ids.sort_unstable();
            ids.dedup();
            counts.add(&ids, at);
        }
        shas.push(c.sha);
    })
    .map_err(ExtractError::Git)?;
    let pairs = counts
        .top()
        .into_iter()
        .map(|((a, b), (count, newest))| Pair {
            files: (files[a as usize].0.clone(), files[b as usize].0.clone()),
            count,
            newest: shas[newest as usize].clone(),
        })
        .collect();
    Ok(Read { authors, pairs, commits: shas.len() })
}

/// Co-change counts by pair of file indices: how many commits, and the newest commit's place.
#[derive(Default)]
struct PairCounts {
    pairs: HashMap<(u32, u32), (u32, u32)>,
    /// The count below which pairs were dropped to keep to [`PAIR_CAP`]; 0 until then.
    floor: u32,
}

impl PairCounts {
    /// Count every pair of `ids`, sorted and distinct, as changed together at commit `at`.
    fn add(&mut self, ids: &[u32], at: u32) {
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                let entry = self.pairs.entry((*a, *b)).or_insert((0, at));
                entry.0 += 1;
            }
        }
        while self.pairs.len() > PAIR_CAP {
            self.floor += 1;
            let floor = self.floor;
            self.pairs.retain(|_, (count, _)| *count > floor);
        }
    }

    /// The top pairs that changed together often enough, most commits first, then by index,
    /// which is sorted path order.
    fn top(self) -> Vec<((u32, u32), (u32, u32))> {
        let mut top: Vec<_> =
            self.pairs.into_iter().filter(|(_, (count, _))| *count >= MIN_TOGETHER).collect();
        top.sort_unstable_by(|(pa, (ca, _)), (pb, (cb, _))| cb.cmp(ca).then(pa.cmp(pb)));
        top.truncate(TOP_PAIRS);
        top
    }
}

/// The facts of one read: a person per author under the repo, the repo's owner, and the top
/// co-change pairs, sorted by ID.
fn facts(repo: &RepoView, opts: &Options, read: &Read) -> FactBatch {
    let provenance = Provenance::Extracted { extractor: NAME.into(), tier: Tier::Universal };
    let evidence = |commit: &str| vec![Evidence::Ref { commit: commit.to_owned(), ref_name: None }];
    let kinds = &opts.kinds;
    let repo_id = node_id(&repo.name, &kinds.repo.id_prefix, &repo.name);
    let person_id = |key: &str| node_id(&repo.name, &kinds.person.id_prefix, key);
    let edge = |kind: &str, from: NodeId, to: NodeId, props: BTreeMap<String, Value>, commit: &str| {
        let kind = Kind(kind.to_owned());
        let (from, to) = (Ref::Id(from), Ref::Id(to));
        Edge {
            id: edge_id(&kind, &from, &to, None),
            kind,
            from,
            to,
            via: None,
            via_ref: None,
            props,
            evidence: evidence(commit),
            confidence: 1.0,
            layer: Layer::Actual,
            provenance: provenance.clone(),
        }
    };
    let mut nodes: Vec<Node> = read
        .authors
        .values()
        .map(|a| Node {
            id: person_id(&a.key),
            kind: Kind(kinds.person.kind.clone()),
            level: kinds.person.level.clone(),
            parent: Some(repo_id.clone()),
            name: a.name.clone(),
            aliases: vec![],
            props: BTreeMap::new(),
            layer: Layer::Actual,
            provenance: provenance.clone(),
            evidence: evidence(&a.newest),
        })
        .collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let mut edges = Vec::new();
    if let Some(owner) = read.owner() {
        let to = person_id(&owner.key);
        edges.push(edge(&opts.relations.owned_by, repo_id.clone(), to, BTreeMap::new(), &owner.newest));
    }
    for pair in &read.pairs {
        let file = |path: &str| node_id(&repo.name, &kinds.file.id_prefix, path);
        let props = BTreeMap::from([("count".to_owned(), Value::from(pair.count))]);
        edges.push(edge(
            &opts.relations.co_change,
            file(&pair.files.0),
            file(&pair.files.1),
            props,
            &pair.newest,
        ));
    }
    edges.sort_by(|a, b| a.id.cmp(&b.id));
    FactBatch {
        format: FACTS_FORMAT.into(),
        extractor: NAME.into(),
        tier: Tier::Universal,
        nodes,
        edges,
        hints: vec![],
        partial: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_person_key_is_12_hex_of_the_lowercased_email() {
        let key = person_key("Alice@Acme.TEST");
        assert_eq!(key, person_key("alice@acme.test"));
        assert_eq!(key.len(), 12);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(key, person_key("bob@acme.test"));
    }

    #[test]
    fn pairs_need_two_commits_and_keep_the_newest_and_the_top_fifty() {
        let mut counts = PairCounts::default();
        // Commit 0 is the newest: it changed 0, 1 and 2.
        counts.add(&[0, 1, 2], 0);
        counts.add(&[0, 1], 1);
        counts.add(&[1, 2], 2);
        counts.add(&[0, 1], 3);
        let top = counts.top();
        assert_eq!(top, [((0, 1), (3, 0)), ((1, 2), (2, 0))], "0-2 changed together once");

        let mut counts = PairCounts::default();
        for at in 0..2 {
            counts.add(&(0..20).collect::<Vec<u32>>(), at);
        }
        let top = counts.top();
        assert_eq!(top.len(), TOP_PAIRS);
        assert_eq!(top[0].0, (0, 1));
        assert!(top.windows(2).all(|w| w[0].0 < w[1].0), "ties sort by path order");
    }

    #[test]
    fn the_pair_count_stays_under_its_cap_and_keeps_pairs_seen_most() {
        let mut counts = PairCounts::default();
        // A pair seen in every commit, among pairs each seen once, past the cap.
        let mut at = 0;
        let mut next = 2;
        while next < 2 * PAIR_CAP as u32 + 4 {
            counts.add(&[0, 1], at);
            counts.add(&[next, next + 1], at);
            next += 2;
            at += 1;
        }
        assert!(counts.pairs.len() <= PAIR_CAP);
        assert_eq!(counts.top()[0].0, (0, 1));
    }

    #[test]
    fn the_owner_has_the_most_commits_and_a_tie_takes_the_smaller_key() {
        let author =
            |key: &str, commits| Author { key: key.into(), name: key.into(), commits, newest: "c".into() };
        let mut read = Read::default();
        for a in [author("b", 3), author("a", 3), author("c", 1)] {
            read.authors.insert(a.key.clone(), a);
        }
        assert_eq!(read.owner().unwrap().key, "a");
    }
}
