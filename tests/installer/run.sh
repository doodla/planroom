#!/bin/sh
# The resolver's offline suite (plugin/scripts/planroom). `cargo xtask test --installer` runs it.
# Every test works in a temporary directory, with a temporary data root, and fetches only from a
# release server on 127.0.0.1. Run it with any POSIX sh: sh tests/installer/run.sh [FILTER]
set -eu

repo=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd -P)
resolver="$repo/plugin/scripts/planroom"
filter=${1:-}
work=$(mktemp -d "${TMPDIR:-/tmp}/planroom-installer.XXXXXX")
# The physical path, as the resolver reports its own: on macOS $TMPDIR is /var/... (a link to
# /private/var/...) and ends in a slash.
work=$(CDPATH='' cd -- "$work" && pwd -P)
server_pid=''

cleanup() {
  if [ -n "$server_pid" ]; then
    kill "$server_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# The resolver reads only its own variables; drop any the caller's environment holds.
unset PLANROOM_ENGINE PLANROOM_RESOLVE PLANROOM_RELEASE_BASE_URL PLANROOM_DATA XDG_DATA_HOME

case $(uname -m) in
  x86_64 | amd64) arch=x86_64 ;;
  *) arch=aarch64 ;;
esac
case $(uname -s) in
  Darwin) triple="$arch-apple-darwin" ;;
  *) triple="$arch-unknown-linux-musl" ;;
esac

# --- fixtures -----------------------------------------------------------------------------------

# A stand-in engine: `--version` prints its version, `version --json` reports the catalogue hash
# `h<version>`, as make_plugin's engine.env names it; anything else prints its label and args.
fake_engine() {
  mkdir -p "$(dirname "$1")"
  cat >"$1" <<EOF
#!/bin/sh
if [ "\${1:-}" = --version ]; then echo "planroom $2"
elif [ "\${1:-}" = version ]; then echo '{"ok":true,"result":{"version":"$2","catalogue_hash":"h$2"}}'
else echo "$3: \$*"; fi
EOF
  chmod 755 "$1"
}

# A plugin copy at $1/plugin with engine.env naming version $2 and catalogue hash h$2; with $3 = checkout, $1 is a
# checkout (it holds Cargo.toml).
make_plugin() {
  mkdir -p "$1/plugin/scripts"
  cp "$resolver" "$1/plugin/scripts/planroom"
  cp "$repo/plugin/scripts/contract.sh" "$1/plugin/scripts/contract.sh"
  printf 'PLANROOM_ENGINE_VERSION=%s\nPLANROOM_CATALOGUE_HASH=h%s\nPLANROOM_REPO=doodla/planroom\n' "$2" "$2" \
    >"$1/plugin/scripts/engine.env"
  if [ "${3:-}" = checkout ]; then
    printf '[workspace.package]\nversion = "%s"\nrust-version = "1.97"\n' "$2" >"$1/Cargo.toml"
  fi
}

# The cache directory, or with $3 = src the source clone, of version $2 under the plugin copy at $1:
# keyed by the version and the first 8 characters of make_plugin's catalogue hash, h$2.
cached() {
  printf '%s/data/%s/%s+%.8s' "$1" "${3:-engine}" "$2" "h$2"
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{ print $1 }'
  else
    shasum -a 256 "$1" | awk '{ print $1 }'
  fi
}

# Publish version $1 on the release server, its engine printing label $2; with $3 = bad, its
# SHA256SUMS line names another digest.
publish() {
  dir="$work/www/v$1"
  mkdir -p "$dir/pkg"
  fake_engine "$dir/pkg/planroom" "$1" "$2"
  archive="planroom-v$1-$triple.tar.gz"
  tar -czf "$dir/$archive" -C "$dir/pkg" planroom
  sum=$(sha256_of "$dir/$archive")
  if [ "${3:-}" = bad ]; then
    sum=$(printf '%s' "$sum" | tr '0-9a-f' '1-9a-f0')
  fi
  printf '%s  %s\n' "$sum" "$archive" >"$dir/SHA256SUMS"
}

