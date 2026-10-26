//! The git CLI adapter (DR-16): every call runs with hooks, fsmonitor and optional locks off, a
//! cleared environment and `LC_ALL=C`, so nothing from a scanned repo runs. Every call is a local
//! read, and no transport may run, each refused by name whatever the repo's own config allows: a
//! partial clone's missing object is never fetched, so the `uploadpack`, `sshCommand` or `ext::`
//! command a repo's own config names never runs either, on any git version.

use std::collections::BTreeSet;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// git exited non-zero, or couldn't start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitError {
    pub argv: Vec<String>,
    pub stderr: String,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "git {} failed: {}", self.argv.join(" "), self.stderr.trim())
    }
}

impl std::error::Error for GitError {}

impl GitError {
    /// git refused the repo because another user owns it (`safe.directory`).
    pub fn is_other_owner(&self) -> bool {
        self.stderr.contains("dubious ownership")
    }
}

/// A repo's checked-out commit and branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub commit: String,
    /// `None` when `HEAD` is detached.
    pub branch: Option<String>,
}

/// One working tree's git.
#[derive(Debug, Clone)]
pub struct Git {
    dir: PathBuf,
}

/// Whether `dir` is the top of a git working tree (a `.git` directory, or the `.git` file of a
/// submodule or linked worktree).
pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Every transport git has: each is refused by name on the command line, since a repo's own
/// `protocol.<name>.allow` beats `protocol.allow` (correctness-r3-3).
pub const TRANSPORTS: &[&str] = &["ext", "fd", "file", "git", "ssh", "http", "https", "ftp", "ftps"];

