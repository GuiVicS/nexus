#!/bin/sh
# Instala ou atualiza o Nexus a partir das releases do GitHub (PC x86_64 ou Raspberry Pi arm64).
#   curl -fsSL https://raw.githubusercontent.com/GuiVicS/nexus/main/tools/update.sh | sh
#   ... | sh -s -- --force                       reinstala mesmo se já estiver atualizado
#   ... | sh -s -- --versao 0.4.2                instala uma versão específica (voltar para uma anterior)
#   ... | sh -s -- --autologin SEU_USUARIO       (1ª instalação) entra direto no Nexus ao ligar
#   ... | sh -s -- --sem-steam / --sem-emuladores (1ª instalação) deixa de fora a Steam / o RetroArch
# Sem o Nexus instalado, roda o instalador completo (Nexus + Steam + RetroArch + navegador + dependências).
# Com o Nexus instalado, só troca o pacote pela versão nova.
set -eu
REPO="${NEXUS_REPO:-GuiVicS/nexus}"
FORCE=0
WANT=""
ARGS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --force) FORCE=1 ;;
    --versao) WANT="$2"; shift ;;
    --autologin) ARGS="$ARGS --autologin $2"; shift ;;
    --sem-steam|--sem-emuladores|--sem-otimizar) ARGS="$ARGS $1" ;;
    *) echo "opção desconhecida: $1" >&2; exit 1 ;;
  esac
  shift
done

if ! command -v apt-get >/dev/null 2>&1; then
  echo "Este instalador é para Debian/Ubuntu/Raspberry Pi OS (apt)." >&2
  exit 1
fi
SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"
command -v curl >/dev/null 2>&1 || { $SUDO apt-get update && $SUDO apt-get install -y curl ca-certificates; }

ARCH=$(dpkg --print-architecture)
case "$ARCH" in
  amd64) TAG=x86_64 ;;
  arm64) TAG=arm64 ;;
  *) echo "Arquitetura $ARCH sem pacote do Nexus (só amd64 = PC e arm64 = Raspberry Pi 4/5 com sistema de 64 bits)." >&2; exit 1 ;;
esac

# versão mais recente: o GitHub redireciona /releases/latest para /releases/tag/vX.Y.Z (sem usar a API, que tem limite).
# Repositório privado: o link aberto dá 404; aí usa o GitHub CLI já logado (gh auth login).
GH=0
if [ -n "$WANT" ]; then
  case "$WANT" in [0-9]*.[0-9]*.[0-9]*) ;; *) echo "Versão inválida: $WANT" >&2; exit 1 ;; esac
  VER=$WANT
  # repositório privado: sem acesso direto à release, usa o GitHub CLI já logado
  if ! curl -fsSLI -o /dev/null "https://github.com/$REPO/releases/tag/v$VER" 2>/dev/null && command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then GH=1; fi
else
URL=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest" 2>/dev/null || true)
VER=${URL##*/v}
case "$VER" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *)
    if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
      GH=1
      VER=$(gh release view -R "$REPO" --json tagName -q .tagName | sed 's/^v//')
    else
      echo "Não consegui acessar github.com/$REPO (repositório privado?). Instale o GitHub CLI e entre na conta:" >&2
      echo "  sudo apt install gh && gh auth login" >&2
      exit 1
    fi ;;
esac
case "$VER" in [0-9]*.[0-9]*.[0-9]*) ;; *) echo "Não consegui descobrir a versão mais recente em github.com/$REPO." >&2; exit 1 ;; esac
fi
CUR=$(dpkg-query -W -f='${Version}' nexus 2>/dev/null || echo "nenhuma")
echo "Instalada: $CUR · Mais recente: $VER ($ARCH)"
if [ "$CUR" = "$VER" ] && [ "$FORCE" -eq 0 ]; then
  echo "Já está atualizado."
  exit 0
