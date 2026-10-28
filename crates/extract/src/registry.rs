//! The built-ins this build has: every extractor and linker a pack may name, each with the hint
//! kinds it consumes.
//!
//! The pack loader reads this to refuse a pack that names a built-in this build doesn't have, and
//! a `[[hints]]` kind no loaded linker consumes, both with `pack_invalid` (DR-9). Only built-in
//! linkers join hints, so a new join is a new entry in [`LINKERS`].

use planroom_contract::{Param, ParamNames};

/// Whether a built-in reads one repo's files or joins hints across the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Extractor,
    Linker,
}

/// One built-in extractor or linker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Builtin {
    /// Pack-qualified, such as `code/compose`: what facts' provenance names.
    pub name: &'static str,
    /// What a pack's `builtin = NAME` names, such as `compose`.
    pub builtin: &'static str,
    pub role: Role,
    /// The hint kinds it consumes, pack-qualified, such as `code.env_set`. An extractor reads
    /// files, not hints, so it consumes none.
    pub consumes: &'static [&'static str],
    /// The options a pack sets, as a table of parameters the loader reads them through, as it
    /// reads a query shape's: `kinds`, a table naming the kind of each role the built-in emits.
    /// Every key is required, since the built-in emits every role. A key the table doesn't
    /// declare is `pack_invalid`: what else a built-in reads, such as `files`'s merged
    /// `languages` or `compose`'s `image_roles` and `repos`, the engine fills in.
    pub options: &'static [Param],
}

/// `kinds = { ROLE = KIND }`, one role per kind a built-in emits.
const fn kinds(roles: &'static [Param]) -> Param {
    Param { key: "kinds", names: ParamNames::Table(roles) }
}

/// A role of `kinds`, which names a node kind.
const fn role(key: &'static str) -> Param {
    Param { key, names: ParamNames::NodeKind }
}

/// Every built-in extractor, in the order a scan runs them.
pub const EXTRACTORS: &[Builtin] = &[
    Builtin {
        name: crate::files::NAME,
        builtin: "files",
        role: Role::Extractor,
        consumes: &[],
        options: &[kinds(&[role("repo"), role("directory"), role("file")])],
    },
    Builtin {
        name: crate::compose::NAME,
        builtin: "compose",
        role: Role::Extractor,
        consumes: &[],
        options: &[kinds(&[role("service"), role("tooling"), role("repo")])],
    },
];

/// Every built-in linker. None is built yet; each lands with the hint kinds it joins.
pub const LINKERS: &[Builtin] = &[];

/// Every built-in, extractors first.
pub fn all() -> impl Iterator<Item = &'static Builtin> {
    EXTRACTORS.iter().chain(LINKERS)
}

/// The built-in a pack names with `builtin = NAME`, or `None` when this build has none by that name.
pub fn find(builtin: &str) -> Option<&'static Builtin> {
    all().find(|b| b.builtin == builtin)
}

/// Whether some built-in linker consumes the hint kind `kind`.
pub fn consumed(kind: &str) -> bool {
    LINKERS.iter().any(|l| l.consumes.contains(&kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_extractor_runs_under_its_name() {
        for entry in EXTRACTORS {
            let extractor = crate::builtin(entry.builtin).expect("a registered extractor this build has");
            assert_eq!(extractor.name(), entry.name);
            assert_eq!(find(entry.builtin), Some(entry));
        }
    }

    #[test]
    fn names_are_pack_qualified_and_unique() {
        let mut names: Vec<&str> = all().map(|b| b.name).collect();
        for name in &names {
            let (pack, short) = name.split_once('/').expect("a pack-qualified name");
            assert!(!pack.is_empty() && !short.is_empty(), "{name}");
        }
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn roles_match_the_lists_and_hint_kinds_are_pack_qualified() {
        assert!(EXTRACTORS.iter().all(|b| b.role == Role::Extractor && b.consumes.is_empty()));
        for linker in LINKERS {
            assert_eq!(linker.role, Role::Linker);
            assert!(linker.consumes.iter().all(|k| k.split_once('.').is_some()), "{}", linker.name);
        }
    }

    /// The options table is what each extractor reads: every role it declares runs, and one
    /// left out, or one more, is refused.
    #[test]
    fn each_options_table_names_exactly_the_roles_its_extractor_reads() {
        use serde_json::{Value, json};
        let repo = crate::RepoView {
            name: "r".into(),
            root: "/nonexistent".into(),
            commit: "c".repeat(40),
            blobs: crate::walk::Blobs::default(),
        };
        let kind = json!({"kind": "p.k", "id_prefix": "k", "level": "code"});
        for entry in EXTRACTORS {
            let [Param { key: "kinds", names: ParamNames::Table(roles) }] = entry.options else {
                panic!("{}: options are a kinds table", entry.name)
            };
            let all: serde_json::Map<String, Value> =
                roles.iter().map(|r| (r.key.to_owned(), kind.clone())).collect();
            let run = |kinds: &serde_json::Map<String, Value>| {
                crate::builtin(entry.builtin).unwrap().run(&repo, &[], &json!({ "kinds": kinds }))
            };
            assert!(run(&all).is_ok(), "{}", entry.name);
            for role in *roles {
                let mut less = all.clone();
                less.remove(role.key);
                assert!(run(&less).is_err(), "{} runs without {}", entry.name, role.key);
            }
            let mut more = all.clone();
            more.insert("extra".into(), kind.clone());
            assert!(run(&more).is_err(), "{} reads an undeclared role", entry.name);
        }
    }

    #[test]
    fn an_unknown_builtin_and_an_unjoined_hint_kind_are_not_found() {
        assert_eq!(find("nope"), None);
        assert!(!consumed("code.nope"));
    }
}
