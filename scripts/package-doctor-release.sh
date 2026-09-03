#!/usr/bin/env sh
set -eu
umask 077
LC_ALL=C
export LC_ALL

fail() {
    printf '%s\n' "doctor release package rejected: $1" >&2
    exit 2
}

# The script cannot hold or descriptor-execute these host utilities portably.
# Its caller therefore supplies a trusted shell and immutable absolute tool
# paths whose files and ancestor directories cannot change until completion.
# The checks below reject lexical indirection and accidental substitutions;
# they are not a proof of that external immutability precondition.
clean_env_tool=/usr/bin/env
mkdir_tool=/bin/mkdir
copy_tool=/bin/cp
chmod_tool=/bin/chmod
touch_tool=/usr/bin/touch
pwd_tool=/bin/pwd

require_tool() {
    admitted_tool=$1
    case "$admitted_tool" in /*) ;; *) fail "tool path must be absolute" ;; esac
    [ -f "$admitted_tool" ] && [ ! -L "$admitted_tool" ] && [ -x "$admitted_tool" ] \
        || fail "tool must be one physical executable"
}

for fixed_tool in \
    "$clean_env_tool" "$mkdir_tool" "$copy_tool" "$chmod_tool" "$touch_tool" "$pwd_tool"
do
    require_tool "$fixed_tool"
done

# Clear shell-startup and loader controls before the first external invocation.
# Each invoked utility then receives only this closed, deterministic block.
unset BASH_ENV CDPATH ENV GZIP GZIP_OPT LD_AUDIT LD_LIBRARY_PATH LD_PRELOAD \
    LIBARCHIVE HOME PATH TAR_OPTIONS TAPE TMPDIR TZ \
    DYLD_FRAMEWORK_PATH DYLD_FALLBACK_FRAMEWORK_PATH DYLD_FALLBACK_LIBRARY_PATH \
    DYLD_INSERT_LIBRARIES DYLD_LIBRARY_PATH DYLD_PRINT_LIBRARIES \
    DYLD_PRINT_LIBRARIES_POST_LAUNCH DYLD_PRINT_RPATHS

run_closed() {
    "$clean_env_tool" -i LC_ALL=C TZ=UTC0 "$@"
}

run_archive_closed() {
    "$clean_env_tool" -i LC_ALL=C TZ=UTC0 COPYFILE_DISABLE=1 "$@"
}

[ "$#" -eq 17 ] || fail "expected RELEASE_TOOL TAR_TOOL GZIP_TOOL TAG COMMIT TARGET SELECTOR PROFILE PUBLIC_KEY SIGNING_KEY REQUEST BUNDLE LAUNCHER WORKER COLLECTOR PROVISIONER OUTPUT_ROOT"
tool=$1
tar_tool=$2
gzip_tool=$3
tag=$4
commit=$5
target=$6
selector=$7
profile=$8
public_key=$9
shift 9
signing_key=$1
request=$2
shift 2
bundle=$1
launcher=$2
worker=$3
collector=$4
provisioner=$5
output_root=$6

require_tool "$tool"
for archive_tool in "$tar_tool" "$gzip_tool"; do
    require_tool "$archive_tool"
done
case "$tag" in v[0-9]*) ;; *) fail "tag must begin with v and a decimal version" ;; esac
version=${tag#v}
[ "${#version}" -le 64 ] || fail "release version is too long"
case "$version" in [0-9]*) ;; *) fail "release version is not canonical" ;; esac
case "$version" in *[!A-Za-z0-9.-]*) fail "release version is not canonical" ;; esac
case "$version" in *[!A-Za-z0-9]) fail "release version is not canonical" ;; esac
[ "${#commit}" -eq 40 ] || fail "commit must be exactly 40 lowercase hexadecimal characters"
case "$commit" in *[!0-9a-f]*) fail "commit must be exactly 40 lowercase hexadecimal characters" ;; esac
case "$target" in
    x86_64-unknown-linux-musl) architecture=x86_64 ;;
    aarch64-unknown-linux-musl) architecture=aarch64 ;;
    *) fail "target must be an admitted static Linux target" ;;
esac
case "$profile" in contributor|native|web|all) ;; *) fail "profile is unsupported" ;; esac
case "$selector" in
    [a-z]* ) ;;
    * ) fail "selector is not canonical" ;;
esac
[ "${#selector}" -le 64 ] || fail "selector is too long"
case "$selector" in *[!a-z0-9-]*) fail "selector is not canonical" ;; esac
[ "${#public_key}" -eq 64 ] || fail "public key must be canonical lowercase hexadecimal"
case "$public_key" in *[!0-9a-f]*) fail "public key must be canonical lowercase hexadecimal" ;; esac

for input in "$signing_key" "$request" "$bundle" "$launcher" "$worker" "$collector" "$provisioner" "$output_root"; do
    case "$input" in /*) ;; *) fail "every input and output path must be absolute" ;; esac
done
[ -d "$output_root" ] && [ ! -L "$output_root" ] || fail "output root must be one physical directory"
# Portable pathname operations cannot hold the output ancestry. The caller
# guarantees this directory and every ancestor are trusted, immutable and
# quiescent until completion; exclusive creation is defense in depth, not that
# authority proof.
output_root=$(CDPATH= cd -P "$output_root" && run_closed "$pwd_tool" -P) \
    || fail "output root cannot be resolved"
package_name="semaprax-doctor-$tag-$target"
package_root="$output_root/$package_name"
archive="$output_root/$package_name.tar.gz"
tar_archive="$output_root/$package_name.tar"
verify_root="$output_root/verify-$target"
verify_tar="$output_root/verify-$target.tar"
for output in "$package_root" "$tar_archive" "$archive" "$verify_root" "$verify_tar"; do
    [ ! -e "$output" ] && [ ! -L "$output" ] || fail "output path already exists"
done
run_closed "$mkdir_tool" "$package_root" || fail "package staging directory cannot be created"

run_closed "$tool" create \
    --request "$request" \
    --bundle "$bundle" \
    --launcher "$launcher" \
    --worker "$worker" \
    --collector "$collector" \
    --provisioner "$provisioner" \
    --selector "$selector" \
    --architecture "$architecture" \
    --target "$profile" \
    --release-version "$version" \
    --release-commit "$commit" \
    --target-triple "$target" \
    --signing-key "$signing_key" \
    --output-directory "$package_root" || fail "signed release construction failed"

copy_artifact() {
    source=$1
    destination=$2
    [ ! -e "$destination" ] && [ ! -L "$destination" ] || fail "artifact destination already exists"
    run_closed "$copy_tool" -p "$source" "$destination" || fail "artifact copy failed"
}
copy_artifact "$request" "$package_root/semaprax-doctor-request.bin"
copy_artifact "$bundle" "$package_root/semaprax-doctor-bundle.bin"
copy_artifact "$launcher" "$package_root/semaprax-doctor-launcher"
copy_artifact "$worker" "$package_root/semaprax-doctor-worker"
copy_artifact "$collector" "$package_root/semaprax-doctor-collector"
copy_artifact "$provisioner" "$package_root/semaprax-doctor-provisioner"

# Canonical archive metadata is release data, not inherited input metadata.
# The signed verifier replays file bytes; these fixed modes and timestamps make
# two archives of the same exact inputs byte-identical across release users.
run_closed "$chmod_tool" 700 "$package_root" || fail "package directory mode cannot be fixed"
run_closed "$chmod_tool" 600 \
    "$package_root/semaprax-doctor-bundle.bin" \
    "$package_root/semaprax-doctor-release-manifest.json" \
    "$package_root/semaprax-doctor-release-manifest.sig" \
    "$package_root/semaprax-doctor-release.capsule" \
    "$package_root/semaprax-doctor-request.bin" || fail "data artifact modes cannot be fixed"
run_closed "$chmod_tool" 500 \
    "$package_root/semaprax-doctor-collector" \
    "$package_root/semaprax-doctor-launcher" \
    "$package_root/semaprax-doctor-provisioner" \
    "$package_root/semaprax-doctor-worker" || fail "executable artifact modes cannot be fixed"
run_closed "$touch_tool" -t 200001010000.00 \
    "$package_root" \
    "$package_root/semaprax-doctor-bundle.bin" \
    "$package_root/semaprax-doctor-collector" \
    "$package_root/semaprax-doctor-launcher" \
    "$package_root/semaprax-doctor-provisioner" \
    "$package_root/semaprax-doctor-release-manifest.json" \
    "$package_root/semaprax-doctor-release-manifest.sig" \
    "$package_root/semaprax-doctor-release.capsule" \
    "$package_root/semaprax-doctor-request.bin" \
    "$package_root/semaprax-doctor-worker" || fail "archive timestamps cannot be fixed"

verify_directory() {
    directory=$1
    run_closed "$tool" verify \
        --directory "$directory" \
        --public-key-hex "$public_key" \
        --release-version "$version" \
        --release-commit "$commit" \
        --target-triple "$target" \
        --architecture "$architecture" \
        --target "$profile" \
        --selector "$selector"
}
verify_directory "$package_root" || fail "staged distribution replay failed"

set -C
# `--no-recursion` plus the explicit sorted inventory prevents filesystem
# enumeration order from entering the archive. ustar excludes ambient xattrs;
# numeric ownership and gzip -n exclude release-user and wall-clock metadata.
run_archive_closed "$tar_tool" \
    --format ustar \
    --uid 0 --gid 0 --uname root --gname root \
    --no-recursion \
    -cf - -C "$output_root" \
    "$package_name" \
    "$package_name/semaprax-doctor-bundle.bin" \
    "$package_name/semaprax-doctor-collector" \
    "$package_name/semaprax-doctor-launcher" \
    "$package_name/semaprax-doctor-provisioner" \
    "$package_name/semaprax-doctor-release-manifest.json" \
    "$package_name/semaprax-doctor-release-manifest.sig" \
    "$package_name/semaprax-doctor-release.capsule" \
    "$package_name/semaprax-doctor-request.bin" \
    "$package_name/semaprax-doctor-worker" > "$tar_archive" || fail "canonical tar creation failed"
run_closed "$gzip_tool" -n "$tar_archive" || fail "deterministic compression failed"
[ -f "$archive" ] && [ ! -L "$archive" ] || fail "deterministic archive is unavailable"
run_closed "$mkdir_tool" "$verify_root" || fail "verification root cannot be created"
run_closed "$gzip_tool" -dc "$archive" > "$verify_tar" \
    || fail "archive decompression failed"
run_archive_closed "$tar_tool" -xf "$verify_tar" -C "$verify_root" \
    || fail "archive extraction failed"
verify_directory "$verify_root/$package_name" || fail "unpacked distribution replay failed"
printf '%s\n' "$archive"