start_server() {
  mkdir -p "$work/www"
  python3 -u -m http.server 0 --bind 127.0.0.1 --directory "$work/www" >"$work/server.log" 2>&1 &
  server_pid=$!
  tries=0
  while ! grep -q 'port [0-9]' "$work/server.log" 2>/dev/null; do
    tries=$((tries + 1))
    # Up to 30 s: a cold python3 on a busy macOS runner can take longer than a few seconds.
    if [ "$tries" -gt 300 ]; then
      echo "the release server didn't start" >&2
      cat "$work/server.log" >&2
      exit 1
    fi
    sleep 0.1
  done
  port=$(sed -n 's/.* port \([0-9][0-9]*\).*/\1/p' "$work/server.log" | head -n 1)
  base="http://127.0.0.1:$port"
}

# A PATH holding only the tools the resolver uses, plus whatever a test adds to $1/bin.
# The first executable named $1 on PATH. Not `command -v`: BusyBox sh answers it with the bare
# name of its own applet, which can't be linked to.
path_of() {
  saved_ifs=$IFS
  IFS=:
  for dir in $PATH; do
    if [ -n "$dir" ] && [ -f "$dir/$1" ] && [ -x "$dir/$1" ]; then
      IFS=$saved_ifs
      printf '%s\n' "$dir/$1"
      return 0
    fi
  done
  IFS=$saved_ifs
  return 1
}

sandbox_path() {
  mkdir -p "$1/bin"
  for tool in sh sed awk cat find mkdir rm mv chmod date sleep mktemp tar gzip curl sha256sum shasum \
    uname cp dirname head; do
    found=$(path_of "$tool" || true)
    if [ -n "$found" ] && [ ! -e "$1/bin/$tool" ]; then
      ln -s "$found" "$1/bin/$tool"
    fi
  done
  printf '%s/bin' "$1"
}

fake_tool() {
  printf '#!/bin/sh\necho "%s 1.97.0"\n' "$2" >"$1/bin/$2"
  chmod 755 "$1/bin/$2"
}

# A build toolchain in $1/bin that records each call in $1/calls, apart from version checks:
# `cargo build` writes a stand-in engine of version $2, and `git clone` makes its target directory.
# With $3 = canvas, node and pnpm are installed too.
fake_build_tools() {
  fake_engine "$1/built-engine" "$2" built
  for tool in cargo git pnpm; do
    cat >"$1/bin/$tool" <<EOF
#!/bin/sh
case "\$*" in *--version*) echo "$tool 1.97.0"; exit 0 ;; esac
echo "$tool \$*" >>"$1/calls"
case "$tool \${1:-}" in
  "cargo build") mkdir -p target/release && cp "$1/built-engine" target/release/planroom ;;
  "git -c") for arg; do dest=\$arg; done; mkdir -p "\$dest" ;;
esac
EOF
    chmod 755 "$1/bin/$tool"
  done
  fake_tool "$1" cc
  if [ "${3:-}" = canvas ]; then
    fake_tool "$1" node
  else
    rm "$1/bin/pnpm"
  fi
}

# The calls fake_build_tools recorded, one line joined by |.
calls() {
  tr '\n' '|' <"$1/calls"
}

# --- assertions ---------------------------------------------------------------------------------

fail() {
  echo "    $*" >&2
  return 1
}

expect_eq() {
  [ "$1" = "$2" ] || fail "expected [$2], got [$1]${3:+ ($3)}"
}

expect_contains() {
  case $1 in
    *"$2"*) ;;
    *) fail "expected output to contain [$2], got [$1]" ;;
  esac
}

# Run the resolver at $1 with the remaining args; sets $out and $code.
run() {
  script=$1
  shift
  code=0
  out=$("$script" "$@" 2>"$work/stderr") || code=$?
}

# --- tests --------------------------------------------------------------------------------------

override_runs_the_named_binary() {
  d="$work/override"
  make_plugin "$d" 1.0.0
  fake_engine "$d/engine" 0.0.1 override
  PLANROOM_DATA="$d/data" PLANROOM_ENGINE="$d/engine" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$code" 0
  expect_eq "$out" "override: map --json"
}