fi

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
cd "$T"
FILE="nexus-$VER-linux-$TAG.tar.gz"
BASE="https://github.com/$REPO/releases/download/v$VER"
echo "→ Baixando $FILE (~100 MB)…"
if [ "$GH" -eq 1 ]; then
  gh release download "v$VER" -R "$REPO" -p "$FILE" -p "$FILE.sha256"
else
  curl -fL --progress-bar -o "$FILE" "$BASE/$FILE"
  curl -fsSL -o "$FILE.sha256" "$BASE/$FILE.sha256"
fi
echo "→ Conferindo integridade…"
sha256sum -c "$FILE.sha256"
tar xzf "$FILE"

# Núcleos dos emuladores: travados no deps.lock (versões da máquina de desenvolvimento), numa release
# própria "cores-<ID>". Só baixa quando o ID muda e quando há (ou vai haver) RetroArch.
CID=$(sed -n 's/^CORES_ID=//p' "nexus-$VER/deps.lock" 2>/dev/null || true)
CF=""
EMU=1
case "$ARGS" in *--sem-emuladores*) EMU=0 ;; esac
[ "$CUR" != "nenhuma" ] && ! flatpak info --system org.libretro.RetroArch >/dev/null 2>&1 && EMU=0
if [ -n "$CID" ] && [ "$EMU" -eq 1 ] && [ "$(cat /usr/lib/nexus/cores/ID 2>/dev/null)" != "$CID" ]; then
  CF="nexus-cores-$CID-linux-$TAG.tar.gz"
  echo "→ Baixando os núcleos dos emuladores ($CID)…"
  if [ "$GH" -eq 1 ]; then
    gh release download "cores-$CID" -R "$REPO" -p "$CF" || CF=""
  else
    curl -fL --progress-bar -o "$CF" "https://github.com/$REPO/releases/download/cores-$CID/$CF" || CF=""
  fi
  [ -n "$CF" ] || echo "  (não deu para baixar os núcleos agora; os emuladores ficam com os atuais)" >&2
fi

if [ "${NEXUS_DRY:-0}" = "1" ]; then
  echo "(simulação: pacote baixado e verificado, nada foi instalado)"
  ls "nexus-$VER"
  exit 0
fi

if [ "$CUR" = "nenhuma" ]; then
  echo "→ Primeira instalação: Nexus e todas as dependências…"
  cd "nexus-$VER"
  # shellcheck disable=SC2086
  $SUDO ./install.sh $ARGS
else
  echo "→ Atualizando…"
  $SUDO apt-get install -y --allow-downgrades "./nexus-$VER/nexus_${VER}_${ARCH}.deb"
  echo
  echo "Pronto: Nexus $VER instalado. Saia da sessão e entre de novo em \"Nexus\" para usar a nova versão."
fi
if [ -n "$CF" ]; then
  $SUDO /usr/lib/nexus/nexus-deps-apply --cores "$T/$CF" || echo "  (os núcleos baixados não conferem; ficam os atuais)" >&2
fi

# Transição para o repositório apt (docs/APT.md): se ele já estiver no ar, configura a chave e a origem para o
# Nexus passar a atualizar junto com o sistema (sudo apt upgrade). Enquanto não estiver, nada muda.
APT_URL="${NEXUS_APT_URL:-https://guivics.github.io/nexus}"
if [ ! -f /etc/apt/sources.list.d/nexus.sources ] && curl -fsSL -o "$T/nexus-archive-keyring.gpg" "$APT_URL/nexus-archive-keyring.gpg" 2>/dev/null \
   && curl -fsSL -o "$T/nexus.sources" "$APT_URL/nexus.sources" 2>/dev/null && grep -q '^Signed-By:' "$T/nexus.sources"; then
  $SUDO install -Dm644 "$T/nexus-archive-keyring.gpg" /usr/share/keyrings/nexus-archive-keyring.gpg
  $SUDO install -Dm644 "$T/nexus.sources" /etc/apt/sources.list.d/nexus.sources
  echo "→ Repositório apt do Nexus configurado: daqui em diante, sudo apt upgrade atualiza o Nexus."
fi
