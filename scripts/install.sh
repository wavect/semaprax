#!/bin/sh
# SEMAPRAX Unix installer (docs/INSTALLER-V1.md).
#
#   curl -fsSL https://github.com/wavect/semaprax/releases/latest/download/install.sh | sh
#   curl -fsSL https://github.com/wavect/semaprax/releases/latest/download/install.sh | sh -s -- --version v0.9.0
#
# Installs the published full `semaprax` CLI and `semapraxd` for the current
# user. It never uses sudo, never prompts, and touches nothing outside the
# install prefix except one marked block in a shell profile (unless
# --no-modify-path). Every download is verified before anything is activated.
#
# Test-only environment overrides (never needed in normal use):
#   SEMAPRAX_INSTALL_DOWNLOAD_BASE   asset URL base; asset = $BASE/<tag>/<name>
#   SEMAPRAX_INSTALL_LATEST_URL      URL whose redirect names the latest tag
#   SEMAPRAX_INSTALL_TEST_UNAME_S / _UNAME_M / _LIBC / _ROSETTA
#                                    fake `uname -s`, `uname -m`, libc
#                                    ("glibc 2.35" or "musl"), and
#                                    sysctl.proc_translated (0 or 1)

set -eu
umask 022

REPO="wavect/semaprax"
SIGNER_WORKFLOW="wavect/semaprax/.github/workflows/ci.yml"
DEFAULT_BASE="https://github.com/wavect/semaprax/releases/download"
DEFAULT_LATEST="https://github.com/wavect/semaprax/releases/latest"
# Single source of truth for the Unix target list; the Windows target is owned
# by install.ps1. A contract test compares this with ARCHIVE_TARGETS.
SUPPORTED_TARGETS="x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu aarch64-apple-darwin x86_64-apple-darwin"
GLIBC_MIN_MAJOR=2
GLIBC_MIN_MINOR=35
BLOCK_BEGIN="# >>> semaprax installer >>>"
BLOCK_END="# <<< semaprax installer <<<"

TAG=""
PREFIX=""
MODIFY_PATH=1
UNINSTALL=0
REQUIRE_PUBLISHER=0

# Mutable state consulted by the EXIT trap.
STAGE=""
STATE="init"            # init -> activated -> done
FRESH_PREFIX_ROOT=""    # highest directory this run created for the prefix
CREATED_VERSIONS=0
NEW_DEST=""
CREATED_BIN=0
CREATED_LINKS=""
ACTIVATED=0
PREV_CURRENT=""
PROFILE_ADDED=""
MV_MODE="ln"

say() { printf 'semaprax-install: %s\n' "$*"; }
warn() { printf 'semaprax-install: warning: %s\n' "$*" >&2; }
die() {
  printf 'semaprax-install: error: %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: install.sh [options]

Install the SEMAPRAX CLI (semaprax) and daemon (semapraxd) for the current user.

Options:
  --version <tag>                  Install this exact release tag (e.g. v0.9.0).
                                   Default: the latest published release,
                                   resolved once.
  --prefix <dir>                   Install under <dir> (default: $HOME/.semaprax).
  --no-modify-path                 Do not edit any shell profile.
  --yes                            Accepted for noninteractive use; the installer
                                   never prompts.
  --uninstall                      Remove an installation made by this installer.
  --require-publisher-verification Fail unless the release attestation is
                                   verified with `gh attestation verify`.
  --help                           Show this help.

Without `gh` the installer verifies the SHA-256 checksum only and says so.
EOF
}

# ---------------------------------------------------------------- arguments

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      [ $# -ge 2 ] || die "--version needs a value"
      TAG="$2"
      shift 2
      ;;
    --version=*)
      TAG="${1#--version=}"
      shift
      ;;
    --prefix)
      [ $# -ge 2 ] || die "--prefix needs a value"
      PREFIX="$2"
      shift 2
      ;;
    --prefix=*)
      PREFIX="${1#--prefix=}"
      shift
      ;;
    --no-modify-path)
      MODIFY_PATH=0
      shift
      ;;
    --yes | -y)
      shift
      ;;
    --uninstall)
      UNINSTALL=1
      shift
      ;;
    --require-publisher-verification)
      REQUIRE_PUBLISHER=1
      shift
      ;;
    --help | -h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      die "unknown option: $1"
      ;;
  esac
done

# --------------------------------------------------------------- validation

NL='
'

if [ -n "$TAG" ]; then
  case "$TAG" in
    [0-9]*) TAG="v$TAG" ;;
  esac
  printf '%s\n' "$TAG" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' ||
    die "invalid release tag '$TAG' (expected e.g. v0.9.0)"
fi

if [ -z "$PREFIX" ]; then
  [ -n "${HOME:-}" ] || die "HOME is not set; pass --prefix <dir>"
  PREFIX="$HOME/.semaprax"
