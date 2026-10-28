//! The `git` built-in over real history: its people, owners and co-change match the fixture's
//! truth on the tracked branch, it reads the commit it's given and never a ref of the same name,
//! and it reads no further back than its bound (DR-47).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use planroom_contract::{Edge, Evidence, FactBatch, Ref};
use planroom_extract::git::Git;
use planroom_extract::history::{self, person_key};
use planroom_extract::verify::Verifier;
use planroom_extract::{RepoPath, RepoView, builtin, walk};
use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn truth() -> Value {
    let text = std::fs::read_to_string(repo_root().join("examples/TRUTH.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The fixture's five repos, built fresh into `out`.
fn fixture(out: &Path) {
    let built = Command::new("python3")
        .arg(repo_root().join("examples/build-fixture"))
        .arg(out)
        .output()
        .expect("python3 runs examples/build-fixture");
    assert!(built.status.success(), "build-fixture: {}", String::from_utf8_lossy(&built.stderr));
}

/// git as a person runs it, as `name <email>` at a fixed date.
fn git_as(dir: &Path, who: (&str, &str), args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", who.0)
        .env("GIT_AUTHOR_EMAIL", who.1)
        .env("GIT_COMMITTER_NAME", who.0)
        .env("GIT_COMMITTER_EMAIL", who.1)
        .env("GIT_AUTHOR_DATE", "2026-10-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-10-01T00:00:00Z")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// The repo at `root`, named `name`, at its `HEAD`, with the walk's files.
fn view(root: &Path, name: &str) -> (RepoView, Vec<RepoPath>) {
    let git = Git::new(root);
    let head = git.head().unwrap();
    let walked = walk::walk(&git, &head.commit, &walk::Exclude::none()).unwrap();
    let view =
        RepoView { name: name.into(), root: root.to_path_buf(), commit: head.commit, blobs: walked.blobs };
    (view, walked.files)
}

/// The `git` built-in's facts, with the kinds and relations the code pack names for it, as the
/// engine fills them in from the loaded vocabulary.
fn history_facts(view: &RepoView, files: &[RepoPath]) -> FactBatch {
    let kind = |kind: &str, id_prefix: &str, level: &str| json!({"kind": kind, "id_prefix": id_prefix, "level": level});
    let opts = json!({
        "kinds": {
            "repo": kind("code.repo", "repo", "system"),
            "file": kind("code.file", "file", "code"),
            "person": kind("code.person", "person", "component"),
        },
        "relations": {"co_change": "code.co_change", "owned_by": "code.owned_by"},
    });
    builtin("git").expect("the git built-in").run(view, files, &opts).unwrap()
}

fn id(r: &Ref) -> &str {
    match r {
        Ref::Id(id) => &id.0,
        Ref::Alias(alias) => alias,
    }
}

fn edges<'a>(batch: &'a FactBatch, kind: &str) -> Vec<&'a Edge> {
    batch.edges.iter().filter(|e| e.kind.0 == kind).collect()
}

#[test]
fn co_change_and_owners_match_truth() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let truth = truth();
    let history = &truth["history"];
    let people = history["people"].as_object().unwrap();
    // Every person's key, from the truth's emails: the facts hold the key, never the email.
    let by_key: BTreeMap<String, &str> = people
        .iter()
        .map(|(handle, p)| (person_key(p["email"].as_str().unwrap()), handle.as_str()))
        .collect();

    for (repo, owner) in history["owners"].as_object().unwrap() {
        let (view, files) = view(&dir.path().join(repo), repo);
        let batch = history_facts(&view, &files);

        // No email reaches a fact, in any field.
        let text = serde_json::to_string(&batch).unwrap();
        for p in people.values() {
            assert!(!text.contains(p["email"].as_str().unwrap()), "{repo}: an email reached the facts");
        }
        // A person per author, named by the author's name, under the repo.
        let repo_id = format!("{repo}:repo:{repo}");
        let mut authors = BTreeSet::new();
        for node in &batch.nodes {
            assert_eq!(node.kind.0, "code.person", "{repo}: only people are nodes");
            assert_eq!(node.parent.as_ref().map(|p| p.0.as_str()), Some(repo_id.as_str()));
            let key = node.id.0.strip_prefix(&format!("{repo}:person:")).expect("a person ID");
            let handle = by_key.get(key).unwrap_or_else(|| panic!("{repo}: {key} is no truth author"));
            assert_eq!(node.name, people[*handle]["name"].as_str().unwrap());
            authors.insert(handle.to_string());
        }
        let truth_authors: BTreeSet<String> =
            history["authors"][repo].as_object().unwrap().keys().cloned().collect();
        assert_eq!(authors, truth_authors, "{repo}'s authors");

        // The repo is owned by its top author on the tracked branch.
        let owned = edges(&batch, "code.owned_by");
        assert_eq!(owned.len(), 1, "{repo}");
        assert_eq!(id(&owned[0].from), repo_id);
        let owner_key = person_key(people[owner.as_str().unwrap()]["email"].as_str().unwrap());
        assert_eq!(id(&owned[0].to), format!("{repo}:person:{owner_key}"), "{repo}'s owner");
    }

    // Co-change on the tracked branch: the top pair and the next best, with their counts.
    for (repo, expected) in history["co_change"].as_object().unwrap() {
        let (view, files) = view(&dir.path().join(repo), repo);
        let batch = history_facts(&view, &files);
        let mut pairs: Vec<(u64, String, String)> = edges(&batch, "code.co_change")
            .iter()
            .map(|e| (e.props["count"].as_u64().unwrap(), id(&e.from).to_owned(), id(&e.to).to_owned()))
            .collect();
        pairs.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| (&a.1, &a.2).cmp(&(&b.1, &b.2))));
        let node = |path: &Value| {
            format!("{repo}:file:{}", path.as_str().unwrap().strip_prefix(&format!("{repo}/")).unwrap())
        };
        let top = &expected["top_pair"];
        assert_eq!(
            (pairs[0].1.clone(), pairs[0].2.clone()),
            (node(&top[0]), node(&top[1])),
            "{repo}'s top pair"
        );
        assert_eq!(pairs[0].0, expected["commits_together"]["main"].as_u64().unwrap(), "{repo}'s top count");
        assert_eq!(pairs[1].0, expected["next_best"]["main"].as_u64().unwrap(), "{repo}'s next best");
        assert!(pairs.len() <= history::TOP_PAIRS && pairs.iter().all(|p| p.0 >= 2), "{pairs:?}");
    }
}