impl Git {
    pub fn new(dir: &Path) -> Git {
        Git { dir: dir.to_path_buf() }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.env_clear();
        for key in ["PATH", "HOME"] {
            if let Some(value) = std::env::var_os(key) {
                cmd.env(key, value);
            }
        }
        cmd.env("LC_ALL", "C").env("GIT_TERMINAL_PROMPT", "0").env("GIT_LFS_SKIP_SMUDGE", "1");
        // No lazy fetch from a promisor remote. git reads the variable from the 2024 security
        // releases on (2.39.4 and later maintenance lines, 2.45) and in distributions' backports
        // such as Ubuntu's 2.43; an older git ignores it. So no transport may run either:
        // `protocol.allow=never` alone isn't enough, since a repo's own `protocol.<name>.allow`
        // beats it, so each transport is refused by name too, and command-line config beats the
        // repo's for the same key. git then refuses before it runs a configured upload-pack, ssh
        // or ext:: command.
        cmd.env("GIT_NO_LAZY_FETCH", "1");
        #[cfg(test)]
        if tests::AS_OLD_GIT.with(std::cell::Cell::get) {
            cmd.env_remove("GIT_NO_LAZY_FETCH");
        }
        cmd.args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "protocol.allow=never",
        ]);
        for name in TRANSPORTS {
            cmd.arg("-c").arg(format!("protocol.{name}.allow=never"));
        }
        cmd.arg("--no-optional-locks");
        cmd.args(args).current_dir(&self.dir).stdin(Stdio::null());
        cmd
    }

    fn run(&self, args: &[&str]) -> Result<String, GitError> {
        self.run_bytes(args).map(|out| String::from_utf8_lossy(&out).into_owned())
    }

    fn run_bytes(&self, args: &[&str]) -> Result<Vec<u8>, GitError> {
        let fail = |stderr: String| GitError { argv: args.iter().map(|a| a.to_string()).collect(), stderr };
        let out = self.command(args).output().map_err(|e| fail(e.to_string()))?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(fail(String::from_utf8_lossy(&out.stderr).into_owned()))
        }
    }

    /// Every entry of `commit`'s tree, recursively, as `git ls-tree -r -z` prints it:
    /// NUL-terminated `<mode> <type> <object>[ <size>]\t<path>` records, each path as raw bytes.
    /// With `sizes` (`-l`) git reads every blob's header, which a partial clone can't do for the
    /// blobs it doesn't hold, so it lists without them.
    pub fn ls_tree(&self, commit: &str, sizes: bool) -> Result<Vec<u8>, GitError> {
        let tree = format!("{commit}^{{tree}}");
        let mut args = vec!["ls-tree", "-r", "-z"];
        if sizes {
            args.push("-l");
        }
        args.extend(["--full-tree", tree.as_str()]);
        self.run_bytes(&args)
    }

    /// This working tree's git directory, as `git rev-parse --absolute-git-dir` names it: `.git`
    /// itself, or the directory a submodule's or linked worktree's `.git` file points to.
    pub fn absolute_git_dir(&self) -> Result<PathBuf, GitError> {
        let out = self.run_bytes(&["rev-parse", "--absolute-git-dir"])?;
        let line = out.strip_suffix(b"\n").unwrap_or(&out);
        Ok(PathBuf::from(std::ffi::OsStr::from_bytes(line)))
    }

    /// The commit `HEAD` names and its branch.
    pub fn head(&self) -> Result<Head, GitError> {
        let commit = self.run(&["rev-parse", "--verify", "HEAD^{commit}"])?.trim().to_owned();
        let branch = self.run(&["symbolic-ref", "-q", "--short", "HEAD"]).ok().map(|b| b.trim().to_owned());
        Ok(Head { commit, branch: branch.filter(|b| !b.is_empty()) })
    }

    /// The branch `HEAD` names while it has no commit yet, as after `git init`; `None` when `HEAD`
    /// names a commit, or git can't read the repo at all.
    pub fn unborn_branch(&self) -> Option<String> {
        let branch = self.run(&["symbolic-ref", "-q", "HEAD"]).ok()?.trim().to_owned();
        if branch.is_empty() || self.run(&["rev-parse", "-q", "--verify", &branch]).is_ok() {
            return None;
        }
        Some(branch.strip_prefix("refs/heads/").unwrap_or(&branch).to_owned())
    }

    /// Whether this is a partial clone, whose promisor remote may hold objects that aren't here:
    /// `extensions.partialclone` names one, or a `remote.<name>.promisor` is true. git reads the
    /// latter as a bool, where a key with no value and any nonzero integer are true, so git itself
    /// normalises it.
    pub fn is_partial_clone(&self) -> Result<bool, GitError> {
        if !self.config_matching(r"^extensions\.partialclone$", false)?.is_empty() {
            return Ok(true);
        }
        let promisors = self.config_matching(r"^remote\..*\.promisor$", true)?;
        Ok(promisors.lines().any(|line| line.rsplit_once(' ').is_some_and(|(_, value)| value == "true")))
    }

    /// `git config --get-regexp`'s lines for `pattern`, with each value as a bool when `as_bool`;
    /// empty when no key matches.
    fn config_matching(&self, pattern: &str, as_bool: bool) -> Result<String, GitError> {
        let mut args = vec!["config"];
        if as_bool {
            args.push("--type=bool");
        }
        args.extend(["--get-regexp", pattern]);
        let fail = |stderr: String| GitError { argv: args.iter().map(|a| a.to_string()).collect(), stderr };
        let out = self.command(&args).output().map_err(|e| fail(e.to_string()))?;
        match out.status.code() {
            Some(0) => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
            // `git config --get-regexp` exits 1 when no key matches; any other failure, a crash
            // included, is git failing.
            Some(1) => Ok(String::new()),
            _ => Err(fail(format!("{}{}", out.status, String::from_utf8_lossy(&out.stderr)))),
        }
    }

    /// The objects of `commit`'s tree that aren't in this clone, as `rev-list --missing=print`
    /// lists them, which never fetches.
    pub fn missing_objects(&self, commit: &str) -> Result<BTreeSet<String>, GitError> {
        let commit = format!("{commit}^{{commit}}");
        let out = self.run(&["rev-list", "--objects", "--missing=print", "--no-walk", &commit])?;
        Ok(out.lines().filter_map(|l| l.strip_prefix('?')).map(|o| o.trim().to_owned()).collect())
    }

    /// The paths of the initialized submodules, relative to this working tree.
    pub fn submodules(&self) -> Result<Vec<String>, GitError> {
        if !self.dir.join(".gitmodules").is_file() {
            return Ok(Vec::new());
        }
        let out = self.run(&["submodule", "status"])?;
        // Each line is a status character (' ', '+', '-' uninitialized, 'U'), the sha, the path.
        Ok(out
            .lines()
            .filter(|l| !l.starts_with('-'))
            .filter_map(|l| l.get(1..)?.split_whitespace().nth(1).map(str::to_owned))
            .collect())
    }

    /// Every ref name.
    pub fn refs(&self) -> Result<BTreeSet<String>, GitError> {
        Ok(self.run(&["for-each-ref", "--format=%(refname)"])?.lines().map(str::to_owned).collect())
    }

    /// A `cat-file --batch` reader over this repo's objects.
    pub fn objects(&self) -> Result<Objects, GitError> {
        self.batch(true)
    }

    /// A `cat-file --batch-check` reader: each object's size, never its contents.
    pub fn object_sizes(&self) -> Result<Objects, GitError> {
        self.batch(false)
    }

    fn batch(&self, contents: bool) -> Result<Objects, GitError> {
        let mode = if contents { "--batch" } else { "--batch-check" };
        let fail = |e: std::io::Error| GitError {
            argv: vec!["cat-file".into(), mode.into()],
            stderr: e.to_string(),
        };
        let mut child = self
            .command(&["cat-file", mode])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(fail)?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        Ok(Objects { child, stdin: Some(stdin), stdout, contents })
    }
}