fi
case "$PREFIX" in
  /*) ;;
  *) PREFIX="$(pwd)/$PREFIX" ;;
esac
while :; do
  case "$PREFIX" in
    */) PREFIX="${PREFIX%/}" ;;
    *) break ;;
  esac
done
[ -n "$PREFIX" ] || die "refusing to use / as the install prefix"
case "$PREFIX" in
  *"$NL"* | *'"'* | *'$'* | *'`'* | *\\*)
    die "the install prefix may not contain a newline, quote, dollar sign, backtick or backslash"
    ;;
  */.. | */../* | */. | */./*)
    die "the install prefix must not contain . or .. components"
    ;;
esac

need() {
  command -v "$1" >/dev/null 2>&1 || die "required tool not found on PATH: $1"
}

# ----------------------------------------------------------------- helpers

json_escape() {
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

is_link_or_exists() { [ -e "$1" ] || [ -L "$1" ]; }

# shellcheck disable=SC2317,SC2329  # called from the EXIT trap
rmdir_chain() {
  # rmdir $1 and its parents while empty, stopping after $2.
  rc_dir="$1"
  rc_stop="$2"
  while :; do
    rmdir "$rc_dir" 2>/dev/null || break
    [ "$rc_dir" = "$rc_stop" ] && break
    rc_dir="$(dirname "$rc_dir")"
  done
}

# Remove the marked block from stdin. Mode all=1 drops any installer block;
# otherwise only a block that mentions $2 (the prefix bin directory).
strip_block() {
  awk -v all="$1" -v p="$2" -v begin="$BLOCK_BEGIN" -v end="$BLOCK_END" '
    $0 == begin { inb = 1; buf = $0 ORS; own = (all == 1); next }
    inb {
      buf = buf $0 ORS
      if (all != 1 && index($0, p) > 0) own = 1
      if ($0 == end) { inb = 0; if (!own) printf "%s", buf }
      next
    }
    { print }
    END { if (inb) printf "%s", buf }'
}

# shellcheck disable=SC2016  # $PATH is written literally into the profile
block_text() {
  # $1 = profile file name, $2 = prefix bin
  case "$1" in
    *.fish)
      printf '%s\n' "$BLOCK_BEGIN"
      printf 'if not contains -- "%s" $PATH\n' "$2"
      printf '    set -gx PATH "%s" $PATH\n' "$2"
      printf 'end\n'
      printf '%s\n' "$BLOCK_END"
      ;;
    *)
      printf '%s\n' "$BLOCK_BEGIN"
      printf 'case ":$PATH:" in\n'
      printf '    *":%s:"*) ;;\n' "$2"
      printf '    *) export PATH="%s:$PATH" ;;\n' "$2"
      printf 'esac\n'
      printf '%s\n' "$BLOCK_END"
      ;;
  esac
}

profile_candidates() {
  [ -n "${HOME:-}" ] || return 0
  printf '%s\n' "$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.bash_profile" \
    "$HOME/.profile" "$HOME/.config/fish/conf.d/semaprax.fish"
}

# ------------------------------------------------------------------- trap

# shellcheck disable=SC2317,SC2329  # installed as the EXIT trap
cleanup() {
  rc=$?
  trap - EXIT INT TERM HUP
  if [ "$STATE" != "done" ]; then
    if [ -n "$PROFILE_ADDED" ]; then
      printf '%s\n' "$PROFILE_ADDED" | while IFS= read -r pf; do
        [ -n "$pf" ] || continue
        if [ -f "$pf" ]; then
          strip_block 1 "" <"$pf" >"$pf.semaprax-tmp.$$" 2>/dev/null || :
          if [ -s "$pf.semaprax-tmp.$$" ]; then
            cat "$pf.semaprax-tmp.$$" >"$pf" 2>/dev/null || :
          else
            rm -f "$pf" 2>/dev/null || :
          fi
          rm -f "$pf.semaprax-tmp.$$" 2>/dev/null || :
        fi
      done
    fi
    if [ "$ACTIVATED" = 1 ]; then
      if [ -n "$PREV_CURRENT" ]; then
        activate_link "$PREV_CURRENT" 2>/dev/null || :
      else
        rm -f "$PREFIX/current" 2>/dev/null || :
      fi
    fi
    if [ -n "$NEW_DEST" ]; then
      rm -rf "$NEW_DEST" 2>/dev/null || :
    fi
    if [ -n "$CREATED_LINKS" ]; then
      printf '%s\n' "$CREATED_LINKS" | while IFS= read -r lk; do
        if [ -n "$lk" ]; then rm -f "$lk" 2>/dev/null || :; fi
      done
    fi
    if [ "$CREATED_BIN" = 1 ]; then rmdir "$PREFIX/bin" 2>/dev/null || :; fi
    if [ "$CREATED_VERSIONS" = 1 ]; then rmdir "$PREFIX/versions" 2>/dev/null || :; fi
  fi
  if [ -n "$STAGE" ]; then
    rm -rf "$STAGE" 2>/dev/null || :
  fi
  if [ "$STATE" != "done" ] && [ -n "$FRESH_PREFIX_ROOT" ]; then
    rmdir_chain "$PREFIX" "$FRESH_PREFIX_ROOT"
  fi
  exit "$rc"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP

activate_link() {
  al_tmp="$PREFIX/.current.new.$$"
  rm -f "$al_tmp"
  ln -s "$1" "$al_tmp" || return 1
  case "$MV_MODE" in
    gnu) mv -T "$al_tmp" "$PREFIX/current" ;;
    bsd) mv -h "$al_tmp" "$PREFIX/current" ;;
    *)
      ln -sfn "$1" "$PREFIX/current" || return 1
      rm -f "$al_tmp"
      ;;
  esac
}

probe_mv_mode() {
  pm="$STAGE/mvprobe"
  mkdir -p "$pm/a" "$pm/b"
  ln -s a "$pm/cur"
  ln -s b "$pm/new"
  if mv -T "$pm/new" "$pm/cur" 2>/dev/null && [ "$(readlink "$pm/cur")" = b ]; then
    MV_MODE=gnu
  else
    rm -f "$pm/cur" "$pm/new"
    ln -s a "$pm/cur"
    ln -s b "$pm/new"
    if mv -h "$pm/new" "$pm/cur" 2>/dev/null && [ "$(readlink "$pm/cur")" = b ]; then
      MV_MODE=bsd
    else
      MV_MODE="ln"
    fi
  fi
  rm -rf "$pm"
}

# -------------------------------------------------------------- receipt IO

RECEIPT="$PREFIX/install-receipt.json"

receipt_has_schema() {
  [ -f "$RECEIPT" ] && grep -q '"schema": *"semaprax.install-receipt.v1"' "$RECEIPT"
}

receipt_field() {
  sed -n "s/^  \"$1\": *\"\\([^\"]*\\)\",\\{0,1\\}\$/\\1/p" "$RECEIPT" | head -1
}

receipt_files() {
  sed -n '/^  "files": \[/,/^  \],\{0,1\}$/p' "$RECEIPT" |
    sed -n 's/^    "\(.*\)",\{0,1\}$/\1/p'
}

safe_relpath() {
  case "$1" in
    "" | /* | .. | ../* | */.. | */../* | *\\* | *'"'*) return 1 ;;
  esac
  return 0
}

# --------------------------------------------------------------- uninstall

do_uninstall() {
  [ -d "$PREFIX" ] || die "nothing to uninstall: $PREFIX does not exist"
  receipt_has_schema ||
    die "refusing to uninstall: $PREFIX has no semaprax install receipt"
  for tool in sed grep awk rm rmdir cat dirname cmp; do need "$tool"; done

  files="$(receipt_files)"
  [ -n "$files" ] || die "install receipt lists no files; refusing to guess what to remove"
  printf '%s\n' "$files" | while IFS= read -r rel; do
    safe_relpath "$rel" || {
      printf 'semaprax-install: error: install receipt lists an unsafe path: %s\n' "$rel" >&2
      exit 1
    }
  done || exit 1

  printf '%s\n' "$files" | while IFS= read -r rel; do
    rm -f "$PREFIX/$rel"
  done
  # Empty directories the installer created; anything unrelated keeps them.
  printf '%s\n' "$files" | while IFS= read -r rel; do
    d="$(dirname "$rel")"
    while [ "$d" != "." ] && [ "$d" != "/" ]; do
      rmdir "$PREFIX/$d" 2>/dev/null || break
      d="$(dirname "$d")"
    done
  done
  rm -f "$PREFIX/current" "$PREFIX/.current.new".* 2>/dev/null || :
  rmdir "$PREFIX/versions" 2>/dev/null || :
  rmdir "$PREFIX/bin" 2>/dev/null || :
  rm -f "$RECEIPT"

  # Our own PATH block, wherever the standard profiles carry it.
  profile_candidates | while IFS= read -r pf; do
    if [ -z "$pf" ] || [ ! -f "$pf" ]; then continue; fi
    grep -qF "$BLOCK_BEGIN" "$pf" || continue
    tmpf="$pf.semaprax-tmp.$$"
    strip_block 0 "$PREFIX/bin" <"$pf" >"$tmpf"
    if cmp -s "$pf" "$tmpf"; then
      rm -f "$tmpf"
    elif [ -s "$tmpf" ]; then
      cat "$tmpf" >"$pf"
      rm -f "$tmpf"
      say "removed PATH block from $pf"
    else
      rm -f "$tmpf" "$pf"
      say "removed PATH block from $pf (file is now empty and was removed)"
    fi
  done

  if rmdir "$PREFIX" 2>/dev/null; then
    say "removed $PREFIX"
  else
    say "left $PREFIX in place: it still contains files this installer does not own"
  fi
  STATE="done"
  say "uninstalled semaprax from $PREFIX"
  say "projects, agent configuration and credentials were not touched"
  exit 0
}

if [ "$UNINSTALL" = 1 ]; then
  do_uninstall
fi

# ------------------------------------------------------------ platform

UNAME_S="${SEMAPRAX_INSTALL_TEST_UNAME_S:-}"
UNAME_M="${SEMAPRAX_INSTALL_TEST_UNAME_M:-}"
if [ -z "$UNAME_S" ]; then UNAME_S="$(uname -s)"; fi
if [ -z "$UNAME_M" ]; then UNAME_M="$(uname -m)"; fi

source_hint() {
  tag_hint="${TAG:-<tag>}"
  cat >&2 <<EOF

Build from source instead (needs a Rust toolchain >= 1.88):
  cargo install --locked --git https://github.com/$REPO --tag $tag_hint semaprax
This installs the STANDALONE 'semaprax' and 'semapraxd' from the root package.
The release archive's 'semaprax' is the FULL build. To build that one:
  cargo install --locked --git https://github.com/$REPO --tag $tag_hint semaprax-toolchain --bin semaprax-full
which installs it under the name 'semaprax-full'.
EOF
}

unsupported() {
  printf 'semaprax-install: error: %s\n' "$*" >&2
  source_hint
  exit 1
}

detect_libc() {
  if [ -n "${SEMAPRAX_INSTALL_TEST_LIBC+x}" ]; then
    printf '%s\n' "$SEMAPRAX_INSTALL_TEST_LIBC"
    return 0
  fi
  if command -v getconf >/dev/null 2>&1; then
    gl="$(getconf GNU_LIBC_VERSION 2>/dev/null || :)"
    case "$gl" in
      glibc\ *)
        printf '%s\n' "$gl"
        return 0
        ;;
    esac
  fi
  if command -v ldd >/dev/null 2>&1 && ldd --version 2>&1 | grep -qi musl; then
    printf 'musl\n'
    return 0
  fi
  printf '\n'
}

TARGET=""
case "$UNAME_S" in
  Darwin)
    case "$UNAME_M" in
      arm64 | aarch64) TARGET="aarch64-apple-darwin" ;;
      x86_64)
        if [ -n "${SEMAPRAX_INSTALL_TEST_ROSETTA+x}" ]; then
          translated="$SEMAPRAX_INSTALL_TEST_ROSETTA"
        else
          translated="$(sysctl -n sysctl.proc_translated 2>/dev/null || printf 0)"
        fi
        if [ "$translated" = 1 ]; then
          # A translated shell on Apple silicon: install the native build.
          TARGET="aarch64-apple-darwin"
        else
          TARGET="x86_64-apple-darwin"
        fi
        ;;
      *) unsupported "unsupported macOS CPU architecture: $UNAME_M" ;;
    esac
    ;;
  Linux)
    case "$UNAME_M" in
      x86_64 | amd64) TARGET="x86_64-unknown-linux-gnu" ;;
      aarch64 | arm64) TARGET="aarch64-unknown-linux-gnu" ;;
      *) unsupported "unsupported Linux CPU architecture: $UNAME_M" ;;
    esac
    libc="$(detect_libc)"
    case "$libc" in
      glibc\ *)
        gver="${libc#glibc }"
        gmaj="${gver%%.*}"
        grest="${gver#*.}"
        gmin="${grest%%[!0-9]*}"
        case "$gmaj$gmin" in
          *[!0-9]* | "") unsupported "cannot parse the glibc version '$libc'" ;;
        esac
        if [ "$gmaj" -lt "$GLIBC_MIN_MAJOR" ] ||
          { [ "$gmaj" -eq "$GLIBC_MIN_MAJOR" ] && [ "$gmin" -lt "$GLIBC_MIN_MINOR" ]; }; then
          unsupported "glibc $gver is older than the supported baseline glibc $GLIBC_MIN_MAJOR.$GLIBC_MIN_MINOR"
        fi
        ;;
      musl*)
        unsupported "musl libc (for example Alpine) is not supported; the release is built for GNU/Linux (glibc)"
        ;;
      *)
        unsupported "cannot confirm GNU libc; the release is built for GNU/Linux (glibc >= $GLIBC_MIN_MAJOR.$GLIBC_MIN_MINOR)"
        ;;
    esac
    ;;
  *) unsupported "unsupported operating system: $UNAME_S" ;;
esac
case " $SUPPORTED_TARGETS " in
  *" $TARGET "*) ;;
  *) unsupported "internal error: $TARGET is not a supported target" ;;
esac

# ------------------------------------------------------- tool preflight

for tool in tar awk sed grep tr mkdir mv ln rm rmdir cat wc dirname basename \
  readlink find sort head cmp uname; do
  need "$tool"
done
if command -v curl >/dev/null 2>&1; then
  DOWNLOADER=curl
elif command -v wget >/dev/null 2>&1; then
  DOWNLOADER=wget
else
  die "need curl or wget to download the release"
fi
if command -v sha256sum >/dev/null 2>&1; then
  SHA_TOOL=sha256sum
elif command -v shasum >/dev/null 2>&1; then
  SHA_TOOL=shasum
else
  die "need sha256sum or shasum to verify the download"
fi

GH_STATE="missing"
if command -v gh >/dev/null 2>&1; then
  if gh attestation --help >/dev/null 2>&1; then
    GH_STATE="ok"
  else
    GH_STATE="no-attestation"
  fi
fi
if [ "$REQUIRE_PUBLISHER" = 1 ] && [ "$GH_STATE" != "ok" ]; then
  die "--require-publisher-verification needs the GitHub CLI (gh) with 'gh attestation verify' on PATH; install gh or drop the flag to accept checksum-only verification"
fi

sha256_of() {
  if [ "$SHA_TOOL" = sha256sum ]; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

BASE="${SEMAPRAX_INSTALL_DOWNLOAD_BASE:-$DEFAULT_BASE}"
LATEST_URL="${SEMAPRAX_INSTALL_LATEST_URL:-$DEFAULT_LATEST}"
while :; do
  case "$BASE" in
    */) BASE="${BASE%/}" ;;
    *) break ;;
  esac
