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
    /// reads a query shape's: `kinds`, a table naming the kind of each role the built-in emits,
    /// and for a built-in that emits edges, `relations`, naming the relation of each such role.
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

/// `relations = { ROLE = RELATION }`, one role per relation a built-in emits.
const fn relations(roles: &'static [Param]) -> Param {
    Param { key: "relations", names: ParamNames::Table(roles) }
}

/// A role of `relations`, which names a relation.
const fn relation(key: &'static str) -> Param {
    Param { key, names: ParamNames::Relation }
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
    Builtin {
        name: crate::manifest::NAME,
        builtin: "manifest",
        role: Role::Extractor,
        consumes: &[],
        options: &[kinds(&[role("package"), role("library"), role("repo"), role("directory")])],
    },
    Builtin {
        name: crate::openapi::NAME,
        builtin: "openapi",
        role: Role::Extractor,
        consumes: &[],
        options: &[kinds(&[role("spec"), role("repo"), role("directory")])],
    },
    Builtin {
        name: crate::history::NAME,
        builtin: "git",
        role: Role::Extractor,
        consumes: &[],
        options: &[
            kinds(&[role("repo"), role("file"), role("person")]),
            relations(&[relation("co_change"), relation("owned_by")]),
        ],
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
        // A role of `kinds` names a kind as the engine fills it in; a role of `relations`, a name.
        let value = |names: ParamNames| match names {
            ParamNames::Relation => json!("p.r"),
            _ => kind.clone(),
        };
        for entry in EXTRACTORS {
            let mut all = serde_json::Map::new();
            for param in entry.options {
                let ParamNames::Table(roles) = param.names else {
                    panic!("{}: option {} is a table of roles", entry.name, param.key)
                };
                assert!(matches!(param.key, "kinds" | "relations"), "{}: {}", entry.name, param.key);
                let table: serde_json::Map<String, Value> =
                    roles.iter().map(|r| (r.key.to_owned(), value(r.names))).collect();
                all.insert(param.key.to_owned(), Value::Object(table));
            }
            assert!(all.contains_key("kinds"), "{}: options hold a kinds table", entry.name);
            // Whether the options are refused: a built-in that reads more than the walk's files,
            // such as `git`'s history, may still fail to read the repo, which is no refusal.
            let refused = |options: &serde_json::Map<String, Value>| {
                let options = Value::Object(options.clone());
                matches!(
                    crate::builtin(entry.builtin).unwrap().run(&repo, &[], &options),
                    Err(crate::ExtractError::InvalidOptions(_))
                )
            };
            assert!(!refused(&all), "{}", entry.name);
            let extractor = crate::builtin(entry.builtin).unwrap();
            if !extractor.reads_history() {
                assert!(extractor.run(&repo, &[], &Value::Object(all.clone())).is_ok(), "{}", entry.name);
            }
            for param in entry.options {
                let ParamNames::Table(roles) = param.names else { unreachable!() };
                for role in roles {
                    let mut less = all.clone();
                    less[param.key].as_object_mut().unwrap().remove(role.key);
                    assert!(refused(&less), "{} runs without {}.{}", entry.name, param.key, role.key);
                }
                let mut more = all.clone();
                more[param.key].as_object_mut().unwrap().insert("extra".into(), value(roles[0].names));
                assert!(refused(&more), "{} reads an undeclared role of {}", entry.name, param.key);
            }
        }
    }

    #[test]
    fn an_unknown_builtin_and_an_unjoined_hint_kind_are_not_found() {
        assert_eq!(find("nope"), None);
        assert!(!consumed("code.nope"));
    }
}