a_missing_override_is_engine_unavailable() {
  d="$work/override-missing"
  make_plugin "$d" 1.0.0
  PLANROOM_DATA="$d/data" PLANROOM_ENGINE="$d/nothing" run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 7
  expect_contains "$out" '"code":"engine_unavailable"'
  expect_contains "$out" '"reason":"override_missing"'
}

dev_checkout_runs_the_newest_matching_build() {
  d="$work/checkout"
  make_plugin "$d" 1.0.0 checkout
  fake_engine "$d/target/debug/planroom" 1.0.0 debug
  fake_engine "$d/target/release/planroom" 1.0.0 release
  touch -t 200001010000 "$d/target/release/planroom"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$out" "debug: focus --json"
  touch "$d/target/release/planroom"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$out" "release: focus --json"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" status --json
  expect_contains "$out" '"step":"checkout"'
  [ ! -d "$d/data/engine" ] || fail "a checkout resolve wrote to the cache"
}

a_stale_checkout_build_names_the_rebuild() {
  d="$work/stale"
  make_plugin "$d" 1.0.0 checkout
  fake_engine "$d/target/debug/planroom" 0.9.0 stale
  # A cached release must not stand in for the stale build.
  mkdir -p "$(cached "$d" 1.0.0)"
  fake_engine "$(cached "$d" 1.0.0)/planroom" 1.0.0 cache
  echo x >"$(cached "$d" 1.0.0)/.ok"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 7
  expect_contains "$out" '"reason":"stale_build"'
  expect_contains "$out" "cargo build -p planroom"
}

resolve_release_skips_the_checkout() {
  d="$work/release-mode"
  make_plugin "$d" 1.1.0 checkout
  fake_engine "$d/target/debug/planroom" 1.1.0 checkout
  publish 1.1.0 release
  PLANROOM_DATA="$d/data" PLANROOM_RESOLVE=release PLANROOM_RELEASE_BASE_URL="$base" \
    run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 0
  expect_eq "$out" "release: focus --json"
}

a_verified_download_is_cached() {
  d="$work/download"
  make_plugin "$d" 1.2.0
  publish 1.2.0 downloaded
  PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL="$base" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  expect_eq "$out" "downloaded: map --json"
  expect_eq "$(cat "$(cached "$d" 1.2.0)/.ok")" "$(sha256_of "$(cached "$d" 1.2.0)/planroom")"
  [ -f "$(cached "$d" 1.2.0)/last_used" ] || fail "no last_used stamp"
  # The second run needs no server: it comes from the cache.
  rm -rf "$work/www/v1.2.0"
  PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL="$base" run "$d/plugin/scripts/planroom" status --json
  expect_contains "$out" '"step":"cache"'
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$out" "downloaded: map --json"
}

a_bad_checksum_is_refused() {
  d="$work/bad-sum"
  make_plugin "$d" 1.3.0
  publish 1.3.0 tampered bad
  PATH=$(sandbox_path "$d") PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL="$base" \
    run "$d/plugin/scripts/planroom" map --json
  expect_eq "$code" 7
  expect_contains "$out" '"code":"engine_unavailable"'
  expect_contains "$out" "doesn't match its SHA256SUMS entry"
  [ ! -e "$(cached "$d" 1.3.0)/planroom" ] || fail "a tampered engine was cached"
  [ -z "$(find "$d/data/engine" -name '.fetch.*')" ] || fail "the download's temp directory was left"
  [ ! -d "$d/data/engine/.lock" ] || fail "the fetch lock was left"
}

base_url_refuses_plain_http_to_a_non_loopback_host() {
  d="$work/base-url"
  make_plugin "$d" 1.4.0
  for url in http://example.com/r "http://127.0.0.1.example.com/r" "http://localhost:80@example.com/r" \
    ftp://127.0.0.1/r; do
    PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL="$url" run "$d/plugin/scripts/planroom" map --json
    expect_eq "$code" 7 "$url"
    expect_contains "$out" "PLANROOM_RELEASE_BASE_URL must use https"
  done
}