done

fetch() {
  case "$DOWNLOADER" in
    curl)
      curl --fail --silent --show-error --location --retry 2 \
        --connect-timeout 30 --output "$2" "$1"
      ;;
    wget)
      case "$1" in
        file://*) die "file:// URLs need curl" ;;
      esac
      wget --quiet --tries=3 --timeout=30 --output-document="$2" "$1"
      ;;
  esac
}

resolve_latest() {
  case "$DOWNLOADER" in
    curl)
      final="$(curl --fail --silent --show-error --location --connect-timeout 30 \
        --output /dev/null --write-out '%{url_effective}' "$LATEST_URL")" ||
        die "could not resolve the latest release from $LATEST_URL"
      ;;
    wget)
      final="$(wget --quiet --server-response --max-redirect=0 --output-document=/dev/null \
        "$LATEST_URL" 2>&1 | sed -n 's/^ *[Ll]ocation: *//p' | head -1 | tr -d '\r')" || :
      [ -n "$final" ] || die "could not resolve the latest release from $LATEST_URL"
      ;;
  esac
  case "$final" in
    */tag/*) ;;
    *) die "latest-release URL did not redirect to a tag page: $final" ;;
  esac
  TAG="${final##*/}"
  printf '%s\n' "$TAG" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' ||
    die "latest release resolved to an unusable tag: $TAG"
}

