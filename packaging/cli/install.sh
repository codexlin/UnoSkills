#!/bin/sh
set -eu

release_tag='@RELEASE_TAG@'
export LC_ALL=C

fail() {
    printf 'UnoSkills installer: %s\n' "$*" >&2
    exit 1
}

version_at_least() {
    awk -v version="$1" -v major="$2" -v minor="$3" 'BEGIN {
        if (version !~ /^[0-9]+\.[0-9]+(\.[0-9]+)*$/) exit 1
        split(version, parts, ".")
        exit !(parts[1] > major || (parts[1] == major && parts[2] >= minor))
    }'
}

awk -v tag="$release_tag" 'BEGIN { exit !(tag ~ /^v[0-9]+\.[0-9]+\.[0-9]+$/) }' ||
    fail 'Expected a pinned stable release tag.'

platform=$(uname -s)
arch=$(uname -m)
case "$platform:$arch" in
    Darwin:arm64)
        asset=unoskills-macos-arm64
        macos=$(sw_vers -productVersion) || fail 'Could not determine the macOS version.'
        version_at_least "$macos" 14 0 || fail "macOS 14 or later is required (found $macos)."
        ;;
    Linux:x86_64)
        asset=unoskills-linux-x64
        libc=$(getconf GNU_LIBC_VERSION 2>/dev/null) || libc=
        case "$libc" in
            'glibc '*)
                version_at_least "${libc#glibc }" 2 39 || fail "Linux requires glibc 2.39 or later (found $libc)."
                ;;
            *) fail 'Linux requires glibc 2.39 or later. Could not detect glibc.' ;;
        esac
        ;;
    *) fail "Unsupported platform: $platform $arch. Choose Apple Silicon macOS or x86_64 Linux." ;;
esac

command -v curl >/dev/null 2>&1 || fail 'curl is required.'
install_dir=${UNOSKILLS_INSTALL_DIR:-${HOME:?Set HOME or UNOSKILLS_INSTALL_DIR}/.local/bin}
case "$install_dir" in
    /*) ;;
    *) fail 'UNOSKILLS_INSTALL_DIR must be an absolute path.' ;;
esac

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/unoskills.XXXXXX") || fail 'Could not create a download directory.'
stage_dir=
cleanup() {
    rm -rf "$work_dir"
    if [ -n "$stage_dir" ]; then
        rm -rf "$stage_dir"
    fi
}
trap cleanup 0
trap 'exit 1' HUP INT TERM

base_url="https://github.com/codexlin/UnoSkills/releases/download/$release_tag"
download() {
    curl -q --proto '=https' --proto-redir '=https' -fsSL --retry 3 \
        --output "$work_dir/$1" "$base_url/$1" || fail "Could not download $1 for $release_tag."
}

download SHA256SUMS.txt
expected=$(awk -v asset="$asset" '
    $2 == asset || $2 == "*" asset {
        count++
        if (NF == 2 && length($1) == 64 && $1 !~ /[^[:xdigit:]]/) digest = tolower($1)
    }
    END {
        if (count != 1 || digest == "") exit 1
        print digest
    }
' "$work_dir/SHA256SUMS.txt") || fail "Expected exactly one valid SHA-256 entry for $asset."

download "$asset"
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum < "$work_dir/$asset") || fail 'Could not calculate SHA-256.'
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 < "$work_dir/$asset") || fail 'Could not calculate SHA-256.'
else
    fail 'sha256sum or shasum is required.'
fi
[ "${actual%% *}" = "$expected" ] || fail "SHA-256 checksum mismatch for $asset."

mkdir -p "$install_dir" || fail "Could not create $install_dir."
target="$install_dir/unoskills"
[ ! -d "$target" ] || fail "$target is a directory. Choose a different installation directory."
# Stage beside the destination so the final rename stays on one filesystem.
stage_dir=$(mktemp -d "$install_dir/.unoskills.XXXXXX") || fail 'Could not create an installation staging directory.'
install -m 755 "$work_dir/$asset" "$stage_dir/unoskills" || fail 'Could not stage the CLI.'
if ! version=$(HOME="$work_dir" "$stage_dir/unoskills" --version); then
    if [ "$platform" = Linux ]; then
        fail 'UnoSkills could not start. Check the loader error above and install the missing runtime libraries (including libssl.so.3 and libcrypto.so.3 from OpenSSL 3).'
    fi
    fail 'UnoSkills could not start. See the error above.'
fi
[ "$version" = "unoskills ${release_tag#v}" ] || fail "Unexpected CLI version: $version."

mv -f "$stage_dir/unoskills" "$target" || fail "Could not install to $target."
printf 'Installed %s to %s\n' "$version" "$target"
case ":${PATH:-}:" in
    *":$install_dir:"*) ;;
    *) printf 'Add %s to PATH in your shell configuration.\n' "$install_dir" ;;
esac