a_dev_version_goes_straight_to_the_build_card() {
  d="$work/dev"
  make_plugin "$d" 1.5.0-dev
  path=$(sandbox_path "$d")
  fake_tool "$d" cargo
  fake_tool "$d" cc
  # No base URL: no request is made at all, so a server isn't needed.
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 7
  expect_contains "$out" '"reason":"no_release"'
  expect_contains "$out" "\"fix\":\"$d/plugin/scripts/planroom build\""
  expect_contains "$out" '"est_minutes":4'
  [ ! -d "$d/data/engine/.lock" ] || fail "a dev version took the fetch lock"
}

a_missing_toolchain_names_its_install() {
  d="$work/no-toolchain"
  make_plugin "$d" 1.6.0-dev
  path=$(sandbox_path "$d")
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 7
  expect_contains "$out" '"reason":"no_toolchain"'
  expect_contains "$out" '"missing":"cargo"'
  expect_contains "$out" "rustup"
  fake_tool "$d" cargo
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" focus --json
  expect_contains "$out" '"missing":"cc"'
}

resolve_checkout_skips_the_cache_and_the_download() {
  d="$work/checkout-mode"
  make_plugin "$d" 1.7.0
  mkdir -p "$(cached "$d" 1.7.0)"
  fake_engine "$(cached "$d" 1.7.0)/planroom" 1.7.0 cache
  echo x >"$(cached "$d" 1.7.0)/.ok"
  path=$(sandbox_path "$d")
  fake_tool "$d" cargo
  fake_tool "$d" cc
  PATH=$path PLANROOM_DATA="$d/data" PLANROOM_RESOLVE=checkout run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 7
  expect_contains "$out" '"reason":"no_build"'
  PLANROOM_DATA="$d/data" PLANROOM_RESOLVE=bogus run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 2
  expect_contains "$out" '"code":"invalid_argument"'
}

a_stale_fetch_lock_is_broken() {
  d="$work/lock"
  make_plugin "$d" 1.8.0
  publish 1.8.0 unlocked
  mkdir -p "$d/data/engine/.lock"
  # A pid that can't be running: the process that took the lock died with its session.
  echo "999999 $(date +%s)" >"$d/data/engine/.lock/owner"
  PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL="$base" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  expect_eq "$out" "unlocked: map --json"
}

prune_keeps_a_recently_used_version() {
  d="$work/prune"
  make_plugin "$d" 1.9.0
  publish 1.9.0 fresh
  for v in 0.1.0 0.2.0; do
    mkdir -p "$d/data/engine/$v"
    fake_engine "$d/data/engine/$v/planroom" "$v" old
    echo x >"$d/data/engine/$v/.ok"
    : >"$d/data/engine/$v/last_used"
  done
  touch -t 200001010000 "$d/data/engine/0.1.0/last_used"
  PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL="$base" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  [ ! -d "$d/data/engine/0.1.0" ] || fail "a version unused for 7 days was kept"
  [ -x "$d/data/engine/0.2.0/planroom" ] || fail "a recently used version was pruned"
}

runs_with_no_claude_variables() {
  d="$work/env-i"
  make_plugin "$d" 2.0.0 checkout
  fake_engine "$d/target/debug/planroom" 2.0.0 bare
  out=$(env -i HOME="$d/home" PATH=/usr/bin:/bin "$d/plugin/scripts/planroom" focus --json)
  expect_eq "$out" "bare: focus --json"
}

a_checkout_build_builds_the_canvas_first() {
  d="$work/build-canvas"
  make_plugin "$d" 3.0.0-dev checkout
  path=$(sandbox_path "$d")
  fake_build_tools "$d" 3.0.0-dev canvas
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" build
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  expect_eq "$(calls "$d")" \
    "pnpm install --frozen-lockfile --offline|cargo xtask gen --canvas|cargo build --release --locked -p planroom|"
}

