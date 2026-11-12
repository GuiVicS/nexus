#!/bin/sh
# Instalador do Nexus para Debian/Ubuntu (PC x86_64) e Raspberry Pi OS / Debian arm64 (Raspberry Pi 4/5).
# No ARM não existe Steam para Linux: o Nexus instala sem ela (emuladores, streaming e apps funcionam).
#   sudo ./install.sh                         instala TUDO: Nexus + Steam (PC) + RetroArch (emuladores) + navegador
#                                             para o streaming + Bluetooth e todas as bibliotecas
#   sudo ./install.sh --sem-steam             sem a Steam
#   sudo ./install.sh --sem-emuladores        sem o RetroArch
#   sudo ./install.sh --sem-otimizar          não mexe nos serviços do sistema (por padrão: desliga impressão, modem,
#                                             indexador…, liga zram e tira o apt do horário de jogo; desfazer:
#                                             sudo /usr/lib/nexus/nexus-slim revert)
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
EMU=--emulation
SLIM=--slim
PLAN=0
ARGS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --sem-steam) STEAM="" ;;
    --plano) PLAN=1 ;;
    --autologin) ARGS="$ARGS --autologin $2"; shift ;;
    --sem-emuladores) EMU="" ;;
    --sem-otimizar) SLIM="" ;;
    --emulation) ;;  # já é o padrão (mantido por compatibilidade)
    *) echo "opção desconhecida: $1" >&2; exit 1 ;;
  esac
  shift
done

ARCH=$(dpkg --print-architecture)
case "$ARCH" in
  amd64) ;;
  arm64) [ -n "$STEAM" ] && echo "→ Raspberry Pi/ARM: a Steam não existe para esta arquitetura; instalando sem ela." ; STEAM="" ;;
  *) echo "Arquitetura $ARCH não suportada (precisa de amd64, ou arm64 = Raspberry Pi com sistema de 64 bits)." >&2; exit 1 ;;
esac
DEB=$(ls -1 ./nexus_*_"$ARCH".deb ./dist/nexus_*_"$ARCH".deb 2>/dev/null | tail -1 || true)
if [ -z "$DEB" ] && ! [ -x /usr/bin/nexus-installer ]; then
  echo "Não encontrei o pacote nexus_*_$ARCH.deb ao lado deste script." >&2
  exit 1
fi

apt-get update
if [ -n "$DEB" ]; then
  echo "→ Instalando $DEB"
  [ "$PLAN" -eq 1 ] || apt-get install -y "$DEB"
fi

if [ "$PLAN" -eq 1 ]; then
  # só mostrar o plano: sem o pacote instalado, usa o nexus-installer de dentro do .deb
  if ! command -v nexus-installer >/dev/null 2>&1 && [ -n "$DEB" ]; then
    TMP=$(mktemp -d)
    trap 'rm -rf "$TMP"' EXIT
    dpkg-deb -x "$DEB" "$TMP"
    "$TMP/usr/bin/nexus-installer" plan $STEAM $EMU $SLIM $ARGS
    exit $?
  fi
  exec nexus-installer plan $STEAM $EMU $SLIM $ARGS
fi
nexus-installer check
nexus-installer install $STEAM $EMU $SLIM $ARGS --yes
# núcleos dos emuladores travados pelo Nexus (pacote nexus-cores-* ao lado deste script, se veio junto)
case "$ARCH" in amd64) CT=x86_64 ;; *) CT=arm64 ;; esac
if [ -n "$EMU" ]; then
  for f in ./nexus-cores-*-linux-"$CT".tar.gz; do
    [ -f "$f" ] && { /usr/lib/nexus/nexus-deps-apply --cores "$f" || true; }
  done
fi
echo
echo "Pronto! Reinicie ou saia da sessão e escolha \"Nexus\" na tela de login."
echo "Problemas? Rode: nexus-installer doctor   (Ctrl+Alt+F3 abre um terminal de emergência)"