#[test]
fn a_tag_named_like_the_branch_keeps_the_repo() {
    // A tag that shares the checked-out branch's name. git reads `main` as the
    // tag, so history read by that name would be the tag's; the built-ins read the commit.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    std::fs::create_dir_all(root.join("src")).unwrap();
    let ada = ("Ada", "ada@example.test");
    let ben = ("Ben", "ben@example.test");
    std::fs::write(root.join("src/a.ts"), "export const a = 1;\n").unwrap();
    git_as(&root, ada, &["init", "-q", "-b", "main"]);
    git_as(&root, ada, &["add", "-A"]);
    git_as(&root, ada, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
    git_as(&root, ada, &["-c", "tag.gpgsign=false", "tag", "main"]);
    for n in 2..4 {
        std::fs::write(root.join("src/a.ts"), format!("export const a = {n};\n")).unwrap();
        std::fs::write(root.join(format!("src/b{n}.ts")), "export const b = 1;\n").unwrap();
        git_as(&root, ben, &["add", "-A"]);
        git_as(&root, ben, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "More"]);
    }
    let (view, files) = view(&root, "app");
    let tag = git_as(&root, ada, &["rev-parse", "refs/tags/main^{commit}"]);
    assert_ne!(view.commit, tag, "the tag names an older commit than the branch");

    // The files built-in keeps the repo and its directory, and both verify at the commit.
    let kind = |kind: &str, id_prefix: &str, level: &str| json!({"kind": kind, "id_prefix": id_prefix, "level": level});
    let opts = json!({"kinds": {"repo": kind("code.repo", "repo", "system"),
                                "directory": kind("code.directory", "dir", "component"),
                                "file": kind("code.file", "file", "code")}});
    let mut tree = builtin("files").unwrap().run(&view, &files, &opts).unwrap();
    let mut people = history_facts(&view, &files);
    let git = Git::new(&root);
    let refs = git.refs().unwrap();
    let mut verifier = Verifier::new("app", &view.commit, &refs, git.objects().unwrap());
    assert!(verifier.verify(&mut tree).unwrap().is_empty());
    assert!(verifier.verify(&mut people).unwrap().is_empty());
    let ids: BTreeSet<&str> = tree.nodes.iter().map(|n| n.id.0.as_str()).collect();
    assert!(ids.contains("app:repo:app") && ids.contains("app:dir:src"), "{ids:?}");

    // The history is the branch's: Ben's two commits own the repo, though the tag is all Ada's.
    let owned = edges(&people, "code.owned_by");
    assert_eq!(id(&owned[0].to), format!("app:person:{}", person_key(ben.1)));
    assert_eq!(people.nodes.len(), 2);
    let pair = edges(&people, "code.co_change");
    assert!(pair.is_empty(), "no two files changed together twice: {pair:?}");
}