a_build_without_node_has_no_canvas() {
  d="$work/build-no-node"
  make_plugin "$d" 3.1.0-dev checkout
  path=$(sandbox_path "$d")
  fake_build_tools "$d" 3.1.0-dev
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" build
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  expect_eq "$(calls "$d")" "cargo build --release --locked -p planroom|"
  expect_contains "$(cat "$work/stderr")" "this engine has no canvas"
}

a_clone_build_is_cached_and_its_clone_removed() {
  d="$work/build-clone"
  make_plugin "$d" 3.2.0-dev
  path=$(sandbox_path "$d")
  fake_build_tools "$d" 3.2.0-dev canvas
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" build
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  expect_contains "$(calls "$d")" "|pnpm install --frozen-lockfile --offline|cargo xtask gen --canvas|cargo build"
  [ -x "$(cached "$d" 3.2.0-dev)/planroom" ] || fail "the built engine wasn't cached"
  [ ! -e "$(cached "$d" 3.2.0-dev src)" ] || fail "the clone was left in the data root"
}

the_repo_checkout_resolves_its_own_build() {
  want=$(sed -n 's/^PLANROOM_ENGINE_VERSION=//p' "$repo/plugin/scripts/engine.env")
  if [ ! -x "$repo/target/debug/planroom" ] && [ ! -x "$repo/target/release/planroom" ]; then
    fail "no engine is built in $repo/target; run cargo build -p planroom first"
  fi
  PLANROOM_DATA="$work/repo-data" run "$resolver" --version
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  expect_eq "$out" "planroom $want"
  PLANROOM_DATA="$work/repo-data" run "$resolver" status --json
  expect_contains "$out" '"step":"checkout"'
}

the_resolver_spells_no_code_or_reason_itself() {
  contract="$repo/plugin/scripts/contract.sh"
  codes=$(sed -n 's/.*"code":"\([a-z_]*\)".*/\1/p' "$contract")
  reasons=$(sed -n 's/^contract_reason_[a-z_]*=//p' "$contract")
  if [ -z "$codes" ] || [ -z "$reasons" ]; then
    fail "contract.sh names no codes or reasons"
  fi
  for word in $codes $reasons; do
    if grep -v '^[[:space:]]*#' "$resolver" | grep -q -w -- "$word"; then
      fail "the resolver spells $word itself; take it from contract.sh"
    fi
  done
}

# Each --json output is one JSON document; an envelope's size counts its own bytes.
resolver_outputs_are_one_document_with_their_size() {
  d="$work/outputs"
  make_plugin "$d" 1.0.0
  sized='import json, sys
text = sys.stdin.read()
doc = json.loads(text)
size = doc["meta"]["size"]
assert size["bytes"] == len(text.encode()), (size, len(text.encode()))
assert size["tokens_est"] == (size["bytes"] + 3) // 4, size
print(doc["error"]["code"])'
  PLANROOM_DATA="$d/data" PLANROOM_ENGINE="$d/nothing" run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 7
  expect_eq "$(printf '%s' "$out" | python3 -I -c "$sized")" engine_unavailable "$out"
  PLANROOM_DATA="$d/data" PLANROOM_RESOLVE=bogus run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 2
  expect_eq "$(printf '%s' "$out" | python3 -I -c "$sized")" invalid_argument "$out"
  PLANROOM_DATA="$d/data" PLANROOM_RESOLVE=release run "$d/plugin/scripts/planroom" status --json
  expect_eq "$code" 1
  expect_eq "$out" '{"ok":false,"step":"download","version":"1.0.0"}'
}

