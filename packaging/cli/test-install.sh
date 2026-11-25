#!/bin/sh
set -eu

cd "$(dirname "$0")"
work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' 0
trap 'exit 1' HUP INT TERM
mkdir "$work_dir/mock" "$work_dir/assets" "$work_dir/tmp" "$work_dir/home"
export TMPDIR="$work_dir/tmp" HOME="$work_dir/home"
export UNOSKILLS_INSTALL_DIR="$work_dir/install dir"
export TEST_ASSETS="$work_dir/assets"
export PATH="$work_dir/mock:$PATH"
sed 's/@RELEASE_TAG@/v1.2.3/' install.sh > "$work_dir/install.sh"
sh -n "$work_dir/install.sh"

cat > "$work_dir/mock/uname" <<'SH'
#!/bin/sh
case "$1" in
    -s) printf '%s\n' "${TEST_OS:-Linux}" ;;
    -m) printf '%s\n' "${TEST_ARCH:-x86_64}" ;;
esac
SH
cat > "$work_dir/mock/getconf" <<'SH'
#!/bin/sh
printf '%s\n' "${TEST_LIBC-glibc 2.39}"
SH
cat > "$work_dir/mock/sw_vers" <<'SH'
#!/bin/sh
printf '%s\n' "${TEST_MACOS:-14.0}"
SH
cat > "$work_dir/mock/curl" <<'SH'
#!/bin/sh
set -eu
while [ "$#" -gt 1 ]; do
    if [ "$1" = --output ]; then
        output=$2
        shift
    fi
    shift
done
case "$1" in
    https://github.com/codexlin/UnoSkills/releases/download/v1.2.3/*)
        cp "$TEST_ASSETS/${1##*/}" "$output" ;;
    *) exit 1 ;;
esac
SH
chmod +x "$work_dir/mock/"*

asset="$TEST_ASSETS/unoskills-linux-x64"
manifest="$TEST_ASSETS/SHA256SUMS.txt"
cat > "$asset" <<'SH'
#!/bin/sh
printf '%s\n' 'unoskills 1.2.3'
SH

write_checksum() {
    if command -v sha256sum >/dev/null 2>&1; then
        digest=$(sha256sum < "$1")
    else
        digest=$(shasum -a 256 < "$1")
    fi
    printf '%s  %s\n' "${digest%% *}" "${1##*/}" > "$manifest"
}

write_checksum "$asset"
sh "$work_dir/install.sh"
target="$UNOSKILLS_INSTALL_DIR/unoskills"
cmp "$asset" "$target"
cp "$target" "$work_dir/previous"

expect_failure() {
    diagnostic=$1
    shift
    if env "$@" sh "$work_dir/install.sh" > "$work_dir/output" 2>&1; then
        printf 'Expected failure with %s\n' "$diagnostic" >&2
        exit 1
    fi
    if ! grep -F "$diagnostic" "$work_dir/output"; then
        cat "$work_dir/output" >&2
        exit 1
    fi
    cmp "$work_dir/previous" "$target"
    test -x "$target"
    test -z "$(find "$TMPDIR" "$UNOSKILLS_INSTALL_DIR" -type d -name '*unoskills.*' -print)"
}

expect_failure 'Unsupported platform: Darwin x86_64' TEST_OS=Darwin
expect_failure 'Unsupported platform: Linux aarch64' TEST_ARCH=aarch64
expect_failure 'Unsupported platform: FreeBSD x86_64' TEST_OS=FreeBSD
expect_failure 'macOS 14 or later is required' TEST_OS=Darwin TEST_ARCH=arm64 TEST_MACOS=13.6.9
expect_failure 'Linux requires glibc 2.39 or later' TEST_LIBC='glibc 2.38'
expect_failure 'Could not detect glibc' TEST_LIBC=

printf '%s\n' 'bad-hash  unoskills-linux-x64' > "$manifest"
expect_failure 'Expected exactly one valid SHA-256 entry'
write_checksum "$asset"
sed 's/$/.tar.gz/' "$manifest" > "$manifest.other"
mv "$manifest.other" "$manifest"
expect_failure 'Expected exactly one valid SHA-256 entry'
write_checksum "$asset"
cp "$manifest" "$manifest.other"
cat "$manifest.other" >> "$manifest"
expect_failure 'Expected exactly one valid SHA-256 entry'

write_checksum "$asset"
cat > "$asset" <<'SH'
#!/bin/sh
printf '%s\n' 'error while loading shared libraries: libssl.so.3' >&2
exit 127
SH
expect_failure 'SHA-256 checksum mismatch'
write_checksum "$asset"
expect_failure 'Check the loader error above and install the missing runtime libraries'
grep -F 'error while loading shared libraries: libssl.so.3' "$work_dir/output"
