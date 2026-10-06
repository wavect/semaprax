#!/bin/sh
# Distribution journey for the Unix installer (docs/INSTALLER-V1.md).
#
#   scripts/install-journey.sh <release-assets-dir> <tag>
#
# <release-assets-dir> holds the exact candidate assets for ONE target: the
# target archive semaprax-<tag>-<target>.tar.gz, SHA256SUMS,
# release-manifest.json, release-attestation-<target>.json and install.sh. The
# runner executes THAT directory's install.sh (never the checkout's) against a
# copy of the assets laid out as <base>/<tag>/..., into a fresh prefix that
# contains a space, under a fresh HOME, then walks the beginner journey from a
# fresh directory outside any checkout.
#
# The installer itself runs with the caller's PATH (so `gh` can verify the
# publisher); every command after installation runs with PATH reduced to
# "<prefix>/bin:/usr/bin:/bin". Set SEMAPRAX_JOURNEY_REQUIRE_PUBLISHER=1 to pass
# --require-publisher-verification. Set SEMAPRAX_JOURNEY_KEEP=1 to keep the
# scratch directory.
#
# The last line printed is machine-readable:
#   JOURNEY-RESULT tag=<tag> target=<target> archive_sha256=<hex> publisher=<verified|not-verified> outcome=<pass|fail>

set -eu

[ $# -eq 2 ] || {
  echo "usage: $0 <release-assets-dir> <tag>" >&2
  exit 2
}
ASSETS="$(cd "$1" && pwd)"
TAG="$2"
VERSION="${TAG#v}"

OUTCOME="fail"
TARGET="unknown"
ARCHIVE_SHA="unknown"
PUBLISHER="unknown"
WORK=""

