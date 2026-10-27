//! Each toolchain pin has one home, and no other file names a different version
//! (repo-layout.md, Toolchain pins; iris-release-bar §7). Every row of the pins table has its check
//! here, so a new row without one fails, and Dependabot's cooldown is the direct-pin age
//! (AGENTS.md, Dependencies; DR-70).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The version that follows each `prefix` in `text` (digits and dots, trailing dot dropped).
fn versions_after(text: &str, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(prefix) {
        let after = &rest[at + prefix.len()..];
        let boundary = rest[..at].chars().next_back().is_none_or(|c| !c.is_alphanumeric() && c != '-');
        let v: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        let v = v.trim_end_matches('.');
        if boundary && v.starts_with(|c: char| c.is_ascii_digit()) {
            out.push(v.to_owned());
        }
        rest = &rest[at + prefix.len()..];
    }
    out
}

/// The first `"…"` string after `key` in `text`.
fn quoted_after(text: &str, key: &str) -> Option<String> {
    let after = &text[text.find(key)? + key.len()..];
    let open = after.find('"')?;
    let close = after[open + 1..].find('"')?;
    Some(after[open + 1..open + 1 + close].to_owned())
}

/// The tracked files a stated pin may appear in: the docs and agent files people read (records,
/// research notes, the progress log and the fixture's test data say what was true when written),
/// the CI workflows, xtask's sources and the plugin's README.
fn readers() -> Vec<(String, String)> {
    let out = Command::new("git")
        .args(["ls-files", "-z", "*.md", ".github/*", "crates/xtask/src/*.rs"])
        .current_dir(root())
        .output()
        .expect("git runs");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|p| !p.is_empty())
        .filter(|p| {
            !p.starts_with("examples/")
                && !p.contains("/research/")
                && !p.ends_with("decisions.md")
                && !["progress.md", "KICKOFF.md", "CHANGELOG.md"].contains(p)
        })
        .filter_map(|p| Some((p.to_owned(), fs::read_to_string(root().join(p)).ok()?)))
        .collect()
}

/// A stated version agrees with its pin when it is the pin, or the pin's shorter prefix
/// (`Node 22` for 22.22.0).
fn agrees(stated: &str, pin: &str) -> bool {
    stated == pin || pin.starts_with(&format!("{stated}."))
}

/// One pin: the pins table's row it covers, its version at its home, and the words that name it
/// elsewhere.
struct Pin {
    row: &'static str,
    version: String,
    names: &'static [&'static str],
}

fn pins() -> Vec<Pin> {
    let toolchain = read("rust-toolchain.toml");
    let package = read("package.json");
    let tools = read("crates/xtask/src/tools.rs");
    let plugin = read("crates/xtask/src/plugin.rs");
    let main = read("crates/xtask/src/main.rs");
    let table = pins_table();
    // A pin whose home isn't built yet holds the version its row states.
    let stated = |row: &str, name: &str| -> String {
        let cell = table.iter().find(|r| r.contains(row)).unwrap_or_else(|| panic!("no pins row for {row}"));
        versions_after(cell, name).first().cloned().unwrap_or_else(|| panic!("{row}'s row names no version"))
    };
    let python = main
        .split("sys.version_info >= (")
        .nth(1)
        .and_then(|r| r.split(')').next())
        .map(|v| v.replace(", ", "."))
        .expect("xtask's main.rs checks the Python version");
    vec![
        Pin { row: "Rust", version: quoted_after(&toolchain, "channel").unwrap(), names: &["Rust "] },
        Pin { row: "Node", version: read(".node-version").trim().to_owned(), names: &["Node "] },
        Pin {
            row: "pnpm",
            version: quoted_after(&package, "\"packageManager\"")
                .unwrap()
                .trim_start_matches("pnpm@")
                .to_owned(),
            names: &["pnpm "],
        },
        Pin {
            row: "Claude Code",
            version: quoted_after(&plugin, "CLAUDE_CODE_VERSION: &str =").unwrap(),
            names: &["Claude Code "],
        },
        Pin { row: "Python", version: python, names: &["Python "] },
        Pin { row: "git", version: stated("git", "git "), names: &["git "] },
        Pin { row: "cargo-about", version: stated("cargo-about", "cargo-about "), names: &["cargo-about "] },
        Pin {
            row: "cargo-deny",
            version: quoted_after(&tools, "DENY_VERSION: &str =").unwrap(),
            names: &["cargo-deny "],
        },
        Pin {
            row: "cargo-nextest",
            version: quoted_after(&tools, "NEXTEST_VERSION: &str =").unwrap(),
            names: &["cargo-nextest ", "nextest "],
        },
        Pin {
            row: "json-schema-to-typescript",
            version: quoted_after(&read("packages/contract/package.json"), "\"json-schema-to-typescript\"")
                .unwrap(),
            names: &["json-schema-to-typescript "],
        },
        Pin {
            row: "Playwright",
            version: quoted_after(&read("tests/e2e/package.json"), "\"@playwright/test\"").unwrap(),
            names: &["Playwright "],
        },
    ]
}

