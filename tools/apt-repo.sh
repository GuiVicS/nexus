#!/bin/sh
# Monta o repositório apt assinado do Nexus a partir de pacotes .deb:
#   tools/apt-repo.sh <pasta-com-debs> <saída>        (assina com a chave NEXUS_APT_KEY, do GNUPGHOME atual)
# Estrutura (uma suíte "stable", componente "main", amd64 e arm64):
#   <saída>/pool/main/n/nexus/*.deb
#   <saída>/dists/stable/main/binary-<arq>/Packages(.gz)
#   <saída>/dists/stable/{Release,InRelease,Release.gpg}
#   <saída>/nexus-archive-keyring.gpg   chave pública (binária) para /usr/share/keyrings/
#   <saída>/nexus.sources               linha do apt (deb822) para /etc/apt/sources.list.d/
# Publicação: GitHub Pages (.github/workflows/apt-repo.yml), a cada release.
set -eu
IN=$1
OUT=$2
KEY=${NEXUS_APT_KEY:?defina NEXUS_APT_KEY (id ou e-mail da chave GPG de assinatura)}
URL=${NEXUS_APT_URL:-https://guivics.github.io/nexus}
SUITE=stable
rm -rf "$OUT"
mkdir -p "$OUT/pool/main/n/nexus"
found=0
for f in "$IN"/nexus_*_amd64.deb "$IN"/nexus_*_arm64.deb; do
  [ -f "$f" ] || continue
  cp "$f" "$OUT/pool/main/n/nexus/"
  found=1
done
[ "$found" = 1 ] || { echo "nenhum nexus_*.deb em $IN" >&2; exit 1; }

cd "$OUT"
for a in amd64 arm64; do
  d="dists/$SUITE/main/binary-$a"
  mkdir -p "$d"
  dpkg-scanpackages --multiversion --arch "$a" pool /dev/null 2>/dev/null > "$d/Packages"
  gzip -9nk "$d/Packages"
  printf 'Archive: %s\nComponent: main\nOrigin: Nexus\nLabel: Nexus\nArchitecture: %s\n' "$SUITE" "$a" > "$d/Release"
done

# Release: somas de cada índice (caminhos relativos a dists/<suíte>)
cd "dists/$SUITE"
{
  echo "Origin: Nexus"
  echo "Label: Nexus"
  echo "Suite: $SUITE"
  echo "Codename: $SUITE"
  echo "Date: $(LC_ALL=C date -Ru)"
  echo "Architectures: amd64 arm64"
  echo "Components: main"
  echo "Description: Nexus - interface de console para jogos no Linux"
  for h in MD5Sum:md5sum SHA256:sha256sum; do
    echo "${h%%:*}:"
    for f in $(find main -type f | sort); do
      printf ' %s %16d %s\n' "$(${h#*:} "$f" | cut -d' ' -f1)" "$(wc -c < "$f")" "$f"
    done
  done
} > Release
gpg --batch --yes --local-user "$KEY" --clearsign -o InRelease Release
gpg --batch --yes --local-user "$KEY" -abs -o Release.gpg Release
cd ../..

gpg --batch --yes --export "$KEY" > nexus-archive-keyring.gpg
gpg --batch --yes --armor --export "$KEY" > nexus-archive-keyring.asc
cat > nexus.sources <<EOF
Types: deb
URIs: $URL
Suites: $SUITE
Components: main
Signed-By: /usr/share/keyrings/nexus-archive-keyring.gpg
EOF
cat > index.html <<EOF
<!doctype html><meta charset="utf-8"><title>Nexus · repositório apt</title>
<body style="font:16px/1.5 system-ui;max-width:760px;margin:40px auto;padding:0 16px">
<h1>Nexus · repositório apt</h1>
<p>Debian/Ubuntu (amd64) e Raspberry Pi OS 64 bits (arm64):</p>
<pre>sudo curl -fsSLo /usr/share/keyrings/nexus-archive-keyring.gpg $URL/nexus-archive-keyring.gpg
sudo curl -fsSLo /etc/apt/sources.list.d/nexus.sources $URL/nexus.sources
sudo apt update &amp;&amp; sudo apt install nexus</pre>
<p>Depois disso, o Nexus atualiza junto com o sistema (<code>sudo apt upgrade</code>).</p>
</body>
EOF
echo "repositório pronto em $OUT ($(ls pool/main/n/nexus | wc -l) pacotes)"
