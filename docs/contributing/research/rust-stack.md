# Research: Rust stack

Phase 1 spike for the engine's Rust dependencies. Date: 2026-10-25. Toolchain: `rustc 1.97.0 (2d8144b78 2026-07-07)`, `cargo 1.97.0`, Node `v22.22.0`.

All throwaway work lives in `/tmp/claude-0/-home-user-planroom/d26fcf29-d6d2-57f8-acca-1138f06b3754/scratchpad/rust-stack/` (called `$S` below). The workspace is `$S/ws` (members `stack`, `gixprobe`, `git2probe`, `tsperf`, `codegen`, `mcpprobe`). `sqlx`, `typify`, the TLS comparison and the minimal `rusqlite` probe are separate one-crate workspaces under `$S`, because they conflict with or would skew the main graph.

**Evidence labels.** Every claim is marked:

- **[V]** verified here: a command was run, and its output or the `file:line` is cited.
- **[D]** read in docs, a changelog or source, but not exercised.
- **[A]** assumed.

**Caveat on timings.** The machine is a 4-vCPU, 15 GB Firecracker VM. Other research was running npm and Vite work at the same time (load average about 3 during the builds), so treat every timing as an upper bound for a quiet 4-core CI runner. **[V]** `uptime` showed `load average: 2.22, 3.23, 2.89`.

## 1. Recommendations

| Decision | Recommendation | Why |
|---|---|---|
| Parsing | `tree-sitter =0.27.1` with the five official grammar crates | All six languages load on one runtime through `tree-sitter-language`, with no ABI conflict. Speed is 5 to 30 times the target. |
| Store | `rusqlite =0.41.0` (`bundled`), not `sqlx` | The store is single-writer, local and synchronous. rusqlite builds in 12 s, against 48 s for sqlx. **The two cannot share one workspace:** see Gotcha G1. |
| Git | `gix =0.90.0` (`default-features = false`, `basic`, `sha1`, `max-performance`) for the history extractor and the fixture builder. `git2 =0.21.0` (vendored) stays as the fallback. | gix is pure Rust and produced identical fixtures. Its history walk is as fast as git2's, and every release ships a new 0.x. Pin it exactly and keep it inside one crate. |
| HTTP | `axum =0.8.9` + `tokio =1.54.1` + `tower-http =0.6.11` (`fs`, `trace`, `cors`). Use SSE for the live stream; `ws` is optional. | SSE, WebSocket upgrade and static files were all verified. With tower-http 0.6.11, reqwest and axum share one copy instead of two. |
| Contracts | **Rust types are the source.** `schemars =1.2.3` (with `preserve_order`) generates committed JSON Schema 2020-12, and `json-schema-to-typescript@16.0.0` generates TypeScript from those files. A drift test compares both outputs with what is committed. | JSON Schema is needed anyway: rmcp tool schemas are schemars 1, and Artifact rows and pack files need validation. ts-rs and specta produce only TypeScript, and specta is still a release candidate. |
| MCP | `rmcp =3.7.0` (`server`, `transport-io`, `macros`, no default features), if the architecture needs an MCP server | A stdio server compiles, and initialize, `tools/list` and `tools/call` work, including the error path. The crate moves fast: 3 majors in 5 months. |
| HTTP client | `reqwest =0.13.5` with `rustls` (aws-lc-rs). Switch to `rustls-no-provider` + `rustls/ring` if build time matters. | No OpenSSL anywhere (`cargo tree -i openssl-sys` finds nothing). The ring build is 22 s faster and 2.1 MB smaller. |
| IDs | `uuid =1.29.0` with `v7` | Sortable, standard and on schemars' feature list. Choose `ulid =3.0.0` only if 26-character strings are wanted; note that its 3.0 API renamed `Ulid::new()` to `Ulid::generate()`. |
| CI tools | Download prebuilt `cargo-deny 0.20.2` and `cargo-nextest 0.9.149` release binaries and check them against a pinned sha256. Do not `cargo install` them. | Download takes 0.6 s, against 2 min 30 s for `cargo install --locked cargo-deny`. |
| Dev profile | `[profile.dev.package."*"] opt-level = 2` | Debug tree-sitter parsing is 3.5 times slower without it, and this one setting brings it back to near release speed. |

## 2. Crate table

Versions, licenses and dates come from the crates.io API (`$S/j_<crate>.json`, fetched 2026-10-25). Downloads are lifetime totals. Every crate in the "Engine" group compiled together in `$S/ws/stack` and ran `stack selftest` **[V]**.

### Engine