#[test]
fn history_past_its_bound_is_reported_not_read() {
    // One commit past the bound, each changing one file, written in one fast-import.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("long");
    std::fs::create_dir_all(&root).unwrap();
    git_as(&root, ("A", "a@example.test"), &["init", "-q", "-b", "main"]);
    let commits = history::BOUND + 1;
    let mut stream = String::new();
    for n in 0..commits {
        // The oldest commit is by B, every other by A.
        let who = if n == 0 { "B <b@example.test>" } else { "A <a@example.test>" };
        let parent = if n == 0 { String::new() } else { format!("from :{n}\n") };
        let body = format!("{n}\n");
        stream.push_str(&format!(
            "commit refs/heads/main\nmark :{}\nauthor {who} {} +0000\ncommitter {who} {} +0000\ndata 2\nc\n{parent}M 100644 inline f.txt\ndata {}\n{body}\n",
            n + 1,
            1_700_000_000 + n,
            1_700_000_000 + n,
            body.len()
        ));
    }
    let mut import = Command::new("git")
        .args(["fast-import", "--quiet"])
        .current_dir(&root)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    import.stdin.take().unwrap().write_all(stream.as_bytes()).unwrap();
    assert!(import.wait().unwrap().success());
    git_as(&root, ("A", "a@example.test"), &["checkout", "-q", "main"]);

    let (view, files) = view(&root, "long");
    let read = history::read(&Git::new(&root), &view.commit, &files, history::BOUND).unwrap();
    assert_eq!(read.commits, history::BOUND, "no more than the bound is read");
    assert_eq!(read.authors.len(), 1, "the one commit past the bound, B's, isn't read");
    let last = git_as(&root, ("A", "a@example.test"), &["rev-list", "--max-count=1", "--skip=4999", "HEAD"]);
    let uncovered = builtin("git").unwrap().uncovered(&view).unwrap();
    assert_eq!(uncovered, [format!("history before {last}")]);
    // Every fact cites a commit the bound covers.
    let batch = history_facts(&view, &files);
    let oldest = git_as(&root, ("A", "a@example.test"), &["rev-list", "--max-parents=0", "HEAD"]);
    for evidence in
        batch.nodes.iter().flat_map(|n| &n.evidence).chain(batch.edges.iter().flat_map(|e| &e.evidence))
    {
        assert!(
            matches!(evidence, Evidence::Ref { commit, ref_name: None } if *commit != oldest),
            "{evidence:?}"
        );
    }
    // At the bound exactly, nothing is uncovered.
    let one_back = git_as(&root, ("A", "a@example.test"), &["rev-parse", "HEAD~1"]);
    assert_eq!(Git::new(&root).history_cut(&one_back, history::BOUND).unwrap(), None);
}