# ------------------------------------------------------------ resolve tag

if [ -z "$TAG" ]; then
  if [ -n "${SEMAPRAX_INSTALL_DOWNLOAD_BASE:-}" ] && [ -z "${SEMAPRAX_INSTALL_LATEST_URL:-}" ]; then
    die "--version is required when SEMAPRAX_INSTALL_DOWNLOAD_BASE is overridden without SEMAPRAX_INSTALL_LATEST_URL"
  fi
  resolve_latest
  say "latest release resolved once: $TAG"
fi
VERSION="${TAG#v}"
NAME="semaprax-$TAG-$TARGET.tar.gz"
TOP="semaprax-$TAG-$TARGET"
ATT_NAME="release-attestation-$TARGET.json"
SOURCE_URL="$BASE/$TAG/$NAME"
say "target $TARGET, release $TAG"

# ----------------------------------------------------------- prefix state

PREV_TAG=""
PREV_SHA=""
if [ -e "$PREFIX" ] || [ -L "$PREFIX" ]; then
  [ -d "$PREFIX" ] || die "$PREFIX exists and is not a directory"
  if receipt_has_schema; then
    PREV_TAG="$(receipt_field tag)"
    PREV_SHA="$(receipt_field archive_sha256)"
  else
    if [ -n "$(find "$PREFIX" -mindepth 1 -maxdepth 1 2>/dev/null | head -1)" ]; then
      die "refusing to install into $PREFIX: it is not empty and has no semaprax install receipt"
    fi
  fi