# The mod sets PLANROOM_ENGINE and PLANROOM_RELEASE_BASE_URL empty on every run it starts, so an
# inherited value never picks the engine a press runs: the resolver reads an empty one as unset.
empty_overrides_count_as_unset() {
  d="$work/empty-overrides"
  make_plugin "$d" 3.0.0 checkout
  fake_engine "$d/target/debug/planroom" 3.0.0 debug
  PLANROOM_DATA="$d/data" PLANROOM_ENGINE='' PLANROOM_RELEASE_BASE_URL='' \
    run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  expect_eq "$out" "debug: focus --json"
  PLANROOM_DATA="$d/data" PLANROOM_ENGINE='' run "$d/plugin/scripts/planroom" status --json
  expect_contains "$out" '"step":"checkout"'
  # An empty base is the default one: a -dev version still has no release to fetch.
  d="$work/empty-base-dev"
  make_plugin "$d" 3.1.0-dev
  path=$(sandbox_path "$d")
  fake_tool "$d" cargo
  fake_tool "$d" cc
  PATH=$path PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL='' \
    run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 7
  expect_contains "$out" '"reason":"no_release"'
}

# A binary swapped into the cache after the install doesn't match the sha256 its `.ok` recorded, so
# the resolver treats the cache as empty: status names the download, and the next run fetches the
# release again instead of running the swapped binary (security-r2-1).
a_swapped_cached_engine_is_refused() {
  d="$work/swapped"
  make_plugin "$d" 2.1.0
  publish 2.1.0 genuine
  PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL="$base" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$out" "genuine: map --json" "$(cat "$work/stderr")"
  fake_engine "$(cached "$d" 2.1.0)/planroom" 2.1.0 swapped
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" status --json
  expect_eq "$code" 1
  expect_eq "$out" '{"ok":false,"step":"download","version":"2.1.0"}'
  PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL="$base" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  expect_eq "$out" "genuine: map --json"
  expect_eq "$(cat "$(cached "$d" 2.1.0)/.ok")" "$(sha256_of "$(cached "$d" 2.1.0)/planroom")"
  # A link in the binary's place is no install either, even to a binary whose sum matches.
  mv "$(cached "$d" 2.1.0)/planroom" "$d/genuine"
  ln -s "$d/genuine" "$(cached "$d" 2.1.0)/planroom"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" status --json
  expect_eq "$code" 1
}

# The resolver places the data root as the engine does (unasked-r3-3): PLANROOM_DATA, else an
# absolute XDG_DATA_HOME, else HOME's. With none absolute it refuses, naming PLANROOM_DATA, and
# writes no engine cache into the folder it runs in.
a_data_root_with_no_absolute_place_is_refused() {
  d="$work/no-home"
  make_plugin "$d" 1.0.0
  mkdir -p "$d/cwd"
  r="$d/plugin/scripts/planroom"
  for case in unset empty relative-home relative-data relative-xdg; do
    code=0
    out=$(
      cd "$d/cwd"
      case $case in
        unset) unset HOME && "$r" map --json ;;
        empty) HOME='' "$r" map --json ;;
        relative-home) HOME=home "$r" map --json ;;
        relative-data) HOME="$d/home" PLANROOM_DATA=data "$r" map --json ;;
        relative-xdg) unset HOME && XDG_DATA_HOME=share "$r" map --json ;;
      esac 2>"$work/stderr"
    ) || code=$?
    expect_eq "$code" 2 "$case: $(cat "$work/stderr")"
    expect_contains "$out" '"code":"invalid_argument"'
    expect_contains "$out" '"field":"PLANROOM_DATA"'
  done
  [ -z "$(ls -A "$d/cwd")" ] || fail "the resolver wrote into the folder it ran in: $(ls -A "$d/cwd")"
}

# The cache is keyed by the version and the catalogue hash, so a newer -dev build, which serves
# another catalogue under the same version, never resolves the older one (DR-78).
a_newer_dev_catalogue_misses_the_cache() {
  d="$work/dev-catalogue"
  make_plugin "$d" 4.0.0-dev
  path=$(sandbox_path "$d")
  fake_build_tools "$d" 4.0.0-dev
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" build
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" status --json
  expect_eq "$code" 0
  expect_contains "$out" '"step":"cache"'
  expect_contains "$out" "$(cached "$d" 4.0.0-dev)/planroom"
  # The plugin moves on to a newer catalogue under the same version: the old build isn't it.
  sed 's/^PLANROOM_CATALOGUE_HASH=.*/PLANROOM_CATALOGUE_HASH=0123abcd-newer/' \
    "$d/plugin/scripts/engine.env" >"$d/engine.env" && mv "$d/engine.env" "$d/plugin/scripts/engine.env"
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" focus --json
  expect_eq "$code" 7
  expect_contains "$out" '"reason":"no_release"'
  [ -x "$(cached "$d" 4.0.0-dev)/planroom" ] || fail "the older catalogue's build was removed"
}

