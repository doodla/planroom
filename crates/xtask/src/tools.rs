//! The tools a step needs beyond Rust and Node. Locally a missing tool makes its step `skipped`;
//! under `CI=true`, where a skip fails, xtask installs it first, so ci.yml runs only `cargo xtask`
//! (repo-layout.md, Toolchain pins; DR-79).

use std::path::{Path, PathBuf};

use crate::report::under_ci;
use crate::util;

pub const NEXTEST_VERSION: &str = "0.9.149";
pub const DENY_VERSION: &str = "0.20.2";
/// The sha256 of cargo-deny's x86_64-unknown-linux-musl release archive, the pin ci.yml held.
const DENY_LINUX_X86_64_SHA256: &str = "9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Nextest,
    Deny,
    Shellcheck,
    Busybox,
    Dash,
}

impl Tool {
    fn probe(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Tool::Nextest => ("cargo", &["nextest", "--version"]),
            Tool::Deny => ("cargo", &["deny", "--version"]),
            Tool::Shellcheck => ("shellcheck", &["--version"]),
            Tool::Busybox => ("busybox", &["true"]),
            Tool::Dash => ("dash", &["-c", "true"]),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tool::Nextest => "cargo-nextest",
            Tool::Deny => "cargo-deny",
            Tool::Shellcheck => "shellcheck",
            Tool::Busybox => "busybox",
            Tool::Dash => "dash",
        }
    }
}

/// Where a tool was found: on `PATH` as it is, or in a directory to put first on `PATH`.
pub enum Found {
    OnPath,
    In(PathBuf),
}

impl Found {
    /// `PATH` for a child that runs the tool.
    pub fn path_env(&self) -> Option<std::ffi::OsString> {
        match self {
            Found::OnPath => None,
            Found::In(dir) => {
                let mut paths = vec![dir.clone()];
                paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
                std::env::join_paths(paths).ok()
            }
        }
    }
}

/// The tool, installing it under CI when it's missing. `Err` says why it isn't there.
pub fn ensure(root: &Path, tool: Tool) -> Result<Found, String> {
    let (program, args) = tool.probe();
    if util::available(program, args) {
        return Ok(Found::OnPath);
    }
    let local = root.join("target/tools").join(tool.name());
    if tool == Tool::Deny && local.join("cargo-deny").is_file() {
        return Ok(Found::In(local));
    }
    if !under_ci() {
        return Err(format!("{} is not installed", tool.name()));
    }
    install(root, tool, &local)?;
    if tool == Tool::Deny && local.join("cargo-deny").is_file() {
        return Ok(Found::In(local));
    }
    if util::available(program, args) {
        Ok(Found::OnPath)
    } else {
        Err(format!("{} is still missing after its install", tool.name()))
    }
}

fn install(root: &Path, tool: Tool, local: &Path) -> Result<(), String> {
    let linux = cfg!(target_os = "linux");
    match tool {
        Tool::Nextest => cargo_install(root, "cargo-nextest", NEXTEST_VERSION),
        Tool::Deny if linux && cfg!(target_arch = "x86_64") => deny_release(root, local),
        Tool::Deny => cargo_install(root, "cargo-deny", DENY_VERSION),
        Tool::Shellcheck | Tool::Busybox | Tool::Dash if linux => {
            util::capture(root, "sudo", &["apt-get", "update", "--quiet"])?;
            let package = tool.name();
            util::capture(root, "sudo", &["apt-get", "install", "--yes", "--no-install-recommends", package])
                .map(|_| ())
        }
        Tool::Shellcheck | Tool::Dash => util::capture(root, "brew", &["install", tool.name()]).map(|_| ()),
        Tool::Busybox => Err("BusyBox runs on Linux only (DR-79)".into()),
    }
}

fn cargo_install(root: &Path, krate: &str, version: &str) -> Result<(), String> {
    util::capture(root, "cargo", &["install", "--locked", krate, "--version", &format!("={version}")])
        .map(|_| ())
}

/// cargo-deny's sha256-pinned release archive, unpacked into `target/tools/cargo-deny/`.
fn deny_release(root: &Path, dir: &Path) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let name = format!("cargo-deny-{DENY_VERSION}-x86_64-unknown-linux-musl");
    let url =
        format!("https://github.com/EmbarkStudios/cargo-deny/releases/download/{DENY_VERSION}/{name}.tar.gz");
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let archive = dir.join("cargo-deny.tar.gz");
    let archive_str = archive.to_str().ok_or("target/tools isn't UTF-8")?;
    util::capture(root, "curl", &["-sSfL", "-o", archive_str, &url])?;
    let bytes = std::fs::read(&archive).map_err(|e| e.to_string())?;
    let sum: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if sum != DENY_LINUX_X86_64_SHA256 {
        let _ = std::fs::remove_file(&archive);
        return Err(format!("{url}: sha256 {sum}, not the pinned {DENY_LINUX_X86_64_SHA256}"));
    }
    let dir_str = dir.to_str().ok_or("target/tools isn't UTF-8")?;
    util::capture(root, "tar", &["-xzf", archive_str, "-C", dir_str, "--strip-components", "1"]).map(|_| ())
}
