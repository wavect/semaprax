#!/usr/bin/env sh
set -eu
umask 077

fail() {
    echo "doctor release package rejected: $1" >&2
    exit 2
}

[ "$#" -eq 15 ] || fail "expected TOOL TAG COMMIT TARGET SELECTOR PROFILE PUBLIC_KEY SIGNING_KEY REQUEST BUNDLE LAUNCHER WORKER COLLECTOR PROVISIONER OUTPUT_ROOT"
tool=$1
tag=$2
commit=$3
target=$4
selector=$5
profile=$6
public_key=$7
signing_key=$8
request=$9
shift 9
bundle=$1
launcher=$2
worker=$3
collector=$4
provisioner=$5
output_root=$6

case "$tool" in /*) ;; *) fail "release tool path must be absolute" ;; esac
[ -f "$tool" ] && [ ! -L "$tool" ] && [ -x "$tool" ] || fail "release tool must be one physical executable"
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
output_root=$(CDPATH= cd -P "$output_root" && pwd -P) || fail "output root cannot be resolved"
package_name="semaprax-doctor-$tag-$target"
package_root="$output_root/$package_name"
archive="$output_root/$package_name.tar.gz"
verify_root="$output_root/verify-$target"
for output in "$package_root" "$archive" "$verify_root"; do
    [ ! -e "$output" ] && [ ! -L "$output" ] || fail "output path already exists"
done
mkdir "$package_root" || fail "package staging directory cannot be created"

"$tool" create \
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
    cp -p "$source" "$destination" || fail "artifact copy failed"
}
copy_artifact "$request" "$package_root/semaprax-doctor-request.bin"
copy_artifact "$bundle" "$package_root/semaprax-doctor-bundle.bin"
copy_artifact "$launcher" "$package_root/semaprax-doctor-launcher"
copy_artifact "$worker" "$package_root/semaprax-doctor-worker"
copy_artifact "$collector" "$package_root/semaprax-doctor-collector"
copy_artifact "$provisioner" "$package_root/semaprax-doctor-provisioner"

verify_directory() {
    directory=$1
    "$tool" verify \
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
tar -czf - -C "$output_root" "$package_name" > "$archive" || fail "archive creation failed"
mkdir "$verify_root" || fail "verification root cannot be created"
tar -xzf "$archive" -C "$verify_root" || fail "archive extraction failed"
verify_directory "$verify_root/$package_name" || fail "unpacked distribution replay failed"
printf '%s\n' "$archive"
