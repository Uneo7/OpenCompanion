# Sourced by scripts/package_app.sh: AppImage, `oc` archive and udev rule for one Linux architecture.
#
# The binary is self-contained apart from what every desktop has: glibc >= 2.31, libudev.so.1, libasound.so.2
# (linked); X11 / Wayland / GL / Vulkan are loaded at run time (dlopen). Nothing else goes into the AppDir.

APPIMAGE_RELEASES=https://github.com/AppImage
APPIMAGE_TOOLS=$CARGO_TARGET_DIR/tools

appimagetool_path() {
    if [ -n "${APPIMAGETOOL:-}" ]; then echo "$APPIMAGETOOL"; return; fi
    local f=$APPIMAGE_TOOLS/appimagetool-$(uname -m).AppImage
    if [ ! -x "$f" ]; then
        mkdir -p "$APPIMAGE_TOOLS"
        curl -sfL -o "$f" "$APPIMAGE_RELEASES/appimagetool/releases/download/continuous/appimagetool-$(uname -m).AppImage"
        chmod +x "$f"
    fi
    echo "$f"
}

# The AppImage runtime of the *target* architecture (appimagetool runs on the host's).
appimage_runtime() {
    local f=$APPIMAGE_TOOLS/runtime-$1
    if [ ! -f "$f" ]; then
        mkdir -p "$APPIMAGE_TOOLS"
        curl -sfL -o "$f" "$APPIMAGE_RELEASES/type2-runtime/releases/download/continuous/runtime-$1"
    fi
    echo "$f"
}

# make_appdir <binary> <AppDir>
make_appdir() {
    local d=$2
    rm -rf "$d"
    mkdir -p "$d/usr/bin" "$d/usr/share/applications" "$d/usr/share/icons" "$d/usr/share/doc/opencompanion"
    cp "$1" "$d/usr/bin/opencompanion"
    chmod +x "$d/usr/bin/opencompanion"
    cp packaging/linux/opencompanion.desktop "$d/opencompanion.desktop"
    cp packaging/linux/opencompanion.desktop "$d/usr/share/applications/"
    cp -R packaging/linux/hicolor "$d/usr/share/icons/"
    cp packaging/linux/hicolor/256x256/apps/opencompanion.png "$d/opencompanion.png"
    cp "$ROOT/../LICENSE" packaging/linux/70-opencompanion.rules "$d/usr/share/doc/opencompanion/"
    cp -R "$ROOT/../licenses" "$d/usr/share/doc/opencompanion/"
    ln -sf opencompanion.png "$d/.DirIcon"
    ln -sf usr/bin/opencompanion "$d/AppRun"
}

package_linux() {
    local arch=$1 triple=$1-unknown-linux-gnu
    ensure_target "$triple"
    linux_cross_env "$triple"
    cargo_release zigbuild "$triple.$LINUX_GLIBC" "${LINUX_CROSS_ENV[@]}"
    local bin=$CARGO_TARGET_DIR/$triple/release appdir=$STAGE/linux-$arch/AppDir

    package_cli "linux-$arch" "$bin/oc"
    cp packaging/linux/70-opencompanion.rules "$OUT/"
    checksum "$OUT/70-opencompanion.rules"

    make_appdir "$bin/opencompanion" "$appdir"
    if [ "$HOST_OS" != linux ]; then
        echo "  AppImage skipped: appimagetool needs a Linux host (AppDir ready in $appdir)"
        return 0
    fi
    local out=$OUT/OpenCompanion-$VERSION-linux-$arch.AppImage tool runtime
    tool=$(appimagetool_path)
    runtime=$(appimage_runtime "$arch")
    # APPIMAGE_EXTRACT_AND_RUN: no FUSE needed to run appimagetool itself (CI containers).
    ARCH=$arch APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream --runtime-file "$runtime" "$appdir" "$out"
    chmod +x "$out"
    checksum "$out"
}
