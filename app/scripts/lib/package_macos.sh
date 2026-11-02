# Sourced by scripts/package_app.sh: OpenCompanion.app (universal), zip + dmg, and the `oc` archive. Mac only.
#
# Signing: ad-hoc only (`codesign --sign -`: no identity, no notarization). Apple Silicon refuses to run code
# without any signature, and lipo output plus the bundle need a fresh one. Gatekeeper still blocks the first
# launch of a downloaded copy: README.md, "First run of an unsigned build". A Developer ID signature and
# notarization (`codesign --options runtime --sign "Developer ID Application: …"`, `xcrun notarytool submit`,
# `xcrun stapler staple`) can replace the ad-hoc step later without other changes.

package_macos() {
    if [ "$HOST_OS" != macos ]; then
        echo "macOS packages need a Mac (Apple SDK, lipo, codesign, hdiutil); on other hosts use check_all_targets" >&2
        return 1
    fi
    local t b st=$STAGE/macos name=OpenCompanion-$VERSION-macos-universal
    for t in aarch64-apple-darwin x86_64-apple-darwin; do
        ensure_target "$t"
        cargo_release build "$t"
    done
    rm -rf "$st"
    mkdir -p "$st/dmg"
    for b in opencompanion oc; do
        lipo -create -output "$st/$b" \
            "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/$b" "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/$b"
    done
    sh packaging/macos/bundle.sh "$st/opencompanion" "$st" >/dev/null
    codesign --force --sign - "$st/OpenCompanion.app"
    codesign --verify --strict --verbose=2 "$st/OpenCompanion.app"
    codesign --force --sign - "$st/oc"

    ditto -c -k --keepParent "$st/OpenCompanion.app" "$OUT/$name.zip"
    checksum "$OUT/$name.zip"

    cp -R "$st/OpenCompanion.app" "$st/dmg/"
    ln -s /Applications "$st/dmg/Applications"
    # hdiutil is known to fail now and then on CI runners ("Resource busy"): retry.
    local i
    for i in 1 2 3; do
        hdiutil create -volname OpenCompanion -srcfolder "$st/dmg" -ov -format UDZO "$OUT/$name.dmg" && break
        [ $i -eq 3 ] && return 1
        sleep 5
    done
    checksum "$OUT/$name.dmg"

    package_cli macos-universal "$st/oc"
}
