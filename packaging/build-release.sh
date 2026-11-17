#!/bin/sh
# Builds Dailix's release assets into dist/: the binary archive (with
# install.sh), the AppImage, install.sh on its own and SHA256SUMS. Built
# in Omnix's Ubuntu 22.04 image (glibc 2.35: runs on Debian 12, Fedora
# 36+ and newer Ubuntus), with Omnix's repo beside this one, as Dailix
# links Omnix's library. Needs docker and network access.
set -eu
cd "$(dirname "$0")/.."
repos="$(cd .. && pwd)"
[ -d "$repos/omnix-360-studio" ] || { echo "Omnix's repo isn't beside this one ($repos/omnix-360-studio)" >&2; exit 1; }
version="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/dailix/Cargo.toml | head -n 1)"
dist=dist
rm -rf "$dist" && mkdir -p "$dist"

docker build -q -t omnix-build:jammy "$repos/omnix-360-studio/packaging" >/dev/null
docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -v "$repos":/src -w /src/dailix \
    -e CARGO_TARGET_DIR=/src/dailix/target/jammy omnix-build:jammy cargo build --release --locked -p dailix
exe=target/jammy/release/dailix
strip "$exe"

# Archive: dailix-x86_64-linux/ with the binary, install script, menu entry
# and icons. The asset names carry no version, so
# .../releases/latest/download/<name> stays valid.
pkg="$dist/dailix-x86_64-linux"
mkdir -p "$pkg"
cp "$exe" packaging/install.sh packaging/dailix.desktop packaging/dailix.png \
    packaging/dailix.svg README.md LICENSE-MIT LICENSE-APACHE "$pkg/"
chmod 755 "$pkg/dailix" "$pkg/install.sh"
tar -czf "$dist/dailix-x86_64-linux.tar.gz" -C "$dist" dailix-x86_64-linux
cp packaging/install.sh "$dist/install.sh"

# AppImage: ffmpeg and ALSA from the system, as Omnix's.
app="$dist/Dailix.AppDir"
mkdir -p "$app/usr/bin" "$app/usr/share/applications" "$app/usr/share/icons/hicolor/256x256/apps"
cp "$exe" "$app/usr/bin/"
cp packaging/AppRun "$app/AppRun" && chmod 755 "$app/AppRun"
cp packaging/dailix.desktop "$app/" && cp packaging/dailix.desktop "$app/usr/share/applications/"
cp packaging/dailix.png "$app/" && cp packaging/dailix.png "$app/usr/share/icons/hicolor/256x256/apps/"
ln -s dailix.png "$app/.DirIcon"
tool="$repos/omnix-360-studio/target/appimagetool-x86_64.AppImage"
if [ ! -x "$tool" ]; then
    mkdir -p "$(dirname "$tool")"
    curl -fsSL https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage -o "$tool"
    chmod 755 "$tool"
fi
ARCH=x86_64 VERSION="$version" "$tool" --appimage-extract-and-run -n "$app" "$dist/Dailix-x86_64.AppImage" >/dev/null
rm -rf "$app" "$pkg"

(cd "$dist" && sha256sum dailix-x86_64-linux.tar.gz Dailix-x86_64.AppImage install.sh > SHA256SUMS)
ls -l "$dist"
