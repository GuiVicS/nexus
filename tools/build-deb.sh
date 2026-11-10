#!/bin/sh
# Gera dist/nexus_<versão>_amd64.deb (sem depender do nfpm). Uso: tools/build-deb.sh
set -eu
cd "$(dirname "$0")/.."
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
OUT=dist/pkg
rm -rf "$OUT" && mkdir -p "$OUT"

. "$HOME/.cargo/env" 2>/dev/null || true
cargo build --release -p nexus-core -p nexus-installer
pnpm --filter @nexus/shell build
pnpm --filter @nexus/remote build

install -Dm755 target/release/nexus-core "$OUT/usr/bin/nexus-core"
install -Dm755 target/release/nexus-installer "$OUT/usr/bin/nexus-installer"
install -Dm755 system/session/nexus-session "$OUT/usr/bin/nexus-session"
install -Dm755 system/session/nexus-launch-desktop "$OUT/usr/bin/nexus-launch-desktop"
install -Dm644 system/session/nexus.desktop "$OUT/usr/share/wayland-sessions/nexus.desktop"
install -Dm644 system/session/nexus-shell.desktop "$OUT/usr/share/applications/nexus-shell.desktop"
install -Dm644 system/sway/nexus-kiosk.conf "$OUT/usr/share/nexus/sway-kiosk.conf"
for f in system/systemd/*; do install -Dm644 "$f" "$OUT/usr/lib/systemd/user/$(basename "$f")"; done
install -Dm644 system/polkit/br.nexus.sysd.policy "$OUT/usr/share/polkit-1/actions/br.nexus.sysd.policy"
install -Dm644 system/udev/70-nexus-uinput.rules "$OUT/usr/lib/udev/rules.d/70-nexus-uinput.rules"
install -Dm644 system/udev/nexus-uinput.conf "$OUT/usr/lib/modules-load.d/nexus-uinput.conf"
install -Dm644 system/apparmor/opt.nexus.shell.nexus-shell "$OUT/etc/apparmor.d/opt.nexus.shell.nexus-shell"
mkdir -p "$OUT/usr/share/nexus/remote" && cp -r apps/remote/dist/. "$OUT/usr/share/nexus/remote/"

# Shell = Electron + app
ELECTRON=apps/shell/node_modules/electron/dist
mkdir -p "$OUT/opt/nexus/shell"
cp -r "$ELECTRON"/. "$OUT/opt/nexus/shell/"
mv "$OUT/opt/nexus/shell/electron" "$OUT/opt/nexus/shell/nexus-shell"
rm -rf "$OUT/opt/nexus/shell/resources/default_app.asar"
mkdir -p "$OUT/opt/nexus/shell/resources/app"
cp -r apps/shell/electron apps/shell/dist apps/shell/package.json "$OUT/opt/nexus/shell/resources/app/"

mkdir -p "$OUT/DEBIAN"
SIZE=$(du -sk "$OUT" | cut -f1)
cat > "$OUT/DEBIAN/control" <<CTL
Package: nexus
Version: $VER
Section: games
Priority: optional
Architecture: amd64
Installed-Size: $SIZE
Depends: sway, xwayland, systemd, dbus, libgtk-3-0, libnss3, libasound2t64 | libasound2
Recommends: steam-installer | steam, pipewire
Maintainer: Nexus <contato@stellarprint.com.br>
Description: Interface de console (TV + controle) para jogos no Linux
 Sessão Wayland própria, Shell em tela cheia, biblioteca Steam e apps Linux,
 e o celular como controle, mouse e teclado.
CTL
cp system/packaging/postinstall.sh "$OUT/DEBIAN/postinst"
chmod 755 "$OUT/DEBIAN/postinst"
dpkg-deb --root-owner-group --build "$OUT" "dist/nexus_${VER}_amd64.deb"
ls -lh "dist/nexus_${VER}_amd64.deb"