/// The pin cells of repo-layout.md's Toolchain pins table, header left out.
fn pins_table() -> Vec<String> {
    let text = read("docs/contributing/repo-layout.md");
    let section = &text[text.find("## Toolchain pins").expect("the pins section")..];
    let section = &section[..section[3..].find("\n## ").map_or(section.len(), |i| i + 3)];
    section
        .lines()
        .filter(|l| l.starts_with('|') && !l.starts_with("|---") && !l.starts_with("| Pin |"))
        .map(|l| l.split('|').nth(1).unwrap_or("").trim().to_owned())
        .collect()
}

#[test]
fn every_pin_has_one_version() {
    let pins = pins();
    let readers = readers();
    let mut wrong = Vec::new();
    for pin in &pins {
        for (path, text) in &readers {
            for (n, line) in text.lines().enumerate() {
                // An assumption row records what a spike observed, not a pin.
                if line.starts_with("| A-") {
                    continue;
                }
                for name in pin.names {
                    for stated in versions_after(line, name) {
                        if !agrees(&stated, &pin.version) {
                            wrong.push(format!(
                                "{path}:{}: {name}{stated}, but the pin is {}",
                                n + 1,
                                pin.version
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "a file names a version other than its pin's home:\n{}", wrong.join("\n"));
}

#[test]
fn every_row_of_the_pins_table_is_checked() {
    let checked: Vec<&str> = pins().iter().map(|p| p.row).collect();
    // The rows with checks of their own below.
    let own = ["Dependabot", "Jev and Claude model IDs", "Every crate and npm package"];
    let unchecked: Vec<String> = pins_table()
        .into_iter()
        .filter(|row| {
            !checked
                .iter()
                .chain(own.iter())
                .any(|key| row.starts_with(key) || row.starts_with(&format!("`{key}")))
        })
        .collect();
    assert!(unchecked.is_empty(), "rows of the pins table with no check in release_pins.rs: {unchecked:?}");
}

#[test]
fn the_pnpm_release_age_is_the_tables() {
    let workspace = read("pnpm-workspace.yaml");
    let age = versions_after(&workspace, "minimumReleaseAge: ");
    let row = pins_table().into_iter().find(|r| r.starts_with("pnpm")).expect("the pnpm row");
    assert_eq!(
        age,
        versions_after(&row, "minimumReleaseAge: "),
        "pnpm-workspace.yaml against the pins table"
    );
    // 7 days in minutes, the age for transitive packages (DR-70).
    assert_eq!(age, ["10080"]);
}

#[test]
fn the_vendored_mod_api_is_claude_codes_pinned_version() {
    let pin = pins().into_iter().find(|p| p.row == "Claude Code").unwrap();
    let header = read("packages/mod/vendor/claude-code.d.ts");
    assert_eq!(versions_after(header.lines().next().unwrap_or(""), "Claude Code "), [pin.version]);
}

#[test]
fn model_ids_live_in_the_defaults_file() {
    let defaults = read("crates/planroom/src/config/defaults.toml");
    let ids: Vec<String> = ["model_cheap", "model_capable", "\nmodel"]
        .iter()
        .map(|key| {
            quoted_after(&defaults, &format!("{key} ="))
                .unwrap_or_else(|| panic!("defaults.toml has no {key}"))
        })
        .collect();
    assert!(ids.iter().all(|id| !id.is_empty()), "{ids:?}");
    // A model ID spelled anywhere else in the engine's or the surfaces' sources is one of those.
    let out = Command::new("git")
        .args([
            "grep",
            "-n",
            "-o",
            "-E",
            "(claude-[a-z]+-[0-9][0-9.]*[0-9]|jev-[0-9][0-9.]*[0-9])",
            "--",
            "crates/*/src",
            "packages/*/src",
        ])
        .current_dir(root())
        .output()
        .expect("git runs");
    let mut stray = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let found = line.rsplit(':').next().unwrap_or("");
        if !line.starts_with("crates/planroom/src/config/defaults.toml")
            && !ids.iter().any(|id| id.ends_with(found))
        {
            stray.push(line.to_owned());
        }
    }
    assert!(stray.is_empty(), "model IDs outside defaults.toml: {stray:?}");
}

#[test]
fn every_crate_and_npm_package_is_pinned_exactly_with_its_lockfile() {
    let manifest: toml::Table = toml::from_str(&read("Cargo.toml")).unwrap();
    let deps = manifest["workspace"]["dependencies"].as_table().unwrap();
    for (name, spec) in deps {
        let version = spec.as_str().or_else(|| spec.get("version").and_then(|v| v.as_str())).unwrap_or("");
        assert!(version.starts_with('='), "Cargo.toml's {name} isn't pinned with =x.y.z: {version:?}");
    }
    let out = Command::new("git")
        .args([
            "ls-files",
            "Cargo.lock",
            "pnpm-lock.yaml",
            "package.json",
            "packages/*/package.json",
            "tests/*/package.json",
            "crates/*/Cargo.toml",
        ])
        .current_dir(root())
        .output()
        .expect("git runs");
    let tracked: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_owned).collect();
    for lock in ["Cargo.lock", "pnpm-lock.yaml"] {
        assert!(tracked.iter().any(|t| t == lock), "{lock} isn't committed");
    }
    for krate in tracked.iter().filter(|t| t.starts_with("crates/") && t.ends_with("Cargo.toml")) {
        let doc: toml::Table = toml::from_str(&read(krate)).unwrap();
        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            for (name, spec) in doc.get(kind).and_then(|d| d.as_table()).into_iter().flatten() {
                let workspace = spec.get("workspace").and_then(|w| w.as_bool()) == Some(true);
                let exact = spec
                    .as_str()
                    .or_else(|| spec.get("version").and_then(|v| v.as_str()))
                    .is_some_and(|v| v.starts_with('='));
                assert!(
                    workspace || exact,
                    "{krate}'s {name} is neither from the workspace nor pinned with =x.y.z"
                );
            }
        }
    }
    for package in tracked.iter().filter(|t| t.ends_with("package.json")) {
        let doc: serde_json::Value = serde_json::from_str(&read(package)).unwrap();
        for section in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
            for (name, version) in doc.get(section).and_then(|s| s.as_object()).into_iter().flatten() {
                let version = version.as_str().unwrap_or("");
                let exact = version.split('.').count() == 3
                    && version.split('.').all(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty());
                assert!(
                    exact || version == "workspace:*",
                    "{package}'s {name} isn't an exact version: {version:?}"
                );
            }
        }
    }
}

/// `cooldown`'s `default-days` for each update in `.github/dependabot.yml`, with its ecosystem.
fn cooldowns(yaml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut ecosystem = String::new();
    for line in yaml.lines() {
        let line = line.trim().trim_start_matches("- ");
        if let Some(e) = line.strip_prefix("package-ecosystem:") {
            ecosystem = e.trim().trim_matches('"').to_owned();
        }
        if let Some(days) = line.strip_prefix("default-days:") {
            out.push((ecosystem.clone(), days.trim().to_owned()));
        }
    }
    out
}

#[test]
fn dependabot_cooldown_matches_the_pin_age() {
    // The age AGENTS.md gives a direct pin, and the one `cargo xtask check audit` enforces.
    let agents = read("AGENTS.md");
    let rule = agents.split("to a release at least ").nth(1).expect("AGENTS.md states the direct-pin age");
    let age: String = rule.chars().take_while(char::is_ascii_digit).collect();
    let tools = read("crates/xtask/src/tools.rs");
    assert_eq!(
        versions_after(&tools, "PIN_AGE_DAYS: i64 = "),
        std::slice::from_ref(&age),
        "xtask's pin-age audit"
    );
    let found = cooldowns(&read(".github/dependabot.yml"));
    let mut ecosystems: Vec<&str> = found.iter().map(|(e, _)| e.as_str()).collect();
    ecosystems.sort();
    assert_eq!(
        ecosystems,
        ["cargo", "github-actions", "npm"],
        "Dependabot covers cargo, npm and the actions"
    );
    for (ecosystem, days) in &found {
        assert_eq!(days, &age, "{ecosystem}'s cooldown against the {age}-day pin age");
    }
}
