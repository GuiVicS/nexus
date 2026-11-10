# Nexus

Interface de console (TV + controle) para jogos no Linux. **Não é uma distro**: é um software instalável que
traz uma sessão própria, uma Shell fullscreen, um Core que cuida de biblioteca/jogos e o celular como controle.

```
Shell (Electron + SolidJS) ⇄ Core (Rust) ⇄ Providers (Steam · Apps Linux · Streaming · Emuladores) ⇄ Steam / .desktop / navegador / RetroArch
       ▲                       ▲
       │ JSON-RPC (socket Unix)│ HTTP+WS (rede local, só o Remote)
       └── Celular (PWA): controle, mouse, teclado
```

**Continuando o desenvolvimento (outra sessão/agente): leia [HANDOFF.md](HANDOFF.md).**
Plano completo: [docs/architecture/ARCHITECTURE.md](docs/architecture/ARCHITECTURE.md) · estado atual: [docs/STATUS.md](docs/STATUS.md)

## Como instalar

Funciona em **PC** (Debian/Ubuntu x86_64) e no **Raspberry Pi 4/5** (Raspberry Pi OS 64 bits ou Debian/Ubuntu arm64).
O instalador já traz **tudo**: o Nexus, a Steam (só no PC — não existe Steam para ARM), o RetroArch com os emuladores,
um navegador para o streaming (se não houver), o Bluetooth e todas as bibliotecas do sistema.

Requisitos: 4 GB de memória e 6 GB livres no PC; 2 GB e 3 GB no Raspberry Pi; internet durante a instalação.

### Opção 1 — um comando (recomendado)
O repositório é privado, então o download usa o GitHub CLI logado na sua conta. Uma vez só:
```sh
sudo apt install -y gh && gh auth login
```
Depois, para **instalar** (ou atualizar — é o mesmo comando):
```sh
gh api repos/GuiVicS/nexus/contents/tools/update.sh -H "Accept: application/vnd.github.raw" | sh
```
Opções da primeira instalação (depois de `| sh -s --`): `--autologin SEU_USUARIO` (liga direto no Nexus),
`--sem-steam`, `--sem-emuladores`. Ex.: `... | sh -s -- --autologin gui`.
Se o repositório ficar público, também funciona sem o `gh`:
`curl -fsSL https://raw.githubusercontent.com/GuiVicS/nexus/main/tools/update.sh | sh`

### Opção 2 — baixando o arquivo
1. Em **[Releases](https://github.com/GuiVicS/nexus/releases/latest)**, baixe o arquivo do seu aparelho:
   PC → `nexus-<versão>-linux-x86_64.tar.gz` · Raspberry Pi → `nexus-<versão>-linux-arm64.tar.gz`
2. No terminal, na pasta do download:
```sh
sha256sum -c nexus-*.tar.gz.sha256          # opcional: confere o download
tar xzf nexus-*-linux-*.tar.gz && cd nexus-*/
sudo ./install.sh                            # instala tudo
sudo ./install.sh --autologin SEU_USUARIO    # opcional: entrar direto no Nexus ao ligar
```

### Depois de instalar
- Reinicie e, na tela de login, escolha a sessão **Nexus** (com `--autologin` ele entra sozinho).
- **ROMs:** coloque em `~/ROMs/<console>` (cada pasta tem um `LEIA-ME.txt` com as extensões aceitas).
- **Steam (PC):** na primeira vez o Nexus mostra o QR code — escaneie com o app Steam no celular.
- **Celular como controle:** Ajustes → Controle no celular → escaneie o QR (mesma rede Wi-Fi).
- **Emergência:** Ctrl+Alt+F3 abre um terminal; `sudo touch /etc/nexus/force-desktop` volta ao desktop normal no próximo login.
- **Desinstalar:** `sudo nexus-installer revert-autologin ; sudo apt remove nexus`

## Atualizar
O mesmo comando da instalação (só troca o pacote; mantém perfis, jogos e configurações):
```sh
gh api repos/GuiVicS/nexus/contents/tools/update.sh -H "Accept: application/vnd.github.raw" | sh
```

## Gerar os pacotes a partir do código
Precisa de Rust, Node 24 e pnpm:
```sh
tools/bundle.sh          # PC  → dist/nexus-<versão>-linux-x86_64.tar.gz
tools/bundle.sh arm64    # Pi  → dist/nexus-<versão>-linux-arm64.tar.gz (requer gcc-aarch64-linux-gnu e o alvo aarch64 do Rust)
```

## Rodar em desenvolvimento (qualquer máquina com Chromium)
```sh
pnpm install && pnpm gen && make build
make dev-core            # abre em http://127.0.0.1:8799  (modo demonstração, sem Steam)
```
Senha dos responsáveis no modo demonstração: `1234`.

## Testes
```sh
cargo test --workspace   # ~110 testes (unitários, de ponta a ponta do Core, instalador e tipos)
make e2e                 # cenários de interface (adulto, criança, perfis, primeiro uso, celular)
```

## Estrutura
`core/` Core em Rust · `apps/shell` Shell · `apps/remote` controle no celular · `packages/` protocolo, foco e tema ·
`installer/` instalador · `system/` sessão, sway, systemd, polkit, udev, AppArmor, pacote · `tools/` · `tests/`
