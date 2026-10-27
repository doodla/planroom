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

/// The pin-age audit: each direct pin's release is at least [`pin_age::PIN_AGE_DAYS`] older than
/// the commit that set it, except a release that fixes an advisory in the tree, which DR-83's list
/// names (AGENTS.md, Dependencies; DR-70, DR-83). `cargo xtask check audit` runs it; the release
/// dates come from crates.io and the npm registry, so it needs the network.
pub mod pin_age {
    use std::path::Path;

    use serde_json::Value;

    use crate::util;

    /// The direct-pin age in days: AGENTS.md's, and Dependabot's cooldown.
    pub const PIN_AGE_DAYS: i64 = 14;

    /// The record whose list names the advisory exceptions.
    pub const EXCEPTIONS_RECORD: u32 = 83;

    /// Where a pin's release date comes from.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Registry {
        Crates,
        Npm,
    }

    /// One direct pin: the package, its exact version, and the manifest that holds it.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Pin {
        pub registry: Registry,
        pub name: String,
        pub version: String,
        pub file: String,
    }

    /// One advisory exception from DR-83's list: `NAME x.y.z for RUSTSEC-…` or `GHSA-…`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Exception {
        pub name: String,
        pub version: String,
        pub advisory: String,
    }

    /// The exceptions DR-83's list names, read from `decisions.md`'s text: every
    /// `NAME x.y.z for RUSTSEC-…` or `NAME x.y.z for GHSA-…` in that record, and nothing else.
    pub fn exceptions(decisions: &str) -> Vec<Exception> {
        let heading = format!("## DR-{EXCEPTIONS_RECORD}:");
        let Some(start) = decisions.find(&heading) else { return Vec::new() };
        let body = &decisions[start + heading.len()..];
        let body = &body[..body.find("\n## ").unwrap_or(body.len())];
        let words: Vec<&str> = body
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| matches!(c, '`' | ',' | '.' | ';' | ':' | '(' | ')' | '*')))
            .collect();
        words
            .windows(4)
            .filter(|w| {
                w[2] == "for"
                    && (w[3].starts_with("RUSTSEC-") || w[3].starts_with("GHSA-"))
                    && semver(w[1])
                    && !w[0].is_empty()
            })
            .map(|w| Exception { name: w[0].into(), version: w[1].into(), advisory: w[3].into() })
            .collect()
    }

    fn semver(v: &str) -> bool {
        let core = v.split(['-', '+']).next().unwrap_or("");
        core.split('.').count() == 3
            && core.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    }

    /// The direct pins `text`, a workspace `Cargo.toml`, names: `[workspace.dependencies]` with an
    /// exact `=x.y.z`, path dependencies left out.
    pub fn cargo_pins(file: &str, text: &str) -> Result<Vec<Pin>, String> {
        let doc: toml::Table = toml::from_str(text).map_err(|e| format!("{file}: {e}"))?;
        let deps = doc.get("workspace").and_then(|w| w.get("dependencies")).and_then(|d| d.as_table());
        let mut out = Vec::new();
        for (name, spec) in deps.into_iter().flatten() {
            if spec.get("path").is_some() {
                continue;
            }
            let version = spec.as_str().or_else(|| spec.get("version").and_then(|v| v.as_str()));
            let Some(version) = version.and_then(|v| v.strip_prefix('=')) else { continue };
            let name = spec.get("package").and_then(|p| p.as_str()).unwrap_or(name);
            out.push(Pin {
                registry: Registry::Crates,
                name: name.into(),
                version: version.into(),
                file: file.into(),
            });
        }
        Ok(out)
    }

    /// The direct pins a `package.json` names: every exact version in its dependency sections,
    /// and `packageManager`'s pnpm; `workspace:` packages left out.
    pub fn npm_pins(file: &str, text: &str) -> Result<Vec<Pin>, String> {
        let doc: Value = serde_json::from_str(text).map_err(|e| format!("{file}: {e}"))?;
        let mut out = Vec::new();
        for section in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
            for (name, version) in doc.get(section).and_then(Value::as_object).into_iter().flatten() {
                let Some(version) = version.as_str().filter(|v| semver(v)) else { continue };
                out.push(Pin {
                    registry: Registry::Npm,
                    name: name.clone(),
                    version: version.into(),
                    file: file.into(),
                });
            }
        }
        let manager = doc.get("packageManager").and_then(Value::as_str).and_then(|p| p.split_once('@'));
        if let Some((name, version)) = manager {
            let version = version.split('+').next().unwrap_or(version);
            out.push(Pin {
                registry: Registry::Npm,
                name: name.into(),
                version: version.into(),
                file: file.into(),
            });
        }
        Ok(out)
    }

    /// Every direct pin in the working tree's manifests.
    pub fn direct_pins(root: &Path) -> Result<Vec<Pin>, String> {
        let read = |f: &str| std::fs::read_to_string(root.join(f)).map_err(|e| format!("{f}: {e}"));
        let mut out = cargo_pins("Cargo.toml", &read("Cargo.toml")?)?;
        let listing = util::capture(
            root,
            "git",
            &["ls-files", "package.json", "packages/*/package.json", "tests/*/package.json"],
        )?;
        for file in listing.lines().filter(|l| !l.is_empty()) {
            out.extend(npm_pins(file, &read(file)?)?);
        }
        Ok(out)
    }

    /// The pin's version in `text`, the manifest as some commit holds it.
    fn version_in(pin: &Pin, text: &str) -> Option<String> {
        let pins = match pin.registry {
            Registry::Crates => cargo_pins(&pin.file, text).ok()?,
            Registry::Npm => npm_pins(&pin.file, text).ok()?,
        };
        pins.into_iter().find(|p| p.name == pin.name).map(|p| p.version)
    }

    /// When the commit that set the pin was made, in Unix seconds: the newest commit whose
    /// manifest holds this version and whose parent's doesn't. A pin not committed yet is set now.
    pub fn set_at(root: &Path, pin: &Pin) -> Result<i64, String> {
        let at = |rev: &str| util::capture(root, "git", &["show", &format!("{rev}:{}", pin.file)]).ok();
        let holds =
            |rev: &str| at(rev).and_then(|t| version_in(pin, &t)).as_deref() == Some(pin.version.as_str());
        if !holds("HEAD") {
            return Ok(now());
        }
        let needle = match pin.registry {
            Registry::Crates => format!("-S\"={}\"", pin.version),
            Registry::Npm => format!("-S{}", pin.version),
        };
        let log = util::capture(root, "git", &["log", "--format=%H %ct", &needle, "--", &pin.file])?;
        for line in log.lines() {
            let Some((sha, time)) = line.split_once(' ') else { continue };
            if holds(sha) && !holds(&format!("{sha}^")) {
                return time.trim().parse().map_err(|e| format!("git log's time {time:?}: {e}"));
            }
        }
        Err(format!("no commit in {}'s history sets {} {}", pin.file, pin.name, pin.version))
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or_default()
    }

    /// The full version `Cargo.lock` resolves a crate pin to: `=1.1.8` pins `1.1.8+spec-1.1.0`,
    /// since a `=` requirement ignores build metadata and crates.io names the release with it.
    pub fn locked_version(lock: &str, name: &str, version: &str) -> Option<String> {
        let mut lines = lock.lines();
        while let Some(line) = lines.next() {
            if line.trim() != format!("name = \"{name}\"") {
                continue;
            }
            let found = lines.next()?.trim().strip_prefix("version = \"")?.strip_suffix('"')?;
            if found.split('+').next() == Some(version) {
                return Some(found.to_owned());
            }
        }
        None
    }

    /// When the pinned release was published, in Unix seconds, from its registry.
    pub fn released_at(root: &Path, pin: &Pin) -> Result<i64, String> {
        let (url, pointer) = match pin.registry {
            Registry::Crates => {
                let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap_or_default();
                let full =
                    locked_version(&lock, &pin.name, &pin.version).unwrap_or_else(|| pin.version.clone());
                (
                    format!("https://crates.io/api/v1/crates/{}/{}", pin.name, full.replace('+', "%2B")),
                    "/version/created_at".to_owned(),
                )
            }
            Registry::Npm => (
                format!("https://registry.npmjs.org/{}", pin.name.replace('/', "%2f")),
                format!("/time/{}", pin.version.replace('~', "~0").replace('/', "~1")),
            ),
        };
        let agent = "planroom-xtask pin-age check";
        let body = util::capture(root, "curl", &["-sSfL", "--max-time", "120", "-A", agent, &url])?;
        let doc: Value = serde_json::from_str(&body).map_err(|e| format!("{url}: {e}"))?;
        let stamp =
            doc.pointer(&pointer).and_then(Value::as_str).ok_or_else(|| format!("{url}: no {pointer}"))?;
        unix_seconds(stamp).ok_or_else(|| format!("{url}: {stamp:?} isn't a UTC timestamp"))
    }

    /// `2026-10-13T20:51:46.498182Z` as Unix seconds.
    pub fn unix_seconds(stamp: &str) -> Option<i64> {
        let (date, time) = stamp.split_once('T')?;
        let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
        let (y, m, day) = (d.next()??, d.next()??, d.next()??);
        let mut t = time.trim_end_matches('Z').split(':').map(|p| p.split('.').next()?.parse::<i64>().ok());
        let (h, min, s) = (t.next()??, t.next()??, t.next()??);
        // Days from the civil date (Howard Hinnant's algorithm), a few lines rather than a date crate.
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        Some(days * 86_400 + h * 3_600 + min * 60 + s)
    }

    /// The failure for a pin set at `set` (Unix seconds) whose release came out at `released`, or
    /// `None` when it is old enough or an exception names it.
    pub fn verdict(pin: &Pin, set: i64, released: i64, exceptions: &[Exception]) -> Option<String> {
        let age_days = (set - released) as f64 / 86_400.0;
        if age_days >= PIN_AGE_DAYS as f64 {
            return None;
        }
        if exceptions.iter().any(|e| e.name == pin.name && e.version == pin.version) {
            return None;
        }
        Some(format!(
            "{} {} in {}: its release was {age_days:.1} days old when a commit pinned it, under the {PIN_AGE_DAYS}-day age, and DR-{EXCEPTIONS_RECORD}'s list names no advisory it fixes",
            pin.name, pin.version, pin.file
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn pin(name: &str, version: &str) -> Pin {
            Pin {
                registry: Registry::Crates,
                name: name.into(),
                version: version.into(),
                file: "Cargo.toml".into(),
            }
        }

        #[test]
        fn dr_83s_list_is_the_only_source_of_exceptions() {
            let text = "## DR-82: Before\n\n- tokio 9.9.9 for RUSTSEC-2099-0001 is not this record's.\n\n\
                        ## DR-83: The gates\n\n- cargo-deny failed on tokio 1.53.2's RUSTSEC-2026-0386, fixed in 1.53.3.\n\
                        - tokio 1.53.3 for RUSTSEC-2026-0386 is the first; `left-pad 1.3.0` for GHSA-aaaa-bbbb-cccc.\n\n\
                        ## DR-84: After\n\n- serde 1.0.0 for RUSTSEC-2099-0002.\n";
            let found = exceptions(text);
            let names: Vec<(&str, &str, &str)> =
                found.iter().map(|e| (e.name.as_str(), e.version.as_str(), e.advisory.as_str())).collect();
            assert_eq!(
                names,
                [("tokio", "1.53.3", "RUSTSEC-2026-0386"), ("left-pad", "1.3.0", "GHSA-aaaa-bbbb-cccc")]
            );
        }

        #[test]
        fn a_pin_younger_than_the_age_fails_unless_an_advisory_names_it() {
            let day = 86_400;
            let released = unix_seconds("2026-10-13T20:51:46.498182Z").unwrap();
            let tokio = pin("tokio", "1.53.3");
            assert!(verdict(&tokio, released + 13 * day, released, &[]).is_some());
            assert!(verdict(&tokio, released + 14 * day, released, &[]).is_none());
            let listed = [Exception {
                name: "tokio".into(),
                version: "1.53.3".into(),
                advisory: "RUSTSEC-2026-0386".into(),
            }];
            assert!(verdict(&tokio, released + 13 * day, released, &listed).is_none());
            // An exception covers that release alone.
            assert!(verdict(&pin("tokio", "1.53.4"), released + day, released, &listed).is_some());
        }

        #[test]
        fn a_utc_timestamp_reads_as_unix_seconds() {
            assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
            assert_eq!(unix_seconds("2026-08-13T00:39:55.466Z"), Some(1_786_581_595));
            assert_eq!(unix_seconds("2000-03-01T00:00:00Z"), Some(951_868_800));
        }

        #[test]
        fn the_pins_are_the_exact_registry_versions() {
            let cargo = "[workspace.dependencies]\nplanroom-kernel = { path = \"k\", version = \"=0.1.0-dev\" }\n\
                         toml = \"=1.1.8\"\nclap = { version = \"=4.6.7\", features = [\"derive\"] }\nloose = \"1\"\n";
            let found: Vec<String> = cargo_pins("Cargo.toml", cargo)
                .unwrap()
                .iter()
                .map(|p| format!("{} {}", p.name, p.version))
                .collect();
            assert_eq!(found, ["clap 4.6.7", "toml 1.1.8"]);
            let npm = r#"{"dependencies":{"@planroom/ui":"workspace:*","react":"19.3.0"},"devDependencies":{"vite":"8.3.4"},"packageManager":"pnpm@10.28.2"}"#;
            let found: Vec<String> = npm_pins("package.json", npm)
                .unwrap()
                .iter()
                .map(|p| format!("{} {}", p.name, p.version))
                .collect();
            assert_eq!(found, ["react 19.3.0", "vite 8.3.4", "pnpm 10.28.2"]);
        }
    }
}
