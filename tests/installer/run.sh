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
  printf 'PLANROOM_ENGINE_VERSION=%s\nPLANROOM_CATALOGUE_HASH=h%s\nPLANROOM_REPO=doodla/planroom\n' "$2" "$2" \
    >"$1/plugin/scripts/engine.env"
  if [ "${3:-}" = checkout ]; then
    printf '[workspace.package]\nversion = "%s"\nrust-version = "1.97"\n' "$2" >"$1/Cargo.toml"
  fi
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
  mkdir -p "$d/data/engine/1.0.0"
  fake_engine "$d/data/engine/1.0.0/planroom" 1.0.0 cache
  echo x >"$d/data/engine/1.0.0/.ok"
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
  expect_eq "$(cat "$d/data/engine/1.2.0/.ok")" "$(sha256_of "$d/data/engine/1.2.0/planroom")"
  [ -f "$d/data/engine/1.2.0/last_used" ] || fail "no last_used stamp"
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
  [ ! -e "$d/data/engine/1.3.0/planroom" ] || fail "a tampered engine was cached"
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
  mkdir -p "$d/data/engine/1.7.0"
  fake_engine "$d/data/engine/1.7.0/planroom" 1.7.0 cache
  echo x >"$d/data/engine/1.7.0/.ok"
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
  the_repo_checkout_resolves_its_own_build; do
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