/// Reads objects by name through one `git cat-file --batch` (or `--batch-check`) process.
pub struct Objects {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    /// `--batch`, whose replies carry the contents after the header.
    contents: bool,
}

impl Objects {
    /// The object `spec` names, such as `<commit>:<path>` or `<commit>^{commit}`; `None` when it
    /// doesn't exist. A spec holding a newline can't be asked and reads as missing.
    pub fn get(&mut self, spec: &str) -> Result<Option<Vec<u8>>, GitError> {
        assert!(self.contents, "contents come from a --batch reader");
        let Some(size) = self.header(spec)? else { return Ok(None) };
        let size = usize::try_from(size).map_err(|_| self.fail(std::io::Error::other("object too large")))?;
        let mut body = vec![0; size + 1];
        self.stdout.read_exact(&mut body).map_err(|e| self.fail(e))?;
        body.truncate(size);
        Ok(Some(body))
    }

    /// The size of the object `spec` names, from a `--batch-check` reader; `None` when it
    /// doesn't exist.
    pub fn size(&mut self, spec: &str) -> Result<Option<u64>, GitError> {
        assert!(!self.contents, "sizes alone come from a --batch-check reader");
        self.header(spec)
    }

    /// Ask for `spec` and read the reply's header: the object's size, or `None` when it's missing.
    fn header(&mut self, spec: &str) -> Result<Option<u64>, GitError> {
        if spec.contains('\n') {
            return Ok(None);
        }
        let stdin = self.stdin.as_mut().expect("open until drop");
        let sent = writeln!(stdin, "{spec}").and_then(|_| stdin.flush());
        sent.map_err(|e| self.fail(e))?;
        let mut header = String::new();
        self.stdout.read_line(&mut header).map_err(|e| self.fail(e))?;
        let fields: Vec<&str> = header.split_whitespace().collect();
        match fields.as_slice() {
            [_, "missing"] | [_, "ambiguous"] => Ok(None),
            [_, _kind, size] => {
                size.parse().map(Some).map_err(|_| self.fail(std::io::Error::other(header.clone())))
            }
            _ => Err(self.fail(std::io::Error::other(format!("unexpected cat-file reply: {header}")))),
        }
    }

    fn fail(&self, e: std::io::Error) -> GitError {
        let mode = if self.contents { "--batch" } else { "--batch-check" };
        GitError { argv: vec!["cat-file".into(), mode.into()], stderr: e.to_string() }
    }
}

impl Drop for Objects {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    //! The adapter runs nothing a scanned repo names on any git, including one that predates
    //! `GIT_NO_LAZY_FETCH` (unasked-r3-2, correctness-r3-3).

    use std::cell::Cell;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::Git;

    thread_local! {
        /// Run the adapter's git calls without `GIT_NO_LAZY_FETCH`, as a git older than the 2024
        /// security releases runs them: the seam the old-git tests turn on.
        pub(super) static AS_OLD_GIT: Cell<bool> = const { Cell::new(false) };
    }