#[test]
fn a_shallow_boundary_is_reported_as_a_cut_history() {
    // Three commits, the oldest by Ada. A shallow file that names the middle one stops git's walk
    // there, as a real shallow clone's does; the history read can't be the commit's whole, so the
    // boundary is reported as uncovered, as the bound's cut is.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    std::fs::create_dir_all(&root).unwrap();
    let ada = ("Ada", "ada@example.test");
    let ben = ("Ben", "ben@example.test");
    git_as(&root, ada, &["init", "-q", "-b", "main"]);
    for (n, who) in [ada, ben, ben].into_iter().enumerate() {
        std::fs::write(root.join(format!("f{n}.txt")), "x\n").unwrap();
        git_as(&root, who, &["add", "-A"]);
        git_as(&root, who, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "Next"]);
    }
    let (repo, files) = view(&root, "app");
    let git_dir = Git::new(&root).absolute_git_dir().unwrap();
    let uncovered = |repo: &RepoView| builtin("git").unwrap().uncovered(repo).unwrap();
    assert!(uncovered(&repo).is_empty(), "a whole history leaves nothing out");

    let middle = git_as(&root, ada, &["rev-parse", "HEAD~1"]);
    // An overlong line that ends in its name comes first: git reads only its first 40 hex digits,
    // so it isn't a boundary.
    let long = format!("{}{middle}", "0".repeat(200));
    std::fs::write(git_dir.join("shallow"), format!("{long}\n{middle}\n")).unwrap();
    assert_eq!(git_as(&root, ada, &["rev-list", "--count", "HEAD"]), "2", "the control: git stops there");
    let read = history::read(&Git::new(&root), &repo.commit, &files, history::BOUND).unwrap();
    assert_eq!(read.authors.len(), 1, "Ada's commit is past the boundary");
    assert_eq!(uncovered(&repo), [format!("history before {middle}")]);

    // A shallow clone at depth 1, as a CI checkout makes: its one commit is the boundary.
    let url = format!("file://{}", root.display());
    git_as(dir.path(), ada, &["clone", "-q", "--depth", "1", &url, "clone"]);
    let clone = dir.path().join("clone");
    let (cloned, _) = view(&clone, "clone");
    assert_eq!(uncovered(&cloned), [format!("history before {}", cloned.commit)]);

    // A shallow file that names only a commit the read never reaches cuts nothing.
    std::fs::write(git_dir.join("shallow"), format!("{}\n", "1".repeat(40))).unwrap();
    assert!(uncovered(&repo).is_empty());
}

/// `f` on a thread of its own, or a panic when it hasn't returned in 20 seconds.
fn within_20s<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(std::time::Duration::from_secs(20)).expect("still blocked after 20 s")
}

/// git as a person runs it, with a fixed author.
fn plain_git(dir: &Path, args: &[&str]) -> String {
    git_as(dir, ("T", "t@example.test"), args)
}

/// A repo at `dir` with one commit of `file`; its commit.
fn committed(dir: &Path, file: &str) -> String {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(file), "x\n").unwrap();
    plain_git(dir, &["init", "-q", "-b", "main"]);
    plain_git(dir, &["add", "."]);
    plain_git(dir, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
    plain_git(dir, &["rev-parse", "HEAD"])
}

/// A FIFO at `place`, in place of whatever was there.
fn fifo(place: &Path) {
    let _ = std::fs::remove_file(place);
    std::fs::create_dir_all(place.parent().unwrap()).unwrap();
    assert!(Command::new("mkfifo").arg(place).output().unwrap().status.success());
}

/// The submodules the adapter lists in the repo at `dir`, as paths.
fn submodules_of(dir: &Path) -> Vec<PathBuf> {
    let dir = dir.to_path_buf();
    within_20s(move || Git::new(&dir).submodules().unwrap())
}

#[test]
fn listing_submodules_runs_no_git_in_them() {
    // A superproject's submodules come from its own index and its `.gitmodules`, read as a file,
    // so no git runs in a submodule before its own owner check. A FIFO at a submodule's HEAD,
    // which git there would block on, never blocks the listing, and git status in the
    // superproject, which reads that HEAD to compare the gitlink, doesn't run.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    committed(&src, "s.txt");
    let sup = dir.path().join("sup");
    committed(&sup, "x.txt");
    let url = src.display().to_string();
    for path in ["sub", "deps/other lib"] {
        plain_git(&sup, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, path]);
    }
    plain_git(&sup, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "Subs"]);
    let want = [PathBuf::from("deps/other lib"), PathBuf::from("sub")];
    assert_eq!(submodules_of(&sup), want);
    assert_eq!(Git::new(&sup).uncommitted_files().unwrap(), 0);

    let head = sup.join(".git/modules/sub/HEAD");
    let kept = dir.path().join("HEAD.kept");
    std::fs::rename(&head, &kept).unwrap();
    fifo(&head);
    assert_eq!(submodules_of(&sup), want, "the listing never opens the submodule's HEAD");
    let s = sup.clone();
    let err = within_20s(move || Git::new(&s).uncommitted_files()).unwrap_err();
    let Some(planroom_extract::git::Refusal::NotAFile(named)) = &err.refusal else { panic!("{err:?}") };
    assert_eq!(named.canonicalize().unwrap(), head.canonicalize().unwrap(), "{err:?}");
    std::fs::remove_file(&head).unwrap();
    std::fs::rename(&kept, &head).unwrap();
    assert_eq!(Git::new(&sup).uncommitted_files().unwrap(), 0, "with the FIFO gone, status runs");
}

