# Sourced by scripts/package_app.sh: release builds, checksums, the `oc` archive.
# Uses: ROOT, OUT, STAGE, VERSION, CARGO_TARGET_DIR.

ensure_target() { rustup target list --installed | grep -qx "$1" || rustup target add "$1"; }

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# checksum <file>: writes <file>.sha256 next to it, in the `sha256sum -c` format.
checksum() {
    local dir name
    dir=$(dirname "$1") name=$(basename "$1")
    (cd "$dir" && printf '%s  %s\n' "$(sha256_of "$name")" "$name" > "$name.sha256")
    echo "  $1"
}

# cargo_release <build|zigbuild> <--target value> [VAR=value ...]: the production app (firmware from the GitHub
# release) and the CLI, in two invocations so that the app's `release-updates` feature does not leak into `oc`.
cargo_release() {
    local cmd=$1 target=$2
    shift 2
    env "$@" cargo "$cmd" --release --locked -p oc-app --features release-updates --target "$target"
    env "$@" cargo "$cmd" --release --locked -p oc-cli --target "$target"
}

# package_cli <label> <oc binary>: oc-<ver>-<label>.tar.gz (the command-line tool used by docs/recovery.md).
package_cli() {
    local name=oc-$VERSION-$1 dir=$STAGE/cli
    rm -rf "${dir:?}/$name"
    mkdir -p "$dir/$name"
    cp "$2" "$dir/$name/oc"
    chmod +x "$dir/$name/oc"
    cp "$ROOT/../LICENSE" "$dir/$name/"
    COPYFILE_DISABLE=1 tar -czf "$OUT/$name.tar.gz" -C "$dir" "$name"
    checksum "$OUT/$name.tar.gz"
}