# A rebuild swaps the new engine in so that a resolve at any step finds an engine it installed:
# every rename the build makes is followed by a status from another process, and each says cache.
a_rebuild_never_leaves_the_cache_missing() {
  d="$work/rebuild"
  make_plugin "$d" 4.1.0-dev
  path=$(sandbox_path "$d")
  fake_build_tools "$d" 4.1.0-dev
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" build
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  first=$(sha256_of "$(cached "$d" 4.1.0-dev)/planroom")
  # The second build makes another engine of the same version and catalogue.
  fake_engine "$d/built-engine" 4.1.0-dev rebuilt
  rm "$d/bin/mv"
  real_mv=$(path_of mv)
  cat >"$d/bin/mv" <<EOF
#!/bin/sh
"$real_mv" "\$@" || exit 1
PLANROOM_DATA="$d/data" "$d/plugin/scripts/planroom" status >>"$d/steps" 2>&1 || echo "missing" >>"$d/steps"
EOF
  chmod 755 "$d/bin/mv"
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" build
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  [ -s "$d/steps" ] || fail "the build renamed nothing"
  if grep -v '^cache ' "$d/steps" >/dev/null; then
    fail "a resolve during the rebuild found no cache: $(tr '\n' '|' <"$d/steps")"
  fi
  expect_eq "$(cat "$(cached "$d" 4.1.0-dev)/.ok")" "$(sha256_of "$(cached "$d" 4.1.0-dev)/planroom")"
  [ "$(sha256_of "$(cached "$d" 4.1.0-dev)/planroom")" != "$first" ] || fail "the rebuild wasn't installed"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$out" "rebuilt: map --json"
}

# An install cut short leaves one engine valid, never two. `.ok`'s second sum, the engine being
# replaced, counts only while the new engine is staged beside it as `planroom.new`: cut short before
# the rename, the old engine still runs; cut short after it, the old engine swapped back in is no
# install, though `.ok` still names its sum on its second line.
an_interrupted_install_leaves_one_engine_valid() {
  d="$work/interrupted"
  make_plugin "$d" 2.2.0
  publish 2.2.0 genuine
  PLANROOM_DATA="$d/data" PLANROOM_RELEASE_BASE_URL="$base" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$out" "genuine: map --json" "$(cat "$work/stderr")"
  c=$(cached "$d" 2.2.0)
  fake_engine "$d/older" 2.2.0 older
  # Cut short after the rename: the new engine is in place and `.ok` still names both sums.
  printf '%s\n%s\n' "$(sha256_of "$c/planroom")" "$(sha256_of "$d/older")" >"$c/.ok"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" status --json
  expect_eq "$code" 0
  expect_contains "$out" '"step":"cache"'
  cp "$d/older" "$c/planroom"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" status --json
  expect_eq "$code" 1 "the replaced engine counted once its successor was in place: $out"
  expect_eq "$out" '{"ok":false,"step":"download","version":"2.2.0"}'
  # Cut short before the rename: the old engine runs while the new one waits staged beside it.
  fake_engine "$c/planroom.new" 2.2.0 staged
  printf '%s\n%s\n' "$(sha256_of "$c/planroom.new")" "$(sha256_of "$c/planroom")" >"$c/.ok"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  expect_eq "$out" "older: map --json"
  # A staged binary that isn't the one the first sum names keeps nothing valid but the first sum.
  fake_engine "$c/planroom.new" 2.2.0 other
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" status --json
  expect_eq "$code" 1 "$out"
}

