#!/bin/sh
# Trava as dependências dos aparelhos nas versões DESTA máquina (a de desenvolvimento):
#   tools/pin-deps.sh retroarch   → commit do RetroArch (Flatpak) instalado aqui (+ o mesmo commit para ARM)
#   tools/pin-deps.sh cores       → empacota os núcleos instalados aqui (PC) e congela os de ARM baixados agora
#   tools/pin-deps.sh             → os dois
# Grava system/packaging/deps.lock. Os pacotes de núcleos ficam em dist/ e vão para a release "cores-<ID>":
#   gh release create cores-<ID> dist/nexus-cores-<ID>-linux-*.tar.gz* --title "Núcleos <ID>" --notes "..."
# (só precisa publicar quando os núcleos mudarem; as releases do Nexus apontam para ela pelo deps.lock).
set -eu
cd "$(dirname "$0")/.."
LOCK=system/packaging/deps.lock
WHAT=${1:-all}
APP=org.libretro.RetroArch
set_key() { # set_key CHAVE valor
  if grep -q "^$1=" "$LOCK"; then sed -i "s|^$1=.*|$1=$2|" "$LOCK"; else echo "$1=$2" >> "$LOCK"; fi
}

if [ "$WHAT" = all ] || [ "$WHAT" = retroarch ]; then
  INFO=$(LC_ALL=C flatpak info --system "$APP" 2>/dev/null) || { echo "RetroArch (Flatpak, sistema) não está instalado aqui." >&2; exit 1; }
  VER=$(echo "$INFO" | sed -n 's/^ *Version: //p')
  COMMIT=$(echo "$INFO" | sed -n 's/^ *Commit: //p')
  SUBJ=$(echo "$INFO" | sed -n 's/^ *Subject: //p')
  HOST=$(flatpak --default-arch)
  set_key RETROARCH_VERSION "$VER"
  set_key "RETROARCH_COMMIT_$HOST" "$COMMIT"
  # a outra arquitetura tem commits próprios: acha no histórico do Flathub o commit com o mesmo assunto (mesma versão)
  for A in x86_64 aarch64; do
    [ "$A" = "$HOST" ] && continue
    C=$(LC_ALL=C flatpak remote-info --system --log --arch="$A" flathub "$APP" | awk -v s="$SUBJ" '/^ *Commit:/{c=$2} /^ *Subject:/{sub(/^ *Subject: /,""); if($0==s){print c; exit}}')
    [ -n "$C" ] || { echo "Não achei no Flathub o RetroArch $VER para $A (assunto \"$SUBJ\")." >&2; exit 1; }
    set_key "RETROARCH_COMMIT_$A" "$C"
  done
  echo "RetroArch travado em $VER ($COMMIT)"
fi

if [ "$WHAT" = all ] || [ "$WHAT" = cores ]; then
  SRC="$HOME/.var/app/$APP/config/retroarch/cores"
  ls "$SRC"/*_libretro.so >/dev/null 2>&1 || { echo "Sem núcleos em $SRC" >&2; exit 1; }
  ID=$(date +%Y%m%d)
  mkdir -p dist
  # PC: exatamente os núcleos desta máquina
  W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
  D="$W/x86_64/nexus-cores-$ID"; mkdir -p "$D"
  cp "$SRC"/*_libretro.so "$D/"
  # ARM: os mesmos núcleos, baixados uma vez agora do buildbot e congelados neste pacote
  A="$W/arm64/nexus-cores-$ID"; mkdir -p "$A"
  for f in "$D"/*_libretro.so; do
    n=$(basename "$f")
    if curl -fsSL --retry 5 --retry-all-errors --retry-delay 3 -o "$W/z.zip" "https://buildbot.libretro.com/nightly/linux/aarch64/latest/$n.zip"; then
      unzip -o -q "$W/z.zip" -d "$A" && rm -f "$W/z.zip"
    else
      echo "  (sem $n para ARM no buildbot)"
    fi
  done
  for T in x86_64 arm64; do
    P="$W/$T/nexus-cores-$ID"
    echo "$ID" > "$P/ID"
    ( cd "$P" && sha256sum *_libretro.so > SHA256SUMS )
    F="nexus-cores-$ID-linux-$T.tar.gz"
    tar -C "$W/$T" -czf "dist/$F" "nexus-cores-$ID"
    ( cd dist && sha256sum "$F" > "$F.sha256" )
    set_key "CORES_SHA256_$T" "$(cut -d' ' -f1 "dist/$F.sha256")"
    echo "Núcleos $T: $(ls "$P"/*_libretro.so | wc -l) → dist/$F"
  done
  set_key CORES_ID "$ID"
fi
cat "$LOCK" | grep -v '^#'
