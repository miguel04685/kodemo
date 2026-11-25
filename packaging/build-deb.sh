#!/usr/bin/env bash
# Build Kodemo's .deb package.
#
#   packaging/build-deb.sh              # native architecture
#   packaging/build-deb.sh --arch arm64 # must run on arm64 (or with a cross toolchain)
#
# The package is written to target/kodemo_<version>_<arch>.deb.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

WANT_ARCH="$(dpkg --print-architecture)"
if [ "${1:-}" = "--arch" ] && [ -n "${2:-}" ]; then
    WANT_ARCH="$2"
fi
HOST_ARCH="$(dpkg --print-architecture)"
if [ "$WANT_ARCH" != "$HOST_ARCH" ]; then
    cat >&2 <<EOF
Kodemo: cannot build a $WANT_ARCH package on a $HOST_ARCH host.

The binary embeds llama.cpp shared libraries and must be compiled natively.
Run this script on an $WANT_ARCH machine (or in an $WANT_ARCH container /
debootstrap chroot) with:
    sudo apt install build-essential pkg-config libgl1-mesa-dev libx11-dev
    cargo build --release --offline --bins
    packaging/build-deb.sh
EOF
    exit 1
fi

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
echo "Kodemo $VERSION ($WANT_ARCH): building release binaries..."
cargo build --release --offline --bins

STAGE="$ROOT/target/deb-root"
rm -rf "$STAGE"
install -d "$STAGE/DEBIAN" \
    "$STAGE/usr/bin" \
    "$STAGE/usr/lib/kodemo" \
    "$STAGE/usr/share/applications" \
    "$STAGE/usr/share/icons/hicolor/256x256/apps" \
    "$STAGE/usr/share/icons/hicolor/512x512/apps" \
    "$STAGE/usr/share/kodemo" \
    "$STAGE/usr/share/doc/kodemo"

install -m 0755 target/release/kodemo "$STAGE/usr/bin/kodemo"
install -m 0755 target/release/kodemo-webview "$STAGE/usr/lib/kodemo/kodemo-webview"
install -m 0644 packaging/debian/kodemo.desktop "$STAGE/usr/share/applications/kodemo.desktop"
install -m 0644 packaging/icons/kodemo-256.png "$STAGE/usr/share/icons/hicolor/256x256/apps/kodemo.png"
install -m 0644 packaging/icons/kodemo-512.png "$STAGE/usr/share/icons/hicolor/512x512/apps/kodemo.png"
install -m 0644 assets/kodemi-jump.png "$STAGE/usr/share/kodemo/kodemi-jump.png"
install -m 0644 assets/dict/pt_BR-LICENSE.txt "$STAGE/usr/share/kodemo/pt_BR-LICENSE.txt"

# Bundle llama.cpp when the build machine has it (the same place Kodemo's
# installer and release notes use). Without it the AI option stays disabled.
LLAMA_DIR="$HOME/.local/share/kodemo/llama"
if [ -x "$LLAMA_DIR/llama-server" ]; then
    echo "Bundling llama.cpp from $LLAMA_DIR"
    cp -a "$LLAMA_DIR" "$STAGE/usr/lib/kodemo/llama"
else
    echo "warning: llama.cpp not found in $LLAMA_DIR; the package will not include the local AI server."
fi

cat > "$STAGE/usr/share/doc/kodemo/copyright" <<'EOF'
Kodemo is dual-licensed under MIT OR Apache-2.0.
See /usr/share/doc/kodemo/LICENSE-MIT and LICENSE-APACHE (copied below).

Bundled assets:
  Inter, JetBrains Mono ............ SIL Open Font License 1.1
  Space Grotesk .................... SIL Open Font License 1.1
  Phosphor Icons ................... MIT
  Portuguese word list ............. MPL-2.0 (see pt_BR-LICENSE.txt)
  llama.cpp (optional) ............. MIT
EOF
cp LICENSE-MIT LICENSE-APACHE "$STAGE/usr/share/doc/kodemo/"

SIZE="$(du -sk "$STAGE" | cut -f1)"
sed -e "s/@VERSION@/$VERSION/" -e "s/@ARCH@/$WANT_ARCH/" -e "s/@SIZE@/$SIZE/" \
    packaging/debian/control.in > "$STAGE/DEBIAN/control"
install -m 0755 packaging/debian/postinst "$STAGE/DEBIAN/postinst"
install -m 0644 packaging/debian/templates "$STAGE/DEBIAN/templates"

# md5sums are optional but apt is happier with them.
( cd "$STAGE" && find usr -type f -print0 | xargs -0 md5sum > DEBIAN/md5sums )

OUT="$ROOT/target/kodemo_${VERSION}_${WANT_ARCH}.deb"
dpkg-deb --build --root-owner-group "$STAGE" "$OUT" >/dev/null
echo "Built $OUT"
dpkg-deb --info "$OUT" | sed -n '1,14p'