| Crate | Exact version | License | Released | Why | Alternatives rejected |
|---|---|---|---|---|---|
| tree-sitter | 0.27.1 | MIT | 2026-10-08 | Incremental parsing and queries for every language through one API | `syn` (Rust only); `swc`/`oxc` (JS/TS only); a language server per language (tier 3, optional) |
| tree-sitter-language | 0.1.9 | MIT | 2026-10-08 | The ABI bridge. Grammar crates depend only on it (`^0.1`), never on `tree-sitter` itself, which is a dev-dependency **[V]** (crates.io deps API) | none; it is the official bridge |
| tree-sitter-typescript | 0.23.2 | MIT | 2024-11-11 | TS and TSX grammars. **ABI 14**, older but still loads | none official |
| tree-sitter-javascript | 0.25.0 | MIT | 2025-09-01 | JS grammar, ABI 15 | |
| tree-sitter-python | 0.25.0 | MIT | 2025-09-11 | ABI 15 | |
| tree-sitter-go | 0.25.0 | MIT | 2025-08-29 | ABI 15 | |
| tree-sitter-rust | 0.24.2 | MIT | 2026-03-27 | ABI 15 | |
| petgraph | 0.8.3 | MIT OR Apache-2.0 | 2025-09-30 | Graph algorithms (toposort, SCC, paths) for the kernel's derived views. 573M downloads; a mature, slow-moving API | hand-rolled adjacency lists (the brief says not to hand-roll graphs) |
| rusqlite | 0.41.0 | MIT | 2026-10-24 | Synchronous SQLite with `bundled` (SQLite 3.54.1, JSON1 and WAL verified) | sqlx 0.9.0 (async, which a single writer doesn't need; 4 times slower to build; compile-time-checked macros need `DATABASE_URL` or offline data; conflicts with rusqlite) |
| axum | 0.8.9 | MIT | 2026-04-14 | Local HTTP API, SSE and the optional `ws` feature | actix-web (its own runtime), warp (less maintained) |
| tokio | 1.54.1 | MIT | 2026-10-21 | Runtime; also needed by axum, reqwest and rmcp | smol (the ecosystem assumes tokio) |
| tokio-stream | 0.1.19 | MIT | 2026-07-22 | Turns broadcast channels into SSE streams | |
| tower-http | **0.6.11** (0.7.1 is latest) | MIT | 0.7.1: 2026-08-31 | `ServeDir` for the canvas assets, plus `TraceLayer`. 0.6.11 matches what reqwest 0.13.5 pulls in, so only one copy is built **[V]** (`cargo tree -d` shows no tower-http duplicate) | rust-embed 8.13.0 (only if assets must be compiled into the binary) |
| serde | 1.0.229 | MIT OR Apache-2.0 | 2026-07-18 | | |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | 2026-10-11 | | simd-json (not needed) |
| schemars | 1.2.3 | MIT | 2026-10-12 | Derives JSON Schema 2020-12 that follows serde attributes. rmcp 3.7.0 depends on `schemars ^1.0`, so one derive serves MCP tools too **[V]** (`rmcp-3.7.0/Cargo.toml`) | schemars 0.8 (draft-07; typify still uses it internally) |
| clap | 4.6.7 | MIT OR Apache-2.0 | 2026-09-14 | `derive`; a global `--json` flag verified in `stack` | argh, bpaf (smaller ecosystems) |
| gix | 0.90.0 | MIT OR Apache-2.0 | 2026-10-22 | History walk with per-file line stats; writing commits, annotated and lightweight tags, and branches (section 6) | git2 0.21.0 (C libgit2; works and builds faster, but brings C and libgit2 CVEs into the graph); shelling out to `git` (fastest, but needs git installed and output parsing) |
| notify | 8.2.1 | **CC0-1.0** | 2026-10-25 | Watching the workspace; an inotify `Create(File)` event was received **[V]** | watchexec (too much for this) |
| notify-debouncer-full | 0.7.0 | MIT OR Apache-2.0 | 2026-01-23 | Debounces bursts of editor saves **[D]** | hand-written debounce |
| similar | 3.2.1 | Apache-2.0 | 2026-10-24 | Text diffs for the `.planroom/` export and plan diffs (`TextDiff::ratio` verified) | `diff` / `dissimilar` (fewer algorithms) |
| reqwest | 0.13.5 | MIT OR Apache-2.0 | 2026-09-08 | Jev (Vercel AI Gateway) and Claude HTTP calls. `rustls` is the default TLS in 0.13 | hyper directly (too low level); ureq (blocking) |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | 2026-10-14 | TLS with no OpenSSL. `rustls-platform-verifier` trusted this container's proxy CA, so an HTTPS GET returned `200 OK` **[V]** | native-tls/OpenSSL (the brief says no OpenSSL) |
| tracing | 0.1.44 | MIT | 2025-12-18 | | log |
| tracing-subscriber | 0.3.23 | MIT | 2026-03-13 | `env-filter` and `json`; write logs to stderr so `--json` stdout stays clean | |
| rmcp | 3.7.0 | Apache-2.0 | 2026-10-22 | The official MCP Rust SDK (repo `modelcontextprotocol/rust-sdk`) | hand-rolled JSON-RPC |
| ignore | 0.4.33 | Unlicense OR MIT | 2026-08-04 | Directory walking that respects `.gitignore` (from ripgrep) | walkdir plus a hand-written ignore parser |
| globset | 0.4.20 | Unlicense OR MIT | 2026-08-04 | Pack and extractor globs | glob, wax |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 | 2026-10-09 | Pack and config files | toml_edit (only needed to rewrite files in place while keeping comments) |
| sha2 | 0.11.1 | MIT OR Apache-2.0 | 2026-10-12 | Decision-cache keys (hash of state and question). Note: gix and axum still pull in `digest 0.10`, so two digest stacks are built | blake3 (faster, but adds C or SIMD build work) |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | 2026-09-23 | Typed errors with stable codes in library crates | snafu |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | 2026-07-18 | Only in binaries and tests | eyre |
| uuid | 1.29.0 | Apache-2.0 OR MIT | 2026-10-23 | `v7` time-ordered IDs for events and idempotency keys | ulid 3.0.0 (fine; renamed `new()` to `generate()` **[V]**, compile error) |

### Test and tooling

| Crate or tool | Exact version | License | Released | Why | Alternatives rejected |
|---|---|---|---|---|---|
| insta | 1.49.0 | Apache-2.0 | 2026-10-03 | Snapshot tests for `--json` output and schemas (`json` feature; an inline snapshot passed) | expect-test |
| assert_cmd | 2.2.2 | MIT OR Apache-2.0 | 2026-05-11 | CLI end-to-end tests (`cargo_bin("stack")` test passed) | trycmd (could be added for doc-style CLI cases) |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | 2026-03-11 | | |
| jsonschema | 0.60.1 | MIT | 2026-10-23 | Validating fixtures and imported Artifact rows against the committed schema at runtime and in tests **[D]** (not built here) | boon |
| cargo-deny | 0.20.2 | MIT OR Apache-2.0 | 2026-07-09 | Licenses, bans, advisories, sources | cargo-audit (advisories only) |
| cargo-nextest | 0.9.149 | Apache-2.0 OR MIT | 2026-10-20 | Optional faster test runner | |
| json-schema-to-typescript (npm) | 16.0.0 | MIT | 2026-08-28 | JSON Schema to TypeScript | quicktype (heavier, different output style) |

### Releases younger than 14 days (supply-chain cool-off)

These are the latest stable versions, but each was published within the last 14 days. They are listed in case the project adopts a 14-day cool-off rule. Each shows the newest version at least 14 days old **[V]** (crates.io versions API):

| Crate | Latest (date) | Newest at least 14 days old |
|---|---|---|
| notify | 8.2.1 (2026-10-25) | 8.2.0 (2025-08-03) |
| rusqlite | 0.41.0 (2026-10-24) | 0.40.2 (2026-08-08) |
| similar | 3.2.1 (2026-10-24) | 3.2.0 |
| uuid | 1.29.0 (2026-10-23) | 1.28.0 |
| gix | 0.90.0 (2026-10-22) | 0.89.0 |
| rmcp | 3.7.0 (2026-10-22) | 3.5.1 |
| tokio | 1.54.1 (2026-10-21) | 1.53.2 |
| jsonschema | 0.60.1 (2026-10-23) | 0.58.6 |
| rustls | 0.23.46 (2026-10-14) | 0.23.45 |
| schemars | 1.2.3 (2026-10-12) | 1.2.2 |
| sha2 | 0.11.1 (2026-10-12) | 0.11.0 |

All builds here used the "latest" column. The older versions were not built.

MSRV: the highest declared `rust-version` among the chosen crates is 1.90 (tree-sitter and tree-sitter-language), so Rust 1.97 has room. sqlx 0.9.0 declares 1.94 **[V]** (crates.io data).

## 3. Build time and size (the whole engine stack in one binary)

`$S/ws/stack` links every engine crate above: an MCP stdio server, an axum server with SSE, WebSocket and static files, tree-sitter with six grammars, gix, rusqlite, reqwest with rustls, and the rest. In total 413 unique packages (`cargo tree -e normal`) and 416 build units.

| Measurement | Result **[V]** |
|---|---|
| Clean debug build (`cargo build -p stack`) | **1 min 40 s** wall, 4 min 32 s user |
| Clean release build (`--release`, `strip = "symbols"`) | **4 min 05 s** wall, 11 min 8 s user |
| Clean `dist` build (release + `lto = "fat"`, `codegen-units = 1`) | 4 min 18 s |
| Release binary (stripped) | **21.7 MB**; 7.2 MB with `gzip -9` |
| `dist` binary | 19.6 MB; 6.6 MB with `gzip -9` |
| Debug binary | 261 MB (debuginfo); the debug target dir is 1.9 GB |
| Incremental debug rebuild after touching `main.rs` | 3.6 s |

Slowest units in release, from `cargo build --timings` (`$S/t-rel/cargo-timings/`): `aws-lc-sys` build script 89 s, `libsqlite3-sys` build script 70 s, `rmcp` 36 s, `gix` 31 s, `rustls` 27 s, `regex-automata` 27 s, `tokio` 25 s, `similar` 24 s, `reqwest` 22 s. In debug: `aws-lc-sys` 37 s, `rmcp` 18 s, `gix` 13 s, `reqwest` 10 s. The tree-sitter grammar build scripts take 0.4 to 2.8 s each (TypeScript is the largest).

Per-component clean builds, each with its own target dir **[V]**:

| Probe | Debug | Release | Release binary |
|---|---|---|---|
| gixprobe (gix, history walk and writes) | 26.9 s | 53.2 s | 4.46 MB |
| git2probe (git2, vendored libgit2, no https or ssh) | 17.6 s | 37.1 s | 1.46 MB |
| rusqlite bundled, minimal | 12 s | — | — |
| sqlx 0.9 sqlite + tokio + macros + migrate | 48 s (155 packages) | — | — |
| mcpprobe (rmcp server, stdio, macros) | 30 s | — | — |
| reqwest + rustls, aws-lc-rs provider | — | 59 s | 8.10 MB |
| reqwest + rustls, ring provider | — | 37 s | 5.97 MB |

What the C build scripts need: the `cc` crate and a C compiler, for tree-sitter, SQLite and aws-lc. `aws-lc-sys` may also use `cmake`; it was present here (`/usr/bin/cmake`, `clang`, `nasm`). **[A]** Stock GitHub-hosted Ubuntu, macOS and Windows runners have these. With the `ring` provider, only `cc` is needed.

## 4. tree-sitter: ABI and performance

### ABI compatibility **[V]**

`stack selftest` calls `Language::abi_version()` and `Parser::set_language()` for each grammar:

```
grammar ts: abi=14 set_language_ok=true
grammar tsx: abi=14 set_language_ok=true
grammar js: abi=15 set_language_ok=true
grammar py: abi=15 set_language_ok=true
grammar go: abi=15 set_language_ok=true
grammar rs: abi=15 set_language_ok=true
tree-sitter LANGUAGE_VERSION=15 MIN_COMPATIBLE=13
```

- **`tree-sitter-language` is the bridge.** Each grammar crate's only normal dependency is `tree-sitter-language ^0.1` (plus `cc` to build). `tree-sitter` itself is just a dev-dependency (0.24 for TS, 0.25 for the others), so the grammar crates never force a runtime version. **[V]** (crates.io `/dependencies` API for each grammar version)
- **tree-sitter-typescript 0.23.2 is the laggard.** It was last released 2024-11-11 and emits ABI 14, while the others emit 15. The 0.27 runtime accepts ABI 13 to 15, so it works today. If a future runtime raises `MIN_COMPATIBLE_LANGUAGE_VERSION` above 14, TypeScript breaks first. Guard this with a test that loads every grammar and asserts `abi_version() >= MIN_COMPATIBLE_LANGUAGE_VERSION`.
- **API change in 0.27.** `QueryMatch::captures` is now a method, not a field (compile error E0616 in `tsperf`). Matches are a `StreamingIterator`, re-exported as `tree_sitter::StreamingIterator` (`tree-sitter-0.27.1/binding_rust/lib.rs:33`).

### Performance **[V]**

The corpus is real code, sampled at random from local sources by `$S/scripts/corpus.py`:

- TypeScript from `/opt/node-tools/node_modules` (not `.d.ts`);
- Python 3.12 stdlib;
- Go 1.25.1 stdlib (not `_test.go`);
- Rust from `~/.cargo/registry`.

In total **218 files and 46,166 lines (1.52 MB)**: TS 90 files and 11.2k lines, Python 38 and 13.0k, Go 45 and 11.9k, Rust 45 and 10.0k.

`$S/ws/tsperf` (release build) walks the corpus with `ignore`, parses each file and runs the import queries. The queries cover:

- TS/JS: `import`, `export … from`, dynamic `import()` and `require()`;
- Python: `import`, `import as` and `from … import`;
- Go: `import_spec`;
- Rust: `use`, `mod` and `extern crate`.

The runs found 1,093 imports in total.

| Run | Parse + query | Files/s | Lines/s |
|---|---|---|---|
| 1 thread, 4 runs | 315 to 385 ms | 566 to 693 | 120k to 147k |
| 2 threads | 254 ms | 858 | 182k |
| 4 threads (round-robin split), 3 runs | 227 to 306 ms | 713 to 961 | 151k to 203k |
| 5× corpus (1,090 files, 231k lines), 1 thread | 1,575 ms | 692 | 147k |
| 5× corpus, 4 threads | 502 ms | 2,171 | 460k |
| Debug build, 1 thread | 1,123 ms | 194 | 41k |
| Debug build with `[profile.dev.package."*"] opt-level = 2` | 366 ms | 596 | 126k |

- Creating the six languages and compiling the six queries takes 41 to 60 ms in release (156 ms in debug). Do it once per thread, not per file.
- The directory walk takes about 1 ms for 218 files.
- **Cold scan of a small polyglot fixture:** 32 files and 6.4k lines, run after `echo 3 > /proc/sys/vm/drop_caches`. The whole process took **162 ms** of wall time, against a 5 s target. Met with a margin of about 30 times.
- Parallel scaling on the small corpus is poor (about 1.4×), because setup cost dominates and other research was loading the CPUs. On the 5× corpus it reaches 3.1×. Use rayon or a work queue only once a scan exceeds a few hundred files.
- Four Rust files (`web-sys` generated bindings) parsed with `has_error()`, which tree-sitter-rust 0.24.2 does not fully handle. Parsing still recovers, so extractors must tolerate error nodes and record the file as "partially parsed" evidence instead of failing.

## 5. SQLite: rusqlite vs sqlx

- **rusqlite 0.41.0 `bundled` [V]:** SQLite 3.54.1. `PRAGMA journal_mode=WAL` returned `wal`, and `json_extract` works (JSON1 is built in). The minimal clean build is 12 s.
- **sqlx 0.9.0 [V]:** builds in 48 s with `sqlite`, `runtime-tokio`, `migrate`, `json` and `macros`, and bundles SQLite 3.51.3 (`libsqlite3-sys 0.37.0`). Its compile-time-checked `query!` macros need `DATABASE_URL` or committed `.sqlx` offline data **[D]**.
- **Decision: rusqlite.**
  - Planroom has one writer per workspace. That fits one owned `Connection` on a dedicated thread, with commands sent over a channel. A readers pool is optional: `r2d2 0.8.10` was last released in 2022, so prefer a few plain `Connection`s opened read-only.
  - An async driver adds nothing here and costs build time.
  - `rusqlite_migration 2.6.0` (Apache-2.0) is available if hand-written `user_version` migrations get messy. Its MSRV is 1.95, which is fine.

## 6. git: gix vs git2

Both probes did the same job **[V]**:

- Build a 3-commit fixture: write blobs and trees, commit with fixed author and committer times, create branch `feature`, annotated tag `v0.1.0` and lightweight tag `light`.
- Walk history from HEAD, diffing each commit against its first parent, with per-file `+added -removed`.

Results:

- **Both produced the same HEAD commit, `98edf31f049b…`.** That confirms deterministic fixtures. `git fsck` was clean, and `git log --numstat` matched both tools' per-file stats line for line.
- **Walk speed on a real repo** (bare `tokio-rs/axum` clone, 2,023 commits):

  | Tool | Time | File changes | Lines |
  |---|---|---|---|
  | gix | 1.67 s, 1.76 s | 8,805 | +170,922 −102,577 |
  | git2 | 2.10 s, 1.83 s | 8,811 | +170,925 −102,580 |
  | `git log --numstat` | 0.90 s | 8,737 | +170,651 −102,457 |

  The small differences come from merge commits (git skips them by default, and the probes diff them against the first parent) and from binary files. Rename tracking was off (`track_rewrites(None)`).
- **Builds:** git2 builds faster (17.6 s debug, against 26.9 s for gix) and makes a smaller binary. Both build cleanly with no OpenSSL. Build git2 with `default-features = false, features = ["vendored-libgit2"]`; its `https` and `ssh` features pull in OpenSSL and libssh2.
- **gix API used (0.90.0):**
  - `gix::init`, `write_blob`, `edit_tree(...).upsert(...).write()`, `commit_as(committer, author, "HEAD", msg, tree, parents)`;
  - `reference("refs/heads/feature", id, PreviousValue::MustNotExist, msg)`, `tag(...)`, `tag_reference(...)`;
  - `rev_walk([head]).all()`, `tree.changes()?.for_each_to_obtain_tree(..)` with `change.diff(&mut cache)?.line_counts()`.
  - Source: `gix-0.90.0/src/repository/object.rs:310,334`, `repository/reference.rs:16,69`, `object/tree/diff/mod.rs:177`, `object/blob.rs:112`.
- **Gotchas:**
  - Writing trees directly with gix leaves no index or worktree. `git status` in the fixture then shows every file as `D`. The fixture builder must also write files to disk and either check out (gix `worktree-mutation` feature, not tried) or build the index. git2 does this naturally (`index.add_all` then `write_tree`).
  - **Default branch differs:** gix `init` makes `main`, and libgit2 makes `master`, with no `init.defaultBranch` set. Fixtures must set HEAD explicitly.
  - gix `commit()` fails without configured user identity. Use `commit_as` with explicit signatures; it is deterministic too.
- **Decision: gix**, pinned `=0.90.0` and confined to `planroom-extract-git`.
  - It is pure Rust, as fast as git2 for history, and active (16 releases in 12 months).
  - The cost is churn: a new 0.x every 2 to 4 weeks (0.87 on 08-22, 0.88 on 09-25, 0.89 on 10-08, 0.90 on 10-22). Expect small API changes on every bump, so put all calls behind one module.
  - If gix churn bites, git2 0.21.0 is a verified drop-in for both jobs.

## 7. HTTP server, SSE, WebSocket, static files **[V]**

The `stack serve` binary runs axum 0.8.9 with `ServeDir` and `TraceLayer`:

- `GET /events` streamed `event: tick` / `data: 0..2`.
- `GET /Cargo.toml` returned `200 text/x-toml` from `ServeDir`.
- A WebSocket upgrade returned `HTTP/1.1 101 Switching Protocols`.

Recommendation: SSE for the scoped live stream, because it is one-way, works through proxies, and `EventSource` reconnects with `Last-Event-ID`. Commands go over plain HTTP POST with optimistic-concurrency versions. Keep the `ws` feature off unless a bidirectional need appears; it adds `tokio-tungstenite`, `sha1` and `digest 0.10`.

## 8. MCP: rmcp **[V]**

- **Version and pedigree:** rmcp 3.7.0 (2026-10-22), Apache-2.0, repo `github.com/modelcontextprotocol/rust-sdk` (the official SDK). 40M downloads, 40 releases in 12 months.
- **Majors are frequent:** 1.0 (2026-03-03), 2.0 (2026-06-29), 3.0 (2026-07-28). In the changelog, `[breaking]` entries appear only in 3.0.0 betas, not in 3.x point releases (`awk` over `rmcp-3.7.0/CHANGELOG.md`). Maturity: usable and semver-respecting, but expect a major about every quarter.
- **Protocol versions:** it supports 2024-11-05 to 2026-07-28 (`rmcp-3.7.0/src/model.rs:167-174`).
- **Minimal stdio server** (`$S/ws/mcpprobe`): `#[tool_router]`, `#[tool]`, `Parameters<T: JsonSchema>`, `Json<T>` output, `ServerConfig::new(...).with_server_info(...)`, `.serve(rmcp::transport::stdio()).await?.waiting()`.
  - A clean debug build takes 30 s.
  - Piped JSON-RPC returned `initialize` (protocol 2025-11-25), `tools/call` with `structuredContent: {"sum":42}`, and the bad-arguments case with `isError: true` and `failed to deserialize parameters: …`.
  - `tools/list` returns `inputSchema` and `outputSchema` as draft 2020-12, generated by schemars 1.
- **Gotchas:**
  - Responses can come back out of order (id 3 before id 2), because handlers run concurrently. Clients must match responses by id.
  - Without `with_server_info`, `serverInfo` is `{"name":"rmcp","version":"3.7.0"}`.
  - Logs must go to stderr. `tracing_subscriber` must use `.with_writer(std::io::stderr)` (the stack does this), or the stdio protocol is corrupted.

## 9. Contract codegen: one source for every language

### Prototype **[V]**

`$S/ws/codegen/src/main.rs` defines a sample contract with:

- a newtype ID;
- unit-variant enums (`Level`, `Layer`);
- a struct with `Option` fields, one skipped when absent and one always present as nullable;
- `Vec` with `#[serde(default)]`;
- `BTreeMap<String, serde_json::Value>` and `BTreeMap<String, u32>`;
- `u64`;
- an internally tagged enum (`#[serde(tag = "kind")]`), an adjacently tagged enum (`tag = "type", content = "data"`) and an externally tagged enum;
- `rename_all` in snake and camel case.

The flow: `schemars` (`SchemaSettings::draft2020_12()`, plus `.for_serialize()`) writes `contract.schema.json` and `contract.serialize.schema.json`. Then `npx json2ts -i … --no-additionalProperties --unreachableDefinitions` (json-schema-to-typescript 16.0.0) turns them into `j2ts.*.ts`. Outputs are in `$S/codegen-out/`.

What works well:

- Tagged enums become proper discriminated unions with `const` tags (`{ kind: "import"; file: string; line: number } | …`).
- The adjacent `{type, data}` form and the external `"Internal" | {NotFound: NodeId} | {Conflict: {...}}` form are right.
- Doc comments carry through as JSDoc, and `rename_all` is respected.
- typify 0.8.0 consumed the same 2020-12 schema and round-tripped a sample JSON in Rust (`$S/typifyprobe`).

### Output problems and fixes

| Problem (seen) | Fix (verified unless marked) |
|---|---|
| Every object gets `[k: string]: unknown`, so typos type-check | `json2ts --no-additionalProperties` (verified), or `#[serde(deny_unknown_fields)]` where wanted [A] |
| Properties come out alphabetical, not in source order | Enable schemars feature `preserve_order` (verified) |
| `#[serde(default)]` fields (`tags`, `attrs`, `countsByLevel`) and a plain `Option<T>` (`summary`) are **not required** in the default (deserialize) contract, so TS gets `tags?: string[]` and `summary?: string \| null` even though the server always sends them | Generate **two** schemas: `.for_deserialize()` for inputs (API requests, imported Artifact rows, pack files) and `.for_serialize()` for outputs. In the serialize schema, `tags: string[]`, `summary: string \| null` and `countsByLevel` are required (verified) |
| A field with `skip_serializing_if = "Option::is_none"` becomes `parent?: NodeId \| null`, but it is never `null` on the wire | Accept it, or post-process with a schemars `Transform` that drops `null` from fields with `skip_serializing_if` [A] |
| `u64` and `i64` become TS `number`, which is lossy above 2^53; the schema keeps `"format": "uint64"`, but TS can't express it | Policy: contract integers stay within 2^53 (versions, counts), or 64-bit values travel as strings. Add a schema lint that fails on `uint64` without an explicit annotation [A] |
| `BTreeMap<Level, u32>` would become `{[k: string]: number}`, not `Record<Level, number>` | Acceptable; the canvas can narrow it |
| Noisy "This interface was referenced by…" comments | Cosmetic; `--bannerComment` and formatting with prettier [A] |

### Alternatives compared

| Option | What it does | Verdict |
|---|---|---|
| **Rust (serde + schemars) → JSON Schema → json-schema-to-typescript** | One derive gives the Rust types, the JSON Schema (needed for MCP `inputSchema`/`outputSchema`, Artifact `db` validation, pack-file validation and CLI `--json` docs) and TS through a language-neutral file | **Recommended.** Rust is where the types live and are exercised most. The schema is a reviewable artifact, any future language (Python, Go) reads the same file, and rmcp already uses schemars 1. |
| JSON Schema files as source + typify (Rust) + json2ts (TS) | Schema-first | Rejected. Hand-writing 2020-12 is verbose. typify 0.8.0 still uses schemars 0.8 (draft-07) internally (`typify-impl 0.8.0` depends on `schemars ^0.8.22`; 0.10 is alpha), so its 2020-12 coverage is a risk. Generated Rust is less idiomatic, and serde attributes such as `rename_all` must be expressed in the schema. |
| ts-rs 12.0.1 | Derive → `.ts` per type (exported during `cargo test`) | Good TS output, and it matched serde semantics well (`summary: string \| null`, `parent?: NodeId`). But `u64` becomes **`bigint`**, which is wrong at runtime because `JSON.parse` returns a `number` (needs `#[ts(type = "number")]` per field). `serde_json::Value` needs the `serde-json-impl` feature. It produces no JSON Schema, so a second derive is needed, and two derives can drift. |
| specta 2.0.0-rc.27 + specta-typescript 0.0.14 + specta-serde 0.0.14 | Derive → one TS file | Still a release candidate after 27 RCs; the last stable is 1.0.5 from 2023. It **refuses** to export `u64` and `serde_json::Value` by default (BigInt forbidden), which needs `#[specta(type = specta_typescript::Number)]` overrides (verified). It emitted `f64` as `number \| null` and kept `#[serde(default)]` fields optional. No JSON Schema. |

### Drift test design (recommended)

1. `crates/planroom-schema` owns every contract type: store records, API requests and responses, events, CLI `--json` envelopes, tool I/O and pack files. It has a `gen` binary, or an `xtask` that writes `schema/*.schema.json`, split into deserialize and serialize variants and committed.
2. A Rust test regenerates the schemas in memory and compares them byte for byte with the committed files. Make the failure message say `cargo xtask schema`.
3. `packages/schema` runs `json2ts` (pinned `json-schema-to-typescript@16.0.0`, `--no-additionalProperties`) over the committed schemas into a marked `generated/` directory. CI regenerates and runs `git diff --exit-code`.
4. Round-trip tests: `insta` JSON snapshots of real engine output, each validated against the serialize schema with `jsonschema` (Rust). The same fixtures are then type-checked or validated in TS. This catches places where schemars and serde disagree (custom `Serialize` impls, `flatten`).
5. Add a version field (`schemaVersion`) to each envelope, and a CI check that a breaking schema change bumps it.

## 10. cargo-deny and nextest in CI

- **`cargo install --locked cargo-deny@0.20.2`: 2 min 30 s** wall (4 min 6 s user) on this VM **[V]**.
- **Prebuilt binaries [V]:**
  - cargo-deny: `https://github.com/EmbarkStudios/cargo-deny/releases/download/0.20.2/cargo-deny-0.20.2-x86_64-unknown-linux-musl.tar.gz` downloaded in 0.56 s (4.9 MB). The published `.sha256` matched `sha256sum`: `9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f`. `cargo-deny --version` printed `cargo-deny 0.20.2`.
  - nextest: `https://get.nexte.st/0.9.149/linux` downloaded in 0.95 s (12 MB) and printed `cargo-nextest 0.9.149 (3fe4ef319 2026-10-20)`. Building nextest from source was not timed.
  - The GitHub REST API (`api.github.com/repos/...`) is blocked from this session's proxy, but plain `github.com/.../releases/download/...` URLs work.
- **`cargo deny check` on the spike workspace [V]:** 2.2 s, including fetching the advisory DB. Result: `advisories ok, bans ok, licenses FAILED, sources ok`. The license failures were only the unlicensed probe crates (fix: `publish = false` plus a license, or `private.ignore = true`).
  - Licenses present in the graph: MIT (348 crates), Apache-2.0 (270), Unicode-3.0 (20), Unlicense (13), ISC (9), Zlib (6), Apache-2.0 WITH LLVM-exception (5), BSD-3-Clause (4), BSD-2-Clause, CC0-1.0 (`notify`, `dunce`), MIT-0, BSL-1.0 (`ryu`), CDLA-Permissive-2.0 (`webpki-root-certs`, through rustls-platform-verifier) and **MPL-2.0 (`uluru`, through gix)**. `r-efi` is triple-licensed and includes LGPL-2.1-or-later as one option.
  - Duplicate crates (warn): `hashbrown` (4), `base64` 0.22/0.23, `digest` 0.10/0.11 (sha2 0.11 against sha1 0.10 from axum `ws` and gix), `syn` 2/3, `rand`, `getrandom`, `foldhash`. Fixed so far: `tower-http` (use 0.6.11).
- **What CI should do:**
  - Install the tools with a pinned-version, pinned-sha256 download step, or `taiki-e/install-action` with an exact version [D].
  - Run `cargo deny check` with the deny.toml settings below.
  - Run `cargo nextest run` plus `cargo test --doc`, since nextest skips doctests [D].
  - Cache `~/.cargo/registry` and `target`; Swatinem/rust-cache is the usual choice [D]. Without a cache, the debug build alone is about 1.7 min on 4 cores.

deny.toml settings to use:

- the license allow-list above, with MPL-2.0 allowed because it is file-level copyleft and unmodified use is fine;
- `bans.deny = [openssl, openssl-sys]`;
- `wildcards = "deny"`, `multiple-versions = "warn"`;
- `sources.unknown-git = "deny"`;
- crate-graph boundary bans, using `[bans] deny = [{ crate = "planroom-server", wrappers = [...] }]` or a separate crate-graph test.

## 11. Gotchas (all hit or seen here unless marked)

- **G1. rusqlite and sqlx cannot coexist in one workspace.** Both declare `links = "sqlite3"`, and sqlx-sqlite 0.9.0 pins `libsqlite3-sys >=0.30.1, <0.38.0`, while rusqlite 0.41 needs `^0.39`. Cargo refused to resolve. Since `Cargo.lock` is per workspace, even a dev-only sqlx crate would break the build.
- **G2.** tree-sitter-typescript 0.23.2 has not been released since 2024-11 and still emits ABI 14. Add an ABI-floor test (section 4).
- **G3.** tree-sitter 0.27 changed `QueryMatch::captures` to a method; matches need `StreamingIterator` in scope.
- **G4.** Debug builds compile tree-sitter's C at `-O0`, so parsing is 3.5 times slower. Set `[profile.dev.package."*"] opt-level = 2`. The `cc` crate follows the profile, so the C code is optimized too.
- **G5.** reqwest 0.13's default `rustls` feature uses **aws-lc-rs**. Its build script is the single slowest unit: 89 s in release, 37 s in debug. It is ISC/Apache/BSD/MIT-0 licensed and may want `cmake`. The `ring` variant is cheaper (section 3) but needs `rustls::crypto::ring::default_provider().install_default()` at startup.
- **G6.** gix leaves fixture repos without an index or worktree; set the default branch explicitly (section 6). gix also has a new 0.x every 2 to 4 weeks.
- **G7.** schemars' default (deserialize) contract makes `#[serde(default)]` and `Option` fields optional. Use `.for_serialize()` for output schemas.
- **G8.** 64-bit integers: json2ts produces `number` (lossy), ts-rs produces `bigint` (wrong at runtime) and specta refuses. Decide on one policy in the contract crate.
- **G9.** ulid 3.0 renamed `Ulid::new()` to `Ulid::generate()`. This is one reason to prefer `uuid` v7.
- **G10.** rmcp: answers can arrive out of order, `serverInfo` defaults to `rmcp`, and nothing may be logged to stdout.
- **G11.** Repository size: the release binary is about 22 MB and the debug `target/` about 1.9 GB per profile. Plan the CI cache and the plugin's engine-bootstrap download, about 7 MB gzipped per target triple.
- **G12.** `notify` is CC0-1.0 and `uluru` is MPL-2.0. Both are fine for an MIT/Apache project, but the deny allow-list must name them.
- **G13.** **[A]** Only Linux x86_64 was built here. macOS (FSEvents in `notify`) and Windows (C toolchains for aws-lc, SQLite and tree-sitter) were not tried. CI should build all three before M1.

## 12. Reproduce

```bash
S=/tmp/claude-0/-home-user-planroom/d26fcf29-d6d2-57f8-acca-1138f06b3754/scratchpad/rust-stack
cd $S/ws && cargo build -p stack && ./target/debug/stack selftest      # crate smoke test
CARGO_TARGET_DIR=$S/t-rel cargo build -p stack --release --timings     # build timing
python3 -I $S/scripts/corpus.py $S/corpus && cargo build -p tsperf --release && ./target/release/tsperf $S/corpus 1
./target/release/gixprobe fixture /tmp/fx && ./target/release/gixprobe walk /tmp/fx v
./target/debug/codegen $S/codegen-out && (cd $S/npx && npx json2ts -i $S/codegen-out/contract.serialize.schema.json --no-additionalProperties)
```
