//! The repos with uncommitted work: `meta.dirty_repos` on `scan`, `focus` and `map`, and the one
//! line that says so (DR-71, DR-85).
//!
//! The map holds only committed lines, so a changed, deleted or untracked file never reaches it.
//! Each read runs `git status` in every repo of the workspace, with no rescan, and names the
//! repos it finds files in. Repo names are repo text: the field carries the redaction mark, and
//! the line goes out through the same redaction as every other line.

use std::path::Path;

use planroom_contract::DirtyRepo;
use planroom_extract::git::Git;

use crate::app::Paged;
use crate::app::workspace::{self, Workspace};
use crate::text;

/// Repos `meta.dirty_repos` lists at most; `meta.more_dirty_repos` counts the rest, so a reply
/// keeps to its budget in a workspace of many repos.
pub const LISTED: usize = 5;
/// Repos the line names before "and N more".
const LINE_NAMES: usize = 3;
/// Characters of a repo's name the line quotes.
const NAME_CHARS: usize = 40;

/// The repos with uncommitted files, the first [`LISTED`] by name, and how many more there are.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dirty {
    pub repos: Vec<DirtyRepo>,
    pub more: u32,
}

impl Dirty {
    /// The repos of `ws` whose working tree holds files `HEAD` doesn't. A repo git can't read is
    /// left out: the scan says why with `repo_skipped`.
    pub fn of(ws: &Workspace) -> Dirty {
        let mut all: Vec<DirtyRepo> = ws
            .repos
            .iter()
            .filter_map(|repo| {
                let files = Git::new(&repo.path).uncommitted_files().ok()?;
                (files > 0).then(|| DirtyRepo { repo: repo.name.clone(), files })
            })
            .collect();
        all.sort_by(|a, b| a.repo.cmp(&b.repo));
        let more = all.len().saturating_sub(LISTED) as u32;
        all.truncate(LISTED);
        Dirty { repos: all, more }
    }

    /// [`Dirty::of`] the workspace rooted at `root`; none when it can't be discovered.
    pub fn of_root(root: &Path) -> Dirty {
        workspace::discover(root).map(|ws| Dirty::of(&ws)).unwrap_or_default()
    }

    /// The one line a person reads: "orders: 3 uncommitted files aren't on the map." With more
    /// than one repo, the first few by name with their counts, and how many more.
    pub fn line(&self) -> Option<String> {
        let first = self.repos.first()?;
        let total = self.repos.len() + self.more as usize;
        if total == 1 {
            let repo = text::short(&first.repo, NAME_CHARS);
            let key = if first.files == 1 { "dirty.one_file" } else { "dirty.one" };
            return Some(text::get(key, &[("repo", &repo), ("files", &first.files)]));
        }
        let shown: Vec<String> = self
            .repos
            .iter()
            .take(LINE_NAMES)
            .map(|r| {
                text::get("dirty.repo", &[("repo", &text::short(&r.repo, NAME_CHARS)), ("files", &r.files)])
            })
            .collect();
        let rest = total - shown.len();
        let names = if rest == 0 {
            shown.join(", ")
        } else {
            text::get("map.more", &[("names", &shown.join(", ")), ("more", &rest)])
        };
        Some(text::get("dirty.many", &[("count", &total), ("names", &names)]))
    }
}

/// `page` with the repos of the workspace `dir` belongs to that hold uncommitted work: how `map`
/// carries them, while `import`'s read answers, which reuse its page, don't run `git status`.
pub fn attach<T>(mut page: Paged<T>, dir: &Path) -> Paged<T> {
    if let Some(root) = workspace::find_root(dir) {
        page.dirty = Dirty::of_root(&root);
    }
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirty(names: &[&str], more: u32) -> Dirty {
        let repos = names.iter().map(|n| DirtyRepo { repo: (*n).to_owned(), files: 2 }).collect();
        Dirty { repos, more }
    }

    #[test]
    fn the_line_names_one_repo_or_a_few_and_counts_the_rest() {
        assert_eq!(Dirty::default().line(), None);
        assert_eq!(dirty(&["orders"], 0).line().unwrap(), "orders: 2 uncommitted files aren't on the map.");
        let one = Dirty { repos: vec![DirtyRepo { repo: "web".into(), files: 1 }], more: 0 };
        assert_eq!(one.line().unwrap(), "web: 1 uncommitted file isn't on the map.");
        let line = dirty(&["a", "b", "c", "d", "e"], 4).line().unwrap();
        assert_eq!(
            line,
            "9 repos have uncommitted files that aren't on the map: a (2), b (2), c (2); and 6 more."
        );
        let long = "x".repeat(300);
        assert!(dirty(&[&long, "b"], 0).line().unwrap().len() < 120, "a long name is cut");
    }
}
