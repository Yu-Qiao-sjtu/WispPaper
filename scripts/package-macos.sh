#!/bin/sh
# Put the release build into "Wisp Paper.app" and zip it:
#
#     scripts/package-macos.sh <target triple> <version> <label>
#
# The bundle is not signed with a developer certificate and not notarised.
# It gets an ad-hoc signature only, which Apple silicon needs to run it at all.
set -eu
target=$1
version=$2
label=$3
release="target/$target/release"
name="WispPaper-v$version-$label"
app="dist/Wisp Paper.app"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$release/wisp_paper" "$app/Contents/MacOS/wisp_paper"
# The command line travels inside the bundle: "Wisp Paper.app/Contents/MacOS/wisp".
cp "$release/wisp" "$app/Contents/MacOS/wisp"
cp LICENSE THIRD_PARTY.md "$app/Contents/Resources/"

# The icon in the sizes an .icns holds, from the one PNG.
iconset="dist/app-icon.iconset"
rm -rf "$iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/branding/app-icon.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" assets/branding/app-icon.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/app-icon.icns"
rm -rf "$iconset"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Wisp Paper</string>
    <key>CFBundleDisplayName</key><string>Wisp Paper</string>
    <key>CFBundleIdentifier</key><string>com.wisp.paper</string>
    <key>CFBundleExecutable</key><string>wisp_paper</string>
    <key>CFBundleIconFile</key><string>app-icon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$app"
ditto -c -k --keepParent "$app" "dist/$name.zip"
echo "dist/$name.zip"