else
  # Remember the highest directory we create so a failed install can undo it.
  FRESH_PREFIX_ROOT="$PREFIX"
  while :; do
    parent="$(dirname "$FRESH_PREFIX_ROOT")"
    [ -e "$parent" ] && break
    FRESH_PREFIX_ROOT="$parent"
  done
  mkdir -p "$PREFIX" || die "cannot create $PREFIX"
fi
for n in semaprax semapraxd; do
  p="$PREFIX/bin/$n"
  if is_link_or_exists "$p"; then
    if [ ! -L "$p" ] || [ "$(readlink "$p")" != "../current/$n" ]; then
      die "refusing to replace $p: it is not owned by this installer"
    fi
  fi
done

STAGE="$PREFIX/.install-staging.$$"
rm -rf "$STAGE"
mkdir "$STAGE" || die "cannot create a staging directory in $PREFIX"
probe_mv_mode

# --------------------------------------------------------------- download

say "downloading verification material and $NAME"
fetch "$BASE/$TAG/SHA256SUMS" "$STAGE/SHA256SUMS" || die "could not download SHA256SUMS for $TAG"
fetch "$BASE/$TAG/release-manifest.json" "$STAGE/release-manifest.json" ||
  die "could not download release-manifest.json for $TAG"
fetch "$BASE/$TAG/$ATT_NAME" "$STAGE/$ATT_NAME" ||
  die "could not download $ATT_NAME for $TAG"
