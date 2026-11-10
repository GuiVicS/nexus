#!/bin/sh
# Instalador do Nexus para Debian/Ubuntu x86_64.
#   sudo ./install.sh                         instala o Nexus + Steam
#   sudo ./install.sh --sem-steam             só o Nexus
#   sudo ./install.sh --emulation             acrescenta o RetroArch (Flatpak)
#   sudo ./install.sh --autologin SEU_USUARIO entra direto no Nexus ao ligar
#   sudo ./install.sh --plano                 só mostra o que seria feito
set -eu
cd "$(dirname "$0")"

if [ "$(id -u)" -ne 0 ]; then
  echo "Execute com sudo:  sudo ./install.sh" >&2
  exit 1
fi
if ! command -v apt-get >/dev/null 2>&1; then
  echo "Este instalador é para Debian/Ubuntu (apt). Outras distros: veja o README." >&2
  exit 1
fi

STEAM=--steam
PLAN=0
ARGS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --sem-steam) STEAM="" ;;
    --plano) PLAN=1 ;;
    --autologin) ARGS="$ARGS --autologin $2"; shift ;;
    --emulation) ARGS="$ARGS --emulation" ;;
    *) echo "opção desconhecida: $1" >&2; exit 1 ;;
  esac
  shift
done

DEB=$(ls -1 ./nexus_*_amd64.deb ./dist/nexus_*_amd64.deb 2>/dev/null | tail -1 || true)
if [ -z "$DEB" ] && ! [ -x /usr/bin/nexus-installer ]; then
  echo "Não encontrei o pacote nexus_*_amd64.deb ao lado deste script." >&2
  exit 1
fi

apt-get update
if [ -n "$DEB" ]; then
  echo "→ Instalando $DEB"
  [ "$PLAN" -eq 1 ] || apt-get install -y "$DEB"
fi

if [ "$PLAN" -eq 1 ]; then
  exec nexus-installer plan $STEAM $ARGS
fi
nexus-installer check
nexus-installer install $STEAM $ARGS --yes
echo
echo "Pronto! Reinicie ou saia da sessão e escolha \"Nexus\" na tela de login."
echo "Problemas? Rode: nexus-installer doctor   (Ctrl+Alt+F3 abre um terminal de emergência)"
