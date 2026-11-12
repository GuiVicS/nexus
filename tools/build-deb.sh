#!/bin/sh
# Gera dist/nexus_<versão>_<arq>.deb (sem depender do nfpm).
#   tools/build-deb.sh          → amd64 (PC)
#   tools/build-deb.sh arm64    → arm64 (Raspberry Pi 4/5 com sistema de 64 bits), compilado de forma cruzada no PC.
#     Requer: sudo apt install gcc-aarch64-linux-gnu  e  rustup target add aarch64-unknown-linux-gnu
set -eu
cd "$(dirname "$0")/.."
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=${1:-amd64}
OUT=dist/pkg-$ARCH
rm -rf "$OUT" && mkdir -p "$OUT"

. "$HOME/.cargo/env" 2>/dev/null || true
case "$ARCH" in
  amd64) TRIPLE=""; BIN=target/release; EARCH=x64; RECS="steam-installer | steam, pipewire, gamemode" ;;
  arm64)
    TRIPLE=aarch64-unknown-linux-gnu; BIN=target/$TRIPLE/release; EARCH=arm64
    RECS="pipewire, gamemode, chromium | chromium-browser"  # sem Steam no ARM; navegador para o streaming
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
    ;;
  *) echo "arquitetura desconhecida: $ARCH (use amd64 ou arm64)" >&2; exit 1 ;;
esac
cargo build --release -p nexus-core -p nexus-installer ${TRIPLE:+--target $TRIPLE}
pnpm --filter @nexus/shell build
pnpm --filter @nexus/remote build

install -Dm755 "$BIN/nexus-core" "$OUT/usr/bin/nexus-core"
install -Dm755 "$BIN/nexus-installer" "$OUT/usr/bin/nexus-installer"
install -Dm755 system/session/nexus-session "$OUT/usr/bin/nexus-session"
install -Dm755 system/session/nexus-launch-desktop "$OUT/usr/bin/nexus-launch-desktop"
install -Dm755 system/overlay/nexus-overlay "$OUT/usr/bin/nexus-overlay"
install -Dm644 system/session/nexus.desktop "$OUT/usr/share/wayland-sessions/nexus.desktop"
install -Dm644 system/session/nexus-shell.desktop "$OUT/usr/share/applications/nexus-shell.desktop"
install -Dm644 system/sway/nexus-kiosk.conf "$OUT/usr/share/nexus/sway-kiosk.conf"
install -Dm644 system/brand/splash.png "$OUT/usr/share/nexus/splash.png"
for f in system/plymouth/nexus/*; do install -Dm644 "$f" "$OUT/usr/share/plymouth/themes/nexus/$(basename "$f")"; done
for f in system/systemd/*; do install -Dm644 "$f" "$OUT/usr/lib/systemd/user/$(basename "$f")"; done
install -Dm644 system/polkit/br.nexus.sysd.policy "$OUT/usr/share/polkit-1/actions/br.nexus.sysd.policy"
# atualizar/restaurar apps sem senha (ajudante root + unidade do sistema + regra do polkit só para ela)
install -Dm755 system/sysd/nexus-app-helper "$OUT/usr/lib/nexus/nexus-app-helper"
install -Dm644 system/sysd/nexus-app@.service "$OUT/usr/lib/systemd/system/nexus-app@.service"
# versões travadas das dependências (RetroArch e núcleos) + quem as aplica no aparelho (tools/pin-deps.sh)
install -Dm644 system/packaging/deps.lock "$OUT/usr/share/nexus/deps.lock"
install -Dm755 system/sysd/nexus-deps-apply "$OUT/usr/lib/nexus/nexus-deps-apply"
# sistema leve para jogos (serviços dispensáveis, zram, apt fora do horário de jogo); reversível
install -Dm755 system/sysd/nexus-slim "$OUT/usr/lib/nexus/nexus-slim"
install -Dm644 system/polkit/50-nexus-apps.rules "$OUT/usr/share/polkit-1/rules.d/50-nexus-apps.rules"
install -Dm644 system/polkit/51-nexus-system.rules "$OUT/usr/share/polkit-1/rules.d/51-nexus-system.rules"
install -Dm644 system/udev/70-nexus-uinput.rules "$OUT/usr/lib/udev/rules.d/70-nexus-uinput.rules"
install -Dm644 system/udev/nexus-uinput.conf "$OUT/usr/lib/modules-load.d/nexus-uinput.conf"
install -Dm644 system/apparmor/opt.nexus.shell.nexus-shell "$OUT/etc/apparmor.d/opt.nexus.shell.nexus-shell"
mkdir -p "$OUT/usr/share/nexus/remote" && cp -r apps/remote/dist/. "$OUT/usr/share/nexus/remote/"

# Shell = Electron + app. Para outra arquitetura, baixa o Electron da mesma versão (oficial, conferido por SHA-256).
ELECTRON=apps/shell/node_modules/electron/dist
if [ "$EARCH" != x64 ]; then
  EV=$(node -p "require('./apps/shell/node_modules/electron/package.json').version")
  Z="electron-v$EV-linux-$EARCH.zip"
  CACHE="$HOME/.cache/nexus-build"
  mkdir -p "$CACHE"
  if [ ! -f "$CACHE/$Z" ]; then
    curl -fL --retry 3 -o "$CACHE/$Z.part" "https://github.com/electron/electron/releases/download/v$EV/$Z"
    curl -fsSL "https://github.com/electron/electron/releases/download/v$EV/SHASUMS256.txt" | grep " \*$Z\$" | sed "s| \*$Z\$|  $CACHE/$Z.part|" | sha256sum -c -
    mv "$CACHE/$Z.part" "$CACHE/$Z"
  fi
  ELECTRON="$CACHE/electron-$EV-$EARCH"
  [ -x "$ELECTRON/electron" ] || { rm -rf "$ELECTRON"; mkdir -p "$ELECTRON"; unzip -q "$CACHE/$Z" -d "$ELECTRON"; }
fi
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
Architecture: $ARCH
Installed-Size: $SIZE
Depends: sway, xwayland, systemd, dbus, libgtk-3-0, libnss3, libasound2t64 | libasound2, grim, python3-gi, gir1.2-gtk-3.0, gir1.2-gtklayershell-0.1, unzip, bluez, curl, ca-certificates, rclone, fuse3
Recommends: $RECS
Maintainer: Nexus <contato@stellarprint.com.br>
Description: Interface de console (TV + controle) para jogos no Linux
 Sessão Wayland própria, Shell em tela cheia, biblioteca Steam e apps Linux,
 e o celular como controle, mouse e teclado.
CTL
cp system/packaging/postinstall.sh "$OUT/DEBIAN/postinst"
chmod 755 "$OUT/DEBIAN/postinst"
dpkg-deb --root-owner-group --build "$OUT" "dist/nexus_${VER}_${ARCH}.deb"
rm -rf "$OUT"
ls -lh "dist/nexus_${VER}_${ARCH}.deb"
