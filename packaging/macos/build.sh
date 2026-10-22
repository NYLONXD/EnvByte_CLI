#!/bin/sh
# Builds envbyte.pkg, the macOS installer, from the two release binaries.
#
#   sh packaging/macos/build.sh 0.5.0 <x86_64 envbyte> <aarch64 envbyte> envbyte.pkg
#
# The package carries one universal binary, so Apple silicon and Intel Macs get
# the same file, and installs it as /usr/local/bin/envbyte, which every Mac
# already has on its PATH. macOS only: it needs lipo, pkgbuild and productbuild.
#
# Signing and notarization switch on when these are set; without them the
# package is unsigned, and Gatekeeper makes the user allow it in System
# Settings > Privacy & Security before it will open:
#   MACOS_APP_IDENTITY        "Developer ID Application: <name> (<team id>)"
#   MACOS_INSTALLER_IDENTITY  "Developer ID Installer: <name> (<team id>)"
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD   for notarytool

set -eu

IDENTIFIER="in.trackedge.envbyte"

[ $# -eq 4 ] || {
    echo "usage: $0 <version> <x86_64 binary> <aarch64 binary> <output.pkg>" >&2
    exit 2
}
version=${1#v}
x86_binary=$2
arm_binary=$3
out=$4
here=$(cd "$(dirname "$0")" && pwd)

# Installer versions are numbers only: 0.5.0-rc.1 installs as 0.5.0.
pkg_version=${version%%[-+]*}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/root/usr/local/bin"
binary="$work/root/usr/local/bin/envbyte"
lipo -create -output "$binary" "$x86_binary" "$arm_binary"
chmod 755 "$binary"

if [ -n "${MACOS_APP_IDENTITY:-}" ]; then
    # Hardened runtime and a secure timestamp are what notarization checks.
    codesign --force --options runtime --timestamp \
        --sign "$MACOS_APP_IDENTITY" "$binary"
fi

pkgbuild --root "$work/root" \
    --identifier "$IDENTIFIER" \
    --version "$pkg_version" \
    --install-location / \
    "$work/envbyte-component.pkg"

sed "s/@VERSION@/$pkg_version/g" "$here/distribution.xml" >"$work/distribution.xml"

if [ -n "${MACOS_INSTALLER_IDENTITY:-}" ]; then
    productbuild --distribution "$work/distribution.xml" \
        --resources "$here/resources" \
        --package-path "$work" \
        --sign "$MACOS_INSTALLER_IDENTITY" --timestamp \
        "$out"
else
    productbuild --distribution "$work/distribution.xml" \
        --resources "$here/resources" \
        --package-path "$work" \
        "$out"
fi

if [ -n "${APPLE_ID:-}" ]; then
    xcrun notarytool submit "$out" \
        --apple-id "$APPLE_ID" \
        --team-id "$APPLE_TEAM_ID" \
        --password "$APPLE_APP_PASSWORD" \
        --wait
    # Staples the ticket into the package, so it opens without a network
    # check on the user's machine.
    xcrun stapler staple "$out"
fi

echo "Built $out (Envbyte $pkg_version)"
