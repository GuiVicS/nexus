# Repositório apt do Nexus

Objetivo: instalar e atualizar o Nexus só com o apt (`sudo apt upgrade` atualiza junto com o sistema), com os
pacotes assinados por uma chave GPG do projeto. Hospedagem gratuita: **GitHub Pages** do próprio repositório
(`https://guivics.github.io/nexus`), refeito a cada release por `.github/workflows/apt-repo.yml`.

## No aparelho
```sh
sudo curl -fsSLo /usr/share/keyrings/nexus-archive-keyring.gpg https://guivics.github.io/nexus/nexus-archive-keyring.gpg
sudo curl -fsSLo /etc/apt/sources.list.d/nexus.sources https://guivics.github.io/nexus/nexus.sources
sudo apt update && sudo apt install nexus
```
Transição: o comando de sempre (`curl …/tools/update.sh | sh`) continua funcionando e, quando o repositório estiver
no ar, configura a chave e a origem sozinho.

## Como o repositório é montado
`tools/apt-repo.sh <debs> <saída>`: `pool/` + `dists/stable/main/binary-{amd64,arm64}/Packages(.gz)` +
`Release` com MD5/SHA256, `InRelease` (assinado em claro) e `Release.gpg`; exporta a chave pública e o
`nexus.sources` (deb822 com `Signed-By`, a chave só vale para este repositório).

Testado aqui com uma chave descartável e um `apt-get update` real (sem root, diretórios próprios):
candidato 0.4.1 e download ok; índice adulterado → recusado ("tamanho inesperado"); chave errada → recusado
("NO_PUBKEY … não está assinado").

## Ligar a publicação (uma vez, pelo dono do repositório)
1. Gerar a chave de assinatura (sem senha, só assina; guardar um backup fora do GitHub):
   `gpg --quick-gen-key "Nexus (repositório apt) <EMAIL>" ed25519 sign 3y`
2. GitHub → Settings → Secrets and variables → Actions:
   segredo `APT_GPG_KEY` = `gpg --armor --export-secret-keys EMAIL`; variável `APT_GPG_KEY_ID` = `EMAIL`.
3. GitHub → Settings → Pages → Source: **GitHub Actions**.
4. Rodar o workflow "Repositório apt" (ou publicar uma release).

## Limites e riscos
- O GitHub Pages tem limite de ~1 GB por site: o workflow publica só as **2 últimas versões** (≈ 4 × 100 MB).
  Se o Pages recusar arquivos de ~100 MB, alternativa gratuita: Cloudflare R2 (10 GB grátis) com o mesmo script.
- A chave privada fica no segredo do GitHub e no backup do dono; se vazar, gerar outra e republicar a pública
  (os aparelhos baixam de novo pelo `update.sh`).
- Base suportada: **Debian estável** (oficial, ver `docs/BASE.md`); Ubuntu LTS funciona "sem garantia".
