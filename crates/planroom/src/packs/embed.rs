//! The built-in packs, embedded at build time by `build.rs` from every directory under `packs/`,
//! so a new built-in pack needs no code edit and the binary never reads `packs/` at run time.

use std::collections::BTreeMap;

use super::{Origin, Source};

mod files {
    include!(concat!(env!("OUT_DIR"), "/packs.rs"));
}

/// Each embedded pack's files, one source per directory under `packs/`, in directory order.
pub fn sources() -> Vec<Source> {
    let mut by_dir: BTreeMap<&str, BTreeMap<String, Vec<u8>>> = BTreeMap::new();
    for (dir, path, bytes) in files::FILES {
        by_dir.entry(dir).or_default().insert((*path).to_owned(), bytes.to_vec());
    }
    by_dir
        .into_iter()
        .map(|(dir, files)| Source {
            origin: Origin::BuiltIn,
            label: format!("packs/{dir}"),
            dir: None,
            files,
        })
        .collect()
}

/// The directory names of the embedded packs.
pub fn dirs() -> Vec<&'static str> {
    let mut dirs: Vec<&str> = files::FILES.iter().map(|(dir, _, _)| *dir).collect();
    dirs.dedup();
    dirs
}
