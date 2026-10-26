//! The built-ins this build has: every extractor and linker a pack may name, each with the hint
//! kinds it consumes.
//!
//! The pack loader reads this to refuse a pack that names a built-in this build doesn't have, and
//! a `[[hints]]` kind no loaded linker consumes, both with `pack_invalid` (DR-9). Only built-in
//! linkers join hints, so a new join is a new entry in [`LINKERS`].

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
}

/// Every built-in extractor, in the order a scan runs them.
pub const EXTRACTORS: &[Builtin] = &[
    Builtin { name: crate::files::NAME, builtin: "files", role: Role::Extractor, consumes: &[] },
    Builtin { name: crate::compose::NAME, builtin: "compose", role: Role::Extractor, consumes: &[] },
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

    #[test]
    fn an_unknown_builtin_and_an_unjoined_hint_kind_are_not_found() {
        assert_eq!(find("nope"), None);
        assert!(!consumed("code.nope"));
    }
}