fetch "$SOURCE_URL" "$STAGE/$NAME" || die "could not download $NAME"
[ -s "$STAGE/$ATT_NAME" ] || die "$ATT_NAME is empty"

# ----------------------------------------------------------- verification

ARCHIVE="$STAGE/$NAME"
ACTUAL_SHA="$(sha256_of "$ARCHIVE")"
ACTUAL_SIZE="$(wc -c <"$ARCHIVE" | tr -d ' ')"

SUMS_MATCHES="$(awk -v n="$NAME" '{ f = $2; sub(/^\*/, "", f); if (f == n) print $1 }' "$STAGE/SHA256SUMS")"
case "$SUMS_MATCHES" in
  "") die "SHA256SUMS has no line for $NAME" ;;
  *"$NL"*) die "SHA256SUMS has more than one line for $NAME" ;;
esac
printf '%s\n' "$SUMS_MATCHES" | grep -Eq '^[0-9a-f]{64}$' || die "SHA256SUMS line for $NAME is malformed"
[ "$SUMS_MATCHES" = "$ACTUAL_SHA" ] ||
  die "checksum mismatch for $NAME: SHA256SUMS says $SUMS_MATCHES, downloaded file is $ACTUAL_SHA"

MFLAT="$(tr -d ' \t\r\n' <"$STAGE/release-manifest.json")"
[ -n "$MFLAT" ] || die "release-manifest.json is empty"
printf '%s' "$MFLAT" | grep -q '"schema":"semaprax.release-manifest.v1"' ||
  die "release-manifest.json has an unexpected schema"
M_TAG="$(printf '%s' "$MFLAT" | grep -o '"tag":"[^"]*"' | head -1 | sed 's/^"tag":"//; s/"$//')"
[ "$M_TAG" = "$TAG" ] || die "release-manifest.json is for tag '$M_TAG', expected $TAG"
M_ENTRY="$(printf '%s' "$MFLAT" | tr '}' '\n' | grep -F "\"name\":\"$NAME\"" || :)"
case "$M_ENTRY" in
  "") die "release-manifest.json has no artifacts entry for $NAME" ;;
  *"$NL"*) die "release-manifest.json lists $NAME more than once" ;;
esac
M_PLATFORM="$(printf '%s' "$M_ENTRY" | grep -o '"platform":"[^"]*"' | sed -n '$p' | sed 's/^"platform":"//; s/"$//')"
M_SIZE="$(printf '%s' "$M_ENTRY" | grep -o '"size":[0-9]*' | sed -n '$p' | sed 's/^"size"://')"
M_DIGEST="$(printf '%s' "$M_ENTRY" | grep -o '"digest":"[^"]*"' | sed -n '$p' | sed 's/^"digest":"//; s/"$//')"
[ "$M_PLATFORM" = "$TARGET" ] ||
  die "release-manifest.json says $NAME is for '$M_PLATFORM', expected $TARGET"
if [ -z "$M_SIZE" ] || [ "$M_SIZE" != "$ACTUAL_SIZE" ]; then
  die "release-manifest.json size '$M_SIZE' does not match the downloaded $ACTUAL_SIZE bytes"
fi
[ "$M_DIGEST" = "sha256:$ACTUAL_SHA" ] ||
  die "release-manifest.json digest '$M_DIGEST' does not match the downloaded sha256:$ACTUAL_SHA"
say "checksum verified: SHA256SUMS and release-manifest.json agree on sha256:$ACTUAL_SHA"

PUBLISHER="not-verified"
case "$GH_STATE" in
  ok)
    if gh attestation verify "$ARCHIVE" --bundle "$STAGE/$ATT_NAME" --repo "$REPO" \
      --signer-workflow "$SIGNER_WORKFLOW" --source-ref "refs/tags/$TAG" \
      --deny-self-hosted-runners >"$STAGE/gh.out" 2>&1; then
      PUBLISHER="verified"
      say "publisher: verified (gh attestation verify: $SIGNER_WORKFLOW at refs/tags/$TAG)"
    else
      cat "$STAGE/gh.out" >&2
      die "publisher verification failed for $NAME; nothing was installed"
    fi
    ;;
  no-attestation)
    say "publisher: not verified (installed gh has no 'attestation' command) - checksum only"
    ;;
  *)
    say "publisher: not verified (gh not found) - checksum only"
    ;;
