#!/bin/sh
# Gera um único arquivo para levar ao Linux:
#   tools/bundle.sh         → dist/nexus-<versão>-linux-x86_64.tar.gz  (PC)
#   tools/bundle.sh arm64   → dist/nexus-<versão>-linux-arm64.tar.gz   (Raspberry Pi 4/5, sistema de 64 bits)
set -eu
cd "$(dirname "$0")/.."
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=${1:-amd64}
case "$ARCH" in amd64) TAG=x86_64; ALVO="Debian/Ubuntu 24.04, PC x86_64"; COM="Nexus + Steam" ;; arm64) TAG=arm64; ALVO="Raspberry Pi 4/5 com Raspberry Pi OS 64 bits (ou Debian/Ubuntu arm64)"; COM="Nexus (sem Steam: não existe para ARM)" ;; *) echo "use amd64 ou arm64" >&2; exit 1 ;; esac
tools/build-deb.sh "$ARCH"
B="dist/bundle-$ARCH/nexus-$VER"
rm -rf "dist/bundle-$ARCH" && mkdir -p "$B"
cp "dist/nexus_${VER}_${ARCH}.deb" install.sh "$B/"
cat > "$B/LEIAME.txt" <<TXT
Nexus $VER — instalação ($ALVO)

  tar xzf nexus-$VER-linux-$TAG.tar.gz
  cd nexus-$VER
  sudo ./install.sh                    # $COM
  sudo ./install.sh --emulation        # + RetroArch (emuladores)
  sudo ./install.sh --autologin SEU_USUARIO     # (opcional) entrar direto no Nexus

Depois, na tela de login, escolha a sessão "Nexus".
Emergência: Ctrl+Alt+F3 abre um terminal; "sudo touch /etc/nexus/force-desktop" volta ao desktop normal.
Desinstalar: sudo nexus-installer revert-autologin ; sudo apt remove nexus
TXT
tar -C "dist/bundle-$ARCH" -czf "dist/nexus-$VER-linux-$TAG.tar.gz" "nexus-$VER"
rm -rf "dist/bundle-$ARCH"
( cd dist && sha256sum "nexus-$VER-linux-$TAG.tar.gz" > "nexus-$VER-linux-$TAG.tar.gz.sha256" )
ls -lh dist/nexus-$VER-linux-$TAG.tar.gz*
