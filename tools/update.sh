#!/bin/sh
# Atualiza o Nexus para a versão mais recente.
#   curl -fsSL https://nexus-download-psi.vercel.app/update.sh | sh
#   curl -fsSL https://nexus-download-psi.vercel.app/update.sh | sh -s -- --force   (reinstala mesmo se já estiver atualizado)
set -eu
BASE="${NEXUS_BASE_URL:-https://nexus-download-psi.vercel.app}"
FORCE=0
[ "${1:-}" = "--force" ] && FORCE=1

if ! command -v apt-get >/dev/null 2>&1; then
  echo "Este atualizador é para Debian/Ubuntu (apt)." >&2
  exit 1
fi
SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"

VER=$(curl -fsSL "$BASE/latest.txt" | tr -d ' \r\n')
case "$VER" in [0-9]*.[0-9]*.[0-9]*) ;; *) echo "Não consegui descobrir a versão mais recente." >&2; exit 1 ;; esac
CUR=$(dpkg-query -W -f='${Version}' nexus 2>/dev/null || echo "nenhuma")
echo "Instalada: $CUR · Mais recente: $VER"
if [ "$CUR" = "$VER" ] && [ "$FORCE" -eq 0 ]; then
  echo "Já está atualizado."
  exit 0
fi

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
cd "$T"
FILE="nexus-$VER-linux-x86_64.tar.gz"
echo "→ Baixando $FILE (93 MB)…"
curl -fL --progress-bar -o "$FILE" "$BASE/$FILE"
curl -fsSL -o "$FILE.sha256" "$BASE/$FILE.sha256"
echo "→ Conferindo integridade…"
sha256sum -c "$FILE.sha256"
tar xzf "$FILE"

if [ "${NEXUS_DRY:-0}" = "1" ]; then
  echo "(simulação: pacote baixado e verificado, nada foi instalado)"
  ls "nexus-$VER"
  exit 0
fi

echo "→ Instalando…"
$SUDO apt-get install -y "./nexus-$VER/nexus_${VER}_amd64.deb"
echo
echo "Pronto: Nexus $VER instalado. Saia da sessão e entre de novo em \"Nexus\" para usar a nova versão."