    /// Plain git, as a person runs it, with a fixed author: the tests' own setup.
    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.test")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.test")
            .env_remove("GIT_NO_LAZY_FETCH")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    /// A blob-less clone holding no blob at all, of a repo with two files: the clone, its commit
    /// and one blob it lacks.
    fn partial_clone(dir: &Path) -> (PathBuf, String, String) {
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("b")).unwrap();
        std::fs::write(src.join("a.txt"), "a\n").unwrap();
        std::fs::write(src.join("b/c.txt"), "c\n").unwrap();
        git(&src, &["init", "-q", "-b", "main"]);
        git(&src, &["add", "."]);
        git(&src, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
        git(&src, &["config", "uploadpack.allowFilter", "true"]);
        let url = format!("file://{}", src.display());
        git(dir, &["clone", "-q", "--filter=blob:none", "--no-checkout", &url, "clone"]);
        let clone = dir.join("clone");
        let commit = git(&clone, &["rev-parse", "HEAD"]);
        let blob = git(&clone, &["rev-parse", "HEAD:a.txt"]);
        (clone, commit, blob)
    }

    /// Every adapter call a scan makes that could want a missing blob, as an old git runs them.
    fn scan_like_an_old_git(clone: &Path, commit: &str, blob: &str) {
        AS_OLD_GIT.with(|old| old.set(true));
        let adapter = Git::new(clone);
        // Each may fail; what matters is that none runs a transport.
        let _ = adapter.run(&["cat-file", "-p", blob]);
        let _ = adapter.ls_tree(commit, true);
        if let Ok(mut objects) = adapter.objects() {
            let _ = objects.get(blob);
        }
        if let Ok(mut sizes) = adapter.object_sizes() {
            let _ = sizes.size(blob);
        }
        let _ = crate::walk::walk(&adapter, commit, &crate::walk::Exclude::none());
        AS_OLD_GIT.with(|old| old.set(false));
    }

    /// The control: plain git with only `-c protocol.allow=never` and no `GIT_NO_LAZY_FETCH`, which
    /// runs a live plant.
    fn plain_old_git(clone: &Path, blob: &str) {
        let _ = Command::new("git")
            .args(["-c", "protocol.allow=never", "cat-file", "-p", blob])
            .current_dir(clone)
            .env_remove("GIT_NO_LAZY_FETCH")
            .output()
            .unwrap();
    }

    #[test]
    fn an_ext_remote_the_repo_allows_never_runs_on_an_old_git() {
        let dir = tempfile::tempdir().unwrap();
        let (clone, commit, blob) = partial_clone(dir.path());
        let mark = dir.path().join("PWNED");
        let url = format!("ext::sh -c touch% {}% >&2", mark.display());
        git(&clone, &["config", "remote.origin.url", &url]);
        git(&clone, &["config", "protocol.ext.allow", "always"]);

        scan_like_an_old_git(&clone, &commit, &blob);
        assert!(!mark.exists(), "the repo's ext:: command ran");
        plain_old_git(&clone, &blob);
        assert!(mark.exists(), "the control: a repo-local protocol.ext.allow beats protocol.allow=never");
    }

    #[test]
    fn a_file_remote_the_repo_allows_never_runs_its_upload_pack_on_an_old_git() {
        let dir = tempfile::tempdir().unwrap();
        let (clone, commit, blob) = partial_clone(dir.path());
        let mark = dir.path().join("ran");
        let planted = format!("sh -c 'echo ran >> {}'; git-upload-pack", mark.display());
        git(&clone, &["config", "remote.origin.uploadpack", &planted]);
        git(&clone, &["config", "protocol.file.allow", "always"]);

        scan_like_an_old_git(&clone, &commit, &blob);
        assert!(!mark.exists(), "the repo's upload-pack command ran");
        plain_old_git(&clone, &blob);
        assert!(mark.exists(), "the control: a repo-local protocol.file.allow beats protocol.allow=never");
    }

    #[test]
    fn the_command_line_refuses_each_transport_whatever_the_repo_config_allows() {
        // Command-line config beats the repo's own for the same key, transport by transport.
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main", "r"]);
        let repo = dir.path().join("r");
        let adapter = Git::new(&repo);
        for name in super::TRANSPORTS {
            let key = format!("protocol.{name}.allow");
            git(&repo, &["config", &key, "always"]);
            let got = adapter.run(&["config", "--get", &key]).unwrap();
            assert_eq!(got.trim(), "never", "{key}");
        }
        git(&repo, &["config", "protocol.allow", "always"]);
        assert_eq!(adapter.run(&["config", "--get", "protocol.allow"]).unwrap().trim(), "never");
    }

    /// A repo whose `.git/config` ends with `extra`.
    fn repo_with_config(dir: &Path, name: &str, extra: &str) -> PathBuf {
        git(dir, &["init", "-q", "-b", "main", name]);
        let repo = dir.join(name);
        let config = repo.join(".git/config");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}{extra}")).unwrap();
        repo
    }

    #[test]
    fn a_promisor_key_reads_as_git_reads_a_bool() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            ("\tpromisor\n", true),
            ("\tpromisor = true\n", true),
            ("\tpromisor = 1\n", true),
            ("\tpromisor = 2\n", true),
            ("\tpromisor = -1\n", true),
            ("\tpromisor = On\n", true),
            ("\tpromisor = false\n", false),
            ("\tpromisor = 0\n", false),
            ("\tpromisor = no\n", false),
            ("", false),
        ];
        for (i, (value, partial)) in cases.into_iter().enumerate() {
            let extra = format!("[remote \"origin\"]\n\turl = /nowhere\n{value}");
            let repo = repo_with_config(dir.path(), &format!("r{i}"), &extra);
            assert_eq!(Git::new(&repo).is_partial_clone().unwrap(), partial, "{value:?}");
        }
        // extensions.partialclone marks one too. With no value, git itself fails on the repo (2.43
        // crashes), which is never read as "not a partial clone".
        let repo = repo_with_config(dir.path(), "e0", "[extensions]\n\tpartialclone = origin\n");
        assert!(Git::new(&repo).is_partial_clone().unwrap());
        let repo = repo_with_config(dir.path(), "e1", "[extensions]\n\tpartialclone\n");
        let got = Git::new(&repo).is_partial_clone();
        assert!(!matches!(got, Ok(false)), "{got:?}");
    }
}
