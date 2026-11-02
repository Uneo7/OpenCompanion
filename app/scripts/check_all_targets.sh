#!/usr/bin/env bash
# Needs: rustup targets (added on demand), zig >= 0.13 and cargo-zigbuild (`cargo install cargo-zigbuild`),
# curl and tar. The Linux link libraries (libudev, libasound) and their headers are taken from Debian 11
# packages into target/cross-sysroot, so no -dev package is needed on the host; a tiny pkg-config shim
# points the -sys build scripts at them (scripts/lib/linux_cross.sh).
# Usage: scripts/check_all_targets.sh [--release] [--no-host] [target ...]   (default: all of the above)
set -u

cd "$(dirname "$0")/.."
ROOT=$PWD
TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}
SYSROOTS=$TARGET_DIR/cross-sysroot
. scripts/lib/linux_cross.sh

PROFILE=
HOST=1
TARGETS=()
for a in "$@"; do
    case $a in
        --release) PROFILE=--release ;;
        --no-host) HOST=0 ;;
        *) TARGETS+=("$a") ;;
    esac
done
[ ${#TARGETS[@]} -eq 0 ] && TARGETS=(x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin)

# Parallel arrays (macOS ships bash 3.2: no associative arrays).
NAMES=()
RESULTS=()
record() { NAMES+=("$1"); RESULTS+=("$2"); }

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing: $1 ($2)" >&2; exit 2; }; }
need cargo "https://rustup.rs"
need rustup "https://rustup.rs"

ensure_target() { rustup target list --installed | grep -qx "$1" || rustup target add "$1"; }

if [ $HOST = 1 ]; then
    echo "=== host: build + test"
    if cargo build --workspace --all-targets $PROFILE && cargo test --workspace $PROFILE; then
        record host "build+test OK"
    else
        record host FAILED
    fi
fi

for t in "${TARGETS[@]}"; do
    echo "=== $t"
    ensure_target "$t"
    case $t in
        *-linux-gnu)
            linux_cross_env "$t"; rc=$?
            [ $rc -eq 2 ] && exit 2
            [ $rc -ne 0 ] && { record "$t" "FAILED (sysroot)"; continue; }
            if env "${LINUX_CROSS_ENV[@]}" cargo zigbuild --workspace --all-targets --target "$t" $PROFILE; then
                record "$t" "build OK (linked)"
            else
                record "$t" FAILED
            fi
            ;;
        *-apple-darwin)
            if [ $HOST_OS = macos ]; then
                cargo build --workspace --all-targets --target "$t" $PROFILE && record "$t" "build OK (linked)" || record "$t" FAILED
            elif [ -n "${SDKROOT:-}" ] && command -v zig >/dev/null; then
                cargo zigbuild --workspace --all-targets --target "$t" $PROFILE && record "$t" "build OK (zig + SDKROOT)" || record "$t" FAILED
            else
                # No macOS SDK: type-check everything; hidapi's build script (mac/hid.c, IOKit headers) is replaced
                # by its single output that matters to the Rust side.
                if cargo check --workspace --all-targets --target "$t" $PROFILE \
                       --config "target.$t.hidapi.rustc-cfg=['hidapi']"; then
                    record "$t" "check OK (no SDK: hidapi C and link not verified)"
                else
                    record "$t" FAILED
                fi
            fi
            ;;
        *)
            cargo build --workspace --all-targets --target "$t" $PROFILE && record "$t" "build OK" || record "$t" FAILED
            ;;
    esac
done

echo
echo "=== summary"
fail=0
for i in "${!NAMES[@]}"; do
    printf '  %-28s %s\n' "${NAMES[$i]}" "${RESULTS[$i]}"
    case ${RESULTS[$i]} in FAILED*) fail=1 ;; esac
done
exit $fail
