//! The git CLI adapter (DR-16): every call runs with hooks, fsmonitor and optional locks off, a
//! cleared environment and `LC_ALL=C`, so nothing from a scanned repo runs.

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
        cmd.args(["-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false", "--no-optional-locks"]);
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

    /// Every entry of `commit`'s tree, recursively, as `git ls-tree -r -z -l` prints it:
    /// NUL-terminated `<mode> <type> <object> <size>\t<path>` records, each path as raw bytes.
    pub fn ls_tree(&self, commit: &str) -> Result<Vec<u8>, GitError> {
        self.run_bytes(&["ls-tree", "-r", "-z", "-l", "--full-tree", &format!("{commit}^{{tree}}")])
    }

    /// The commit `HEAD` names and its branch.
    pub fn head(&self) -> Result<Head, GitError> {
        let commit = self.run(&["rev-parse", "--verify", "HEAD^{commit}"])?.trim().to_owned();
        let branch = self.run(&["symbolic-ref", "-q", "--short", "HEAD"]).ok().map(|b| b.trim().to_owned());
        Ok(Head { commit, branch: branch.filter(|b| !b.is_empty()) })
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
        let fail = |e: std::io::Error| GitError {
            argv: vec!["cat-file".into(), "--batch".into()],
            stderr: e.to_string(),
        };
        let mut child = self
            .command(&["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(fail)?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        Ok(Objects { child, stdin: Some(stdin), stdout })
    }
}

/// Reads objects by name through one `git cat-file --batch` process.
pub struct Objects {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Objects {
    /// The object `spec` names, such as `<commit>:<path>` or `<commit>^{commit}`; `None` when it
    /// doesn't exist. A spec holding a newline can't be asked and reads as missing.
    pub fn get(&mut self, spec: &str) -> Result<Option<Vec<u8>>, GitError> {
        if spec.contains('\n') {
            return Ok(None);
        }
        let fail = |e: std::io::Error| GitError {
            argv: vec!["cat-file".into(), "--batch".into()],
            stderr: e.to_string(),
        };
        let stdin = self.stdin.as_mut().expect("open until drop");
        writeln!(stdin, "{spec}").and_then(|_| stdin.flush()).map_err(fail)?;
        let mut header = String::new();
        self.stdout.read_line(&mut header).map_err(fail)?;
        let fields: Vec<&str> = header.split_whitespace().collect();
        match fields.as_slice() {
            [_, "missing"] | [_, "ambiguous"] => Ok(None),
            [_, _kind, size] => {
                let size: usize = size.parse().map_err(|_| fail(std::io::Error::other(header.clone())))?;
                let mut body = vec![0; size + 1];
                self.stdout.read_exact(&mut body).map_err(fail)?;
                body.truncate(size);
                Ok(Some(body))
            }
            _ => Err(fail(std::io::Error::other(format!("unexpected cat-file reply: {header}")))),
        }
    }
}

impl Drop for Objects {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}