esac

# -------------------------------------------------------------- extraction

LIST="$(tar -tzf "$ARCHIVE")" || die "cannot read $NAME as a gzip tar archive"
VLIST="$(tar -tvzf "$ARCHIVE")" || die "cannot read $NAME as a gzip tar archive"
[ -n "$LIST" ] || die "$NAME is empty"
printf '%s\n' "$LIST" | while IFS= read -r m; do
  case "$m" in
    "" | /* | .. | ../* | */.. | */../* | *\\*)
      printf 'semaprax-install: error: unsafe archive member: %s\n' "$m" >&2
      exit 1
      ;;
  esac
  case "$m" in
    "$TOP" | "$TOP"/*) ;;
    *)
      printf 'semaprax-install: error: archive member outside %s/: %s\n' "$TOP" "$m" >&2
      exit 1
      ;;
  esac
  printf '%s\n' "$m" | grep -Eq '^[A-Za-z0-9._/-]+$' || {
    printf 'semaprax-install: error: archive member has unexpected characters: %s\n' "$m" >&2
    exit 1
  }
done || exit 1
printf '%s\n' "$VLIST" | while IFS= read -r v; do
  case "$v" in
    -* | d*) ;;
    *)
      printf 'semaprax-install: error: archive contains a link or special member: %s\n' "$v" >&2
      exit 1
      ;;
  esac
done || exit 1

mkdir "$STAGE/extract"
tar -xzf "$ARCHIVE" -C "$STAGE/extract" || die "extraction of $NAME failed"
PKG="$STAGE/extract/$TOP"
[ -d "$PKG" ] || die "archive does not contain the expected top-level directory $TOP/"
[ -z "$(find "$STAGE/extract" -type l 2>/dev/null | head -1)" ] || die "archive extracted a symbolic link"
[ "$(find "$STAGE/extract" -mindepth 1 -maxdepth 1 | wc -l | tr -d ' ')" = 1 ] ||
  die "archive has more than the expected top-level directory"
for exe in semaprax semapraxd; do
  if [ ! -f "$PKG/$exe" ] || [ ! -x "$PKG/$exe" ]; then die "archive is missing the executable $exe"; fi
done

# ------------------------------------------------------------ staged smoke

SMOKE_OUT="$("$PKG/semaprax" version --json 2>&1)" ||
  die "staged smoke failed: 'semaprax version --json' exited nonzero: $SMOKE_OUT"
SMOKE_FLAT="$(printf '%s' "$SMOKE_OUT" | tr -d ' \t\r\n')"
printf '%s' "$SMOKE_FLAT" | grep -qF "\"version\":\"$VERSION\"" ||
  die "staged smoke failed: 'semaprax version --json' did not report version $VERSION: $SMOKE_OUT"
say "staged smoke passed: semaprax $VERSION"

# -------------------------------------------------------------- activation

DEST="$PREFIX/versions/$TAG"
if [ ! -d "$PREFIX/versions" ]; then
  mkdir "$PREFIX/versions"
  CREATED_VERSIONS=1
fi
REUSE=0
if [ -e "$DEST" ] || [ -L "$DEST" ]; then
  if [ "$PREV_TAG" = "$TAG" ]; then
    [ "$PREV_SHA" = "$ACTUAL_SHA" ] ||
      die "$TAG is already installed from a different archive (sha256:$PREV_SHA); uninstall first"
    REUSE=1
  else
    rm -rf "$DEST"
  fi
fi
if [ "$REUSE" = 0 ]; then
  mv "$PKG" "$DEST" || die "could not place the package in $DEST"
  NEW_DEST="$DEST"
fi

if [ ! -d "$PREFIX/bin" ]; then
  mkdir "$PREFIX/bin"
  CREATED_BIN=1
fi
for n in semaprax semapraxd; do
  if ! [ -L "$PREFIX/bin/$n" ]; then
    ln -s "../current/$n" "$PREFIX/bin/$n" || die "could not create $PREFIX/bin/$n"
    CREATED_LINKS="$CREATED_LINKS$PREFIX/bin/$n$NL"
  fi
done

if [ -L "$PREFIX/current" ]; then
  PREV_CURRENT="$(readlink "$PREFIX/current")"
elif [ -e "$PREFIX/current" ]; then
  die "refusing to replace $PREFIX/current: it is not a symbolic link"
fi
if [ "$PREV_CURRENT" = "versions/$TAG" ] && [ "$REUSE" = 1 ]; then
  : # already the active version
else
  ACTIVATED=1
  activate_link "versions/$TAG" || die "could not activate $TAG"
fi
STATE="activated"
[ "$("$PREFIX/bin/semaprax" version --json 2>&1 | tr -d ' \t\r\n' | grep -cF "\"version\":\"$VERSION\"")" = 1 ] ||
  die "activated install does not report version $VERSION"

# ---------------------------------------------------------------- profile

PM_KIND="none"
PM_LOCATION=""
if [ "$MODIFY_PATH" = 1 ]; then
  if [ -z "${HOME:-}" ]; then
    warn "HOME is not set; not modifying any shell profile"
  else
    case "$(basename "${SHELL:-sh}")" in
      zsh) PROFILES="$HOME/.zshrc" ;;
      bash)
        PROFILES="$HOME/.bashrc"
        if [ "$UNAME_S" = Darwin ]; then PROFILES="$PROFILES$NL$HOME/.bash_profile"; fi
        ;;
      fish) PROFILES="$HOME/.config/fish/conf.d/semaprax.fish" ;;
      *) PROFILES="$HOME/.profile" ;;
    esac
    PM_KIND="profile"
    PM_LOCATION="$(printf '%s\n' "$PROFILES" | head -1)"
    : >"$STAGE/profile.added"
    printf '%s\n' "$PROFILES" | while IFS= read -r pf; do
      [ -n "$pf" ] || continue
      mkdir -p "$(dirname "$pf")" || exit 1
      if [ -f "$pf" ]; then
        if ! grep -qF "$BLOCK_BEGIN" "$pf"; then printf '%s\n' "$pf" >>"$STAGE/profile.added"; fi
        strip_block 1 "" <"$pf" >"$STAGE/profile.new"
      else
        printf '%s\n' "$pf" >>"$STAGE/profile.added"
        : >"$STAGE/profile.new"
      fi
      block_text "$pf" "$PREFIX/bin" >>"$STAGE/profile.new"
      if [ -f "$pf" ] && cmp -s "$pf" "$STAGE/profile.new"; then
        continue
      fi
      cat "$STAGE/profile.new" >"$pf" || exit 1
      say "added PATH block to $pf"
    done
    pm_rc=$?
    # Roll back, on a later failure, only profiles that had no block before.
    PROFILE_ADDED="$(cat "$STAGE/profile.added")"
    [ "$pm_rc" = 0 ] || die "could not update the shell profile"
  fi
fi

# ---------------------------------------------------------------- receipt

{
  printf '{\n'
  printf '  "schema": "semaprax.install-receipt.v1",\n'
  printf '  "installer": "install.sh",\n'
  printf '  "version": "%s",\n' "$VERSION"
  printf '  "tag": "%s",\n' "$TAG"
  printf '  "target": "%s",\n' "$TARGET"
  printf '  "source": "%s",\n' "$(json_escape "$SOURCE_URL")"
  printf '  "archive_sha256": "%s",\n' "$ACTUAL_SHA"
  printf '  "publisher_verification": "%s",\n' "$PUBLISHER"
  printf '  "files": [\n'
  {
    (cd "$DEST" && find . -type f | sed 's|^\./||' | LC_ALL=C sort |
      while IFS= read -r f; do printf 'versions/%s/%s\n' "$TAG" "$f"; done)
    printf 'bin/semaprax\nbin/semapraxd\ncurrent\n'
  } | sed 's/^\(.*\)$/    "\1",/' | sed '$ s/,$//'
  printf '  ],\n'
  if [ "$PM_KIND" = profile ]; then
    printf '  "path_modification": {"kind": "profile", "location": "%s"}\n' "$(json_escape "$PM_LOCATION")"
  else
    printf '  "path_modification": {"kind": "none", "location": null}\n'
  fi
  printf '}\n'
} >"$STAGE/receipt.json" || die "could not write the install receipt"
mv "$STAGE/receipt.json" "$RECEIPT" || die "could not publish the install receipt"
STATE="done"

# ----------------------------------------------------------------- prune

for d in "$PREFIX"/versions/*; do
  [ -d "$d" ] || continue
  [ "$(basename "$d")" = "$TAG" ] && continue
  rm -rf "$d" || warn "could not remove old version $d"
done

# ------------------------------------------------------------------ done

if [ "$REUSE" = 1 ] && [ "$PREV_CURRENT" = "versions/$TAG" ]; then
  say "semaprax $TAG is already installed and active in $PREFIX"
else
  say "installed semaprax $TAG ($TARGET) in $PREFIX"
fi
say "publisher verification: $PUBLISHER"
say "executables: $PREFIX/bin/semaprax and $PREFIX/bin/semapraxd"
OTHER="$(command -v semaprax 2>/dev/null || :)"
if [ -n "$OTHER" ] && [ "$OTHER" != "$PREFIX/bin/semaprax" ]; then
  warn "another semaprax is first on PATH: $OTHER"
fi
if [ "$PM_KIND" = profile ]; then
  say "future terminals pick up PATH from $PM_LOCATION"
fi
say "to use semaprax in this terminal now, run:"
# shellcheck disable=SC2016  # print a literal $PATH for the user to paste
printf '  export PATH="%s/bin:$PATH"\n' "$PREFIX"
say "then try: semaprax --version"
exit 0
