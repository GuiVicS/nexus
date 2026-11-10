#!/bin/sh
# Gera um único arquivo para levar ao Linux: dist/nexus-<versão>-linux-x86_64.tar.gz
set -eu
cd "$(dirname "$0")/.."
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
tools/build-deb.sh
B="dist/bundle/nexus-$VER"
rm -rf dist/bundle && mkdir -p "$B"
cp "dist/nexus_${VER}_amd64.deb" install.sh "$B/"
cat > "$B/LEIAME.txt" <<TXT
Nexus $VER — instalação (Debian/Ubuntu 24.04, x86_64)

  tar xzf nexus-$VER-linux-x86_64.tar.gz
  cd nexus-$VER
  sudo ./install.sh                    # Nexus + Steam
  sudo ./install.sh --autologin SEU_USUARIO     # (opcional) entrar direto no Nexus

Depois, na tela de login, escolha a sessão "Nexus".
Emergência: Ctrl+Alt+F3 abre um terminal; "sudo touch /etc/nexus/force-desktop" volta ao desktop normal.
Desinstalar: sudo nexus-installer revert-autologin ; sudo apt remove nexus
TXT
tar -C dist/bundle -czf "dist/nexus-$VER-linux-x86_64.tar.gz" "nexus-$VER"
( cd dist && sha256sum "nexus-$VER-linux-x86_64.tar.gz" > "nexus-$VER-linux-x86_64.tar.gz.sha256" )
ls -lh dist/nexus-$VER-linux-x86_64.tar.gz*
