//! The git CLI adapter (DR-16): every call runs with hooks, fsmonitor and optional locks off, a
//! cleared environment and `LC_ALL=C`, so nothing from a scanned repo runs. Every call is a local
//! read, and no transport may run: a partial clone's missing object is never fetched, so the
//! `uploadpack` or `sshCommand` a repo's own config names never runs either.

use std::collections::BTreeSet;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
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
        // No lazy fetch from a promisor remote. git 2.44 and later read the variable, as do
        // distributions' security backports such as Ubuntu's 2.43; `protocol.allow=never` refuses
        // every transport on any version, before git runs a configured upload-pack or ssh command.
        cmd.env("GIT_NO_LAZY_FETCH", "1");
        cmd.args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "protocol.allow=never",
        ]);
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

    /// Whether this is a partial clone, whose promisor remote may hold objects that aren't here.
    pub fn is_partial_clone(&self) -> Result<bool, GitError> {
        let pattern = r"^(extensions\.partialclone|remote\..*\.promisor)$";
        let out = match self.run(&["config", "--get-regexp", pattern]) {
            Ok(out) => out,
            // `git config --get-regexp` exits 1, silently, when no key matches.
            Err(e) if e.stderr.trim().is_empty() => return Ok(false),
            Err(e) => return Err(e),
        };
        Ok(out.lines().any(|line| match line.split_once(' ') {
            Some((key, value)) if key.starts_with("remote.") => {
                matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "yes" | "on" | "1")
            }
            Some((_, value)) => !value.trim().is_empty(),
            None => false,
        }))
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