#[test]
fn a_submodule_is_a_gitlink_whose_path_gitmodules_names_as_git_reads_it() {
    // `.gitmodules` is read as git reads a config file: sections and keys in any case, quoting,
    // escapes, a continued line and comments. A gitlink no `submodule` section names, a path
    // that names no gitlink, a path in another section and one git ignores (a leading `-`) list
    // nothing.
    let dir = tempfile::tempdir().unwrap();
    let sup = dir.path().join("sup");
    let commit = committed(&sup, "x.txt");
    for path in ["plain", "quoted \"name\"", "continued", "spaced  out", "-dash", "unnamed", "other"] {
        let entry = format!("160000,{commit},{path}");
        plain_git(&sup, &["update-index", "--add", "--cacheinfo", &entry]);
    }
    let text = "\u{feff}# a comment\r\n[submodule \"a\"]\n\tpath = plain\n\
        [Submodule \"b\"]\n\tPATH = \"quoted \\\"name\\\"\" ; a comment\n\
        [submodule \"c\"] path = cont\\\ninued\n\
        [submodule.d]\n\tpath = spaced  out   # a comment\n\
        [submodule \"e\"]\n\tpath = -dash\n\
        [core]\n\tpath = other\n\
        [submodule \"f\"]\n\tpath = missing\n";
    std::fs::write(sup.join(".gitmodules"), text).unwrap();
    let want: Vec<PathBuf> =
        ["continued", "plain", "quoted \"name\"", "spaced  out"].map(PathBuf::from).into();
    assert_eq!(submodules_of(&sup), want);
    // A file git can't parse names no submodule.
    std::fs::write(sup.join(".gitmodules"), "[submodule \"a\"]\n\tpath = \"open\n").unwrap();
    assert!(submodules_of(&sup).is_empty());
}

#[test]
fn the_alternates_file_git_opens_six_deep_is_checked_before_git_runs() {
    // git reads the `info/alternates` of an object directory six alternates deep, and only then
    // ignores what it names, so a FIFO there blocks git: the check refuses the repo before git
    // runs. One level deeper git never looks, and neither does the check.
    let dir = tempfile::tempdir().unwrap();
    let mine = dir.path().join("mine");
    let head = committed(&mine, "m.txt");
    let chain: Vec<PathBuf> = (1..=7).map(|n| dir.path().join(format!("d{n}/objects"))).collect();
    let mut from = mine.join(".git/objects");
    for objects in &chain {
        std::fs::create_dir_all(objects.join("info")).unwrap();
        std::fs::write(from.join("info/alternates"), format!("{}\n", objects.display())).unwrap();
        from = objects.clone();
    }
    let six_deep = chain[5].join("info/alternates");
    fifo(&six_deep);
    let m = mine.clone();
    let err = within_20s(move || Git::new(&m).head()).unwrap_err();
    let Some(planroom_extract::git::Refusal::NotAFile(named)) = &err.refusal else { panic!("{err:?}") };
    assert_eq!(named.canonicalize().unwrap(), six_deep.canonicalize().unwrap(), "{err:?}");

    std::fs::remove_file(&six_deep).unwrap();
    std::fs::write(&six_deep, format!("{}\n", chain[6].display())).unwrap();
    fifo(&chain[6].join("info/alternates"));
    let m = mine.clone();
    assert_eq!(within_20s(move || Git::new(&m).head()).unwrap().commit, head, "seven deep is never read");
}
