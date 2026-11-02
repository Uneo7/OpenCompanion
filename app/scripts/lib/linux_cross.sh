# Sourced by scripts/check_all_targets.sh and scripts/package_app.sh: cross-link the Linux targets with zig
# (cargo-zigbuild) from any host. The link libraries (libudev, libasound) and their headers come from Debian 11
# packages into $SYSROOTS, so no -dev package is needed on the host; a tiny pkg-config shim points the -sys build
# scripts at them. Debian 11 = glibc 2.31: binaries linked against it run on any distribution with glibc >= 2.31.
# Needs: curl, tar, zig. The caller sets SYSROOTS (a folder under the cargo target dir).

DEBIAN=https://deb.debian.org/debian/pool/main
UDEV_VER=247.3-7+deb11u5
ALSA_VER=1.2.4-1.1
# glibc floor of release binaries (`cargo zigbuild --target <triple>.<glibc>`), same as the Debian 11 sysroot.
LINUX_GLIBC=2.31

case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) HOST_OS=windows ;;
    Darwin) HOST_OS=macos ;;
    *) HOST_OS=linux ;;
esac

native_path() { if [ $HOST_OS = windows ]; then cygpath -m "$1"; else printf '%s' "$1"; fi; }

fetch_sysroot() { # $1 = deb arch (amd64|arm64), $2 = multiarch dir (x86_64-linux-gnu|aarch64-linux-gnu)
    local dst=$SYSROOTS/$1 lib=$SYSROOTS/$1/usr/lib/$2
    [ -f "$lib/libudev.so" ] && [ -f "$lib/libasound.so" ] && return 0
    mkdir -p "$dst" "$SYSROOTS/debs"
    local p
    for p in "s/systemd/libudev1_${UDEV_VER}" "s/systemd/libudev-dev_${UDEV_VER}" \
             "a/alsa-lib/libasound2_${ALSA_VER}" "a/alsa-lib/libasound2-dev_${ALSA_VER}"; do
        local deb=$SYSROOTS/debs/$(basename "$p")_$1.deb
        [ -f "$deb" ] || curl -sfL -o "$deb" "$DEBIAN/${p}_$1.deb" || { echo "download failed: $p" >&2; return 1; }
        local x; x=$(mktemp -d)
        (cd "$x" && zig ar x "$deb" && tar -xf data.tar.* -C "$dst" 2>/dev/null)   # symlinks may fail on Windows
        rm -rf "$x"
    done
    # Recreate the dev symlinks as plain copies (portable to hosts without symlink support).
    # rm first: on hosts where tar did create them, they are symlinks to the very file being copied.
    rm -f "$lib/libudev.so" "$lib/libasound.so"
    cp "$lib"/libudev.so.1.*.* "$lib/libudev.so"   # real files; SONAME stays libudev.so.1
    cp "$lib"/libasound.so.2.*.* "$lib/libasound.so"
}

write_pkg_config_shim() {
    if [ $HOST_OS = windows ]; then
        PKG_SHIM=$SYSROOTS/pkg-config.cmd
        cat > "$PKG_SHIM" <<'EOF'
@echo off
setlocal
echo %* | findstr /c:"--modversion" >nul && (echo 1.0& exit /b 0)
set L=
echo %* | findstr /c:"libudev" >nul && set L=udev
echo %* | findstr /c:" alsa" >nul && set L=asound
if "%L%"=="" (echo pkg-config shim: unknown package in %* 1>&2& exit /b 1)
echo -L%OC_XLIBDIR% -l%L%
EOF
    else
        PKG_SHIM=$SYSROOTS/pkg-config
        cat > "$PKG_SHIM" <<'EOF'
#!/bin/sh
l=
for a; do
    case $a in
        --modversion) echo 1.0; exit 0 ;;
        libudev) l=udev ;;
        alsa) l=asound ;;
    esac
done
[ -n "$l" ] || { echo "pkg-config shim: unknown package in $*" >&2; exit 1; }
echo "-L$OC_XLIBDIR -l$l"
EOF
        chmod +x "$PKG_SHIM"
    fi
}

# linux_cross_env <rust triple>: fetches the sysroot and fills LINUX_CROSS_ENV, to run
# `env "${LINUX_CROSS_ENV[@]}" cargo zigbuild --target <triple>[.<glibc>] ...`.
linux_cross_env() {
    local t=$1 arch ma
    command -v zig >/dev/null 2>&1 || { echo "missing: zig (https://ziglang.org/download/)" >&2; return 2; }
    cargo zigbuild --help >/dev/null 2>&1 || { echo "missing: cargo-zigbuild (cargo install cargo-zigbuild)" >&2; return 2; }
    case $t in
        x86_64-*) arch=amd64 ma=x86_64-linux-gnu ;;
        aarch64-*) arch=arm64 ma=aarch64-linux-gnu ;;
        *) echo "unsupported Linux target: $t" >&2; return 1 ;;
    esac
    fetch_sysroot $arch $ma || return 1
    write_pkg_config_shim
    local sr pc
    sr=$(native_path "$SYSROOTS/$arch")
    if [ $HOST_OS = windows ]; then pc=$(cygpath -w "$PKG_SHIM"); else pc=$PKG_SHIM; fi
    LINUX_CROSS_ENV=("PKG_CONFIG=$pc" PKG_CONFIG_ALLOW_CROSS=1 "OC_XLIBDIR=$sr/usr/lib/$ma"
                     "CFLAGS_$(echo "$t" | tr '-' '_')=-I$sr/usr/include")
}