# An install that follows one cut short with its successor staged keeps the old engine valid at
# every step: the old engine counts only through `.ok`'s second line, and restaging `planroom.new`
# with a third engine must not leave it uncounted. Every copy and rename the rebuild makes is
# followed by a status from another process, and each says cache.
an_install_after_one_cut_short_never_leaves_the_cache_missing() {
  d="$work/resumed"
  make_plugin "$d" 4.2.0-dev
  path=$(sandbox_path "$d")
  fake_build_tools "$d" 4.2.0-dev
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" build
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  c=$(cached "$d" 4.2.0-dev)
  # Cut short before the rename: a staged successor, and `.ok` naming it first and the old second.
  fake_engine "$c/planroom.new" 4.2.0-dev staged
  printf '%s\n%s\n' "$(sha256_of "$c/planroom.new")" "$(sha256_of "$c/planroom")" >"$c/.ok"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$out" "built: map --json" "$(cat "$work/stderr")"
  # The next build makes a third engine of the same version and catalogue.
  fake_engine "$d/built-engine" 4.2.0-dev rebuilt
  for tool in cp mv; do
    real=$(path_of "$tool")
    rm "$d/bin/$tool"
    cat >"$d/bin/$tool" <<EOF
#!/bin/sh
"$real" "\$@" || exit 1
PLANROOM_DATA="$d/data" "$d/plugin/scripts/planroom" status >>"$d/steps" 2>&1 \
  || echo "missing after $tool \$*" >>"$d/steps"
EOF
    chmod 755 "$d/bin/$tool"
  done
  PATH=$path PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" build
  expect_eq "$code" 0 "$(cat "$work/stderr")"
  [ -s "$d/steps" ] || fail "the build copied and renamed nothing"
  if grep -v '^cache ' "$d/steps" >/dev/null; then
    fail "a resolve during the install found no cache: $(tr '\n' '|' <"$d/steps")"
  fi
  expect_eq "$(cat "$c/.ok")" "$(sha256_of "$c/planroom")"
  [ ! -e "$c/planroom.new" ] || fail "the staged engine was left behind"
  PLANROOM_DATA="$d/data" run "$d/plugin/scripts/planroom" map --json
  expect_eq "$out" "rebuilt: map --json"
}

# --- runner -------------------------------------------------------------------------------------

start_server
passed=0
failed=0
for t in override_runs_the_named_binary a_missing_override_is_engine_unavailable \
  dev_checkout_runs_the_newest_matching_build a_stale_checkout_build_names_the_rebuild \
  resolve_release_skips_the_checkout a_verified_download_is_cached a_bad_checksum_is_refused \
  base_url_refuses_plain_http_to_a_non_loopback_host a_dev_version_goes_straight_to_the_build_card \
  a_missing_toolchain_names_its_install resolve_checkout_skips_the_cache_and_the_download \
  a_stale_fetch_lock_is_broken prune_keeps_a_recently_used_version runs_with_no_claude_variables \
  a_checkout_build_builds_the_canvas_first a_build_without_node_has_no_canvas \
  a_clone_build_is_cached_and_its_clone_removed the_repo_checkout_resolves_its_own_build \
  the_resolver_spells_no_code_or_reason_itself resolver_outputs_are_one_document_with_their_size \
  a_swapped_cached_engine_is_refused a_data_root_with_no_absolute_place_is_refused \
  empty_overrides_count_as_unset a_newer_dev_catalogue_misses_the_cache \
  a_rebuild_never_leaves_the_cache_missing an_interrupted_install_leaves_one_engine_valid \
  an_install_after_one_cut_short_never_leaves_the_cache_missing; do
  case $t in *"$filter"*) ;; *) continue ;; esac
  # Not an `if` condition: there `set -e` would be off, and a failed assertion would go on.
  set +e
  (
    set -e
    $t
  )
  status=$?
  set -e
  if [ "$status" -eq 0 ]; then
    echo "ok       $t"
    passed=$((passed + 1))
  else
    echo "FAILED   $t"
    failed=$((failed + 1))
  fi
done
echo "installer: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