finish() {
  rc=$?
  trap - EXIT
  if [ -n "$WORK" ] && [ "${SEMAPRAX_JOURNEY_KEEP:-0}" != 1 ]; then
    rm -rf "$WORK"
  elif [ -n "$WORK" ]; then
    echo "journey: scratch kept at $WORK" >&2
  fi
  printf 'JOURNEY-RESULT tag=%s target=%s archive_sha256=%s publisher=%s outcome=%s\n' \
    "$TAG" "$TARGET" "$ARCHIVE_SHA" "$PUBLISHER" "$OUTCOME"
  exit "$rc"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

step() { printf 'journey: %s\n' "$*"; }
fail() {
  printf 'journey: FAIL: %s\n' "$*" >&2
  exit 1
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

# --------------------------------------------------------------- inputs

printf '%s\n' "$TAG" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' || fail "invalid tag $TAG"
INSTALLER="$ASSETS/install.sh"
[ -f "$INSTALLER" ] || fail "$ASSETS/install.sh is missing"
for f in SHA256SUMS release-manifest.json; do
  [ -f "$ASSETS/$f" ] || fail "$ASSETS/$f is missing"
done
ARCHIVES="$(find "$ASSETS" -maxdepth 1 -name "semaprax-$TAG-*.tar.gz" | sort)"
[ -n "$ARCHIVES" ] || fail "no semaprax-$TAG-*.tar.gz archive in $ASSETS"
case "$ARCHIVES" in
  *"
"*) fail "more than one archive in $ASSETS; the journey needs exactly the candidate target archive" ;;
esac
ARCHIVE_NAME="$(basename "$ARCHIVES")"
TARGET="${ARCHIVE_NAME#semaprax-"$TAG"-}"
TARGET="${TARGET%.tar.gz}"
[ -f "$ASSETS/release-attestation-$TARGET.json" ] || fail "release-attestation-$TARGET.json is missing"
ARCHIVE_SHA="$(sha256_of "$ASSETS/$ARCHIVE_NAME")"
TOP="semaprax-$TAG-$TARGET"

TMP_ROOT="${TMPDIR:-/tmp}"
WORK="$(mktemp -d "${TMP_ROOT%/}/semaprax-journey.XXXXXX")"
case "$WORK" in
  *" "*) fail "scratch directory must not contain a space (it is part of a file:// URL): $WORK" ;;
esac
WORK="$(cd "$WORK" && pwd -P)"

BASE="$WORK/base"
mkdir -p "$BASE/$TAG" "$WORK/home" "$WORK/fresh" "$WORK/ref"
for f in "$ASSETS"/*; do
  cp "$f" "$BASE/$TAG/"
done
HOME_DIR="$WORK/home"
PREFIX="$WORK/with space/semaprax"
FRESH="$WORK/fresh"

# A fresh directory outside any checkout.
if command -v git >/dev/null 2>&1 && git -C "$FRESH" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  fail "$FRESH is inside a git work tree; set TMPDIR outside the checkout"
fi

# Reference members straight from the archive, for byte comparison.
tar -xzf "$ASSETS/$ARCHIVE_NAME" -C "$WORK/ref"
REF_CLI="$WORK/ref/$TOP/semaprax"
REF_DAEMON="$WORK/ref/$TOP/semapraxd"
[ -x "$REF_CLI" ] && [ -x "$REF_DAEMON" ] || fail "archive members semaprax/semapraxd missing"

# ----------------------------------------------------------- installer runs

run_installer() {
  # $1 = download base, remaining = installer args
  rb="$1"
  shift
  env HOME="$HOME_DIR" SHELL=/bin/sh \
    SEMAPRAX_INSTALL_DOWNLOAD_BASE="file://$rb" \
    sh "$INSTALLER" "$@"
}

PUB_FLAG=""
if [ "${SEMAPRAX_JOURNEY_REQUIRE_PUBLISHER:-0}" = 1 ]; then
  PUB_FLAG="--require-publisher-verification"
fi

step "install $TAG ($TARGET) into '$PREFIX'"
# shellcheck disable=SC2086  # PUB_FLAG is intentionally empty or one flag
run_installer "$BASE" --version "$TAG" --prefix "$PREFIX" $PUB_FLAG >"$WORK/install.out" 2>&1 ||
  {
    cat "$WORK/install.out" >&2
    fail "installer exited nonzero"
  }
cat "$WORK/install.out"
grep -q "installed semaprax $TAG" "$WORK/install.out" || fail "installer printed no success line"
if grep -q "publisher: verified" "$WORK/install.out"; then
  PUBLISHER="verified"
else
  PUBLISHER="not-verified"
fi
if [ "${SEMAPRAX_JOURNEY_REQUIRE_PUBLISHER:-0}" = 1 ] && [ "$PUBLISHER" != verified ]; then
  fail "publisher verification was required but not reported verified"
fi

RECEIPT="$PREFIX/install-receipt.json"
[ -f "$RECEIPT" ] || fail "no install receipt"
for want in '"schema": "semaprax.install-receipt.v1"' "\"tag\": \"$TAG\"" "\"target\": \"$TARGET\"" \
  "\"archive_sha256\": \"$ARCHIVE_SHA\"" "\"version\": \"$VERSION\""; do
  grep -qF "$want" "$RECEIPT" || fail "receipt lacks $want"
done
grep -qF "\"publisher_verification\": \"$(printf '%s' "$PUBLISHER" | tr -d '\n')\"" "$RECEIPT" ||
  fail "receipt publisher_verification does not match the installer output"
grep -qF "versions/$TAG/semapraxd" "$RECEIPT" || fail "receipt does not own semapraxd of $TAG"

JPATH="$PREFIX/bin:/usr/bin:/bin"
in_journey() { env HOME="$HOME_DIR" PATH="$JPATH" "$@"; }

step "resolution and byte identity"
RESOLVED="$(in_journey sh -c 'command -v semaprax')"
[ "$RESOLVED" = "$PREFIX/bin/semaprax" ] || fail "command -v semaprax resolved to '$RESOLVED', expected inside the prefix"
RESOLVED_D="$(in_journey sh -c 'command -v semapraxd')"
[ "$RESOLVED_D" = "$PREFIX/bin/semapraxd" ] || fail "command -v semapraxd resolved to '$RESOLVED_D'"
[ "$(sha256_of "$PREFIX/versions/$TAG/semaprax")" = "$(sha256_of "$REF_CLI")" ] || fail "installed semaprax differs from the archive member"
[ "$(sha256_of "$PREFIX/versions/$TAG/semapraxd")" = "$(sha256_of "$REF_DAEMON")" ] || fail "installed semapraxd differs from the archive member"
[ "$(sha256_of "$PREFIX/bin/semaprax")" = "$(sha256_of "$REF_CLI")" ] || fail "bin/semaprax does not resolve to the archive member"
[ -x "$PREFIX/bin/semapraxd" ] || fail "semapraxd is not executable"

step "beginner journey from $FRESH"
cd "$FRESH"
VERSION_LINE="$(in_journey semaprax --version)"
printf '%s\n' "$VERSION_LINE"
case "$VERSION_LINE" in
  "semaprax $VERSION "* | "semaprax $VERSION") ;;
  *) fail "semaprax --version does not name $VERSION: $VERSION_LINE" ;;
esac
in_journey semaprax new first-semaprax
in_journey semaprax check first-semaprax/semaprax.toml
in_journey semaprax test first-semaprax/semaprax.toml
in_journey semaprax run first-semaprax/semaprax.toml >"$WORK/run.out"
cat "$WORK/run.out"
[ "$(tr -d '\r' <"$WORK/run.out" | sed -n '$p')" = 42 ] || fail "semaprax run did not print 42"

step "semapraxd identity and bounded protocol handshake"
# Receipt identity: the daemon is owned by the same version directory as the CLI
# (checked above). Behavioral identity: the packaged daemon's bounded stdio
# transport answers `protocol` with its own version (see
# tests/release_archive_product_v1/daemon.rs), then `shutdown`.
printf '%s\n%s\n' '{"jsonrpc":"2.0","id":1,"method":"protocol"}' \
  '{"jsonrpc":"2.0","id":2,"method":"shutdown"}' >"$WORK/daemon.in"
in_journey semapraxd --stdio --manifest-path "$FRESH/first-semaprax/semaprax.toml" \
  --max-request-bytes 65536 --max-response-bytes 1048576 \
  <"$WORK/daemon.in" >"$WORK/daemon.out" 2>"$WORK/daemon.err" &
DPID=$!
i=0
while kill -0 "$DPID" 2>/dev/null; do
  i=$((i + 1))
  if [ "$i" -gt 30 ]; then
    kill -9 "$DPID" 2>/dev/null || :
    wait "$DPID" 2>/dev/null || :
    fail "semapraxd did not exit within 30 seconds"
  fi
  sleep 1
done
wait "$DPID" || fail "semapraxd exited nonzero: $(cat "$WORK/daemon.err")"
DOUT="$(tr -d ' ' <"$WORK/daemon.out")"
case "$DOUT" in
  *'"protocol":"semaprax.agent-transport.v'*) ;;
  *) fail "semapraxd protocol response missing: $DOUT" ;;
esac
case "$DOUT" in
  *"\"version\":\"$VERSION\""*) ;;
  *) fail "semapraxd does not report version $VERSION: $DOUT" ;;
esac

step "same-version reinstall"
BEFORE="$(cat "$RECEIPT")"
# shellcheck disable=SC2086
run_installer "$BASE" --version "$TAG" --prefix "$PREFIX" $PUB_FLAG >"$WORK/reinstall.out" 2>&1 ||
  {
    cat "$WORK/reinstall.out" >&2
    fail "same-version reinstall failed"
  }
grep -q "already installed and active" "$WORK/reinstall.out" || fail "reinstall did not report an unchanged install"
[ "$(cat "$RECEIPT")" = "$BEFORE" ] || fail "reinstall changed the receipt"
[ "$(grep -cF '# >>> semaprax installer >>>' "$HOME_DIR/.profile")" = 1 ] || fail "reinstall duplicated the profile block"
[ "$(sha256_of "$PREFIX/bin/semaprax")" = "$(sha256_of "$REF_CLI")" ] || fail "reinstall changed the executable"

step "damaged-artifact control (previous install must survive)"
DAMAGED="$WORK/damaged"
mkdir -p "$DAMAGED/$TAG"
cp "$ASSETS"/* "$DAMAGED/$TAG/"
DARCHIVE="$DAMAGED/$TAG/$ARCHIVE_NAME"
orig="$(dd if="$DARCHIVE" bs=1 skip=100 count=1 2>/dev/null | od -An -tu1 | tr -d ' \n')"
flipped=$((orig ^ 255))
# shellcheck disable=SC2059  # the octal escape is built on purpose
printf "\\$(printf '%03o' "$flipped")" | dd of="$DARCHIVE" bs=1 seek=100 conv=notrunc 2>/dev/null
[ "$(sha256_of "$DARCHIVE")" != "$ARCHIVE_SHA" ] || fail "the damage control did not change the archive"
if run_installer "$DAMAGED" --version "$TAG" --prefix "$PREFIX" --no-modify-path >"$WORK/damaged.out" 2>&1; then
  cat "$WORK/damaged.out" >&2
  fail "installer accepted a damaged archive"
fi
grep -q "checksum mismatch" "$WORK/damaged.out" || {
  cat "$WORK/damaged.out" >&2
  fail "damaged archive was not rejected as a checksum mismatch"
}
if grep -Eq "installed semaprax|already installed and active" "$WORK/damaged.out"; then
  fail "damaged install printed a success line"
fi
[ "$(cat "$RECEIPT")" = "$BEFORE" ] || fail "damaged install changed the receipt"
[ "$(in_journey semaprax --version)" = "$VERSION_LINE" ] || fail "previous install no longer works after the damaged attempt"
[ -z "$(find "$PREFIX" -maxdepth 1 -name '.install-staging.*')" ] || fail "damaged install left staging behind"
# A damaged fresh install must leave no prefix at all.
FRESH_PREFIX="$WORK/damaged prefix/semaprax"
if run_installer "$DAMAGED" --version "$TAG" --prefix "$FRESH_PREFIX" --no-modify-path >"$WORK/damaged2.out" 2>&1; then
  fail "installer accepted a damaged archive into a fresh prefix"
fi
[ ! -e "$FRESH_PREFIX" ] || fail "damaged fresh install left $FRESH_PREFIX behind"

step "uninstall"
run_installer "$BASE" --uninstall --prefix "$PREFIX" >"$WORK/uninstall.out" 2>&1 ||
  {
    cat "$WORK/uninstall.out" >&2
    fail "uninstall failed"
  }
cat "$WORK/uninstall.out"
[ ! -e "$PREFIX" ] || fail "uninstall left $PREFIX behind"
# gh keeps its own cache under HOME; only the installer's footprint matters.
[ ! -e "$HOME_DIR/.profile" ] || fail "uninstall left $HOME_DIR/.profile"
[ -z "$(grep -rl 'semaprax installer' "$HOME_DIR" 2>/dev/null | head -1)" ] || fail "uninstall left a PATH block under HOME"
[ -f "$FRESH/first-semaprax/semaprax.toml" ] || fail "uninstall touched the user's project"

OUTCOME="pass"
step "journey passed"
