# Nexus

Interface de console (TV + controle) para jogos no Linux. **Não é uma distro**: é um software instalável que
traz uma sessão própria, uma Shell em tela cheia, um Core que cuida de jogos, apps e do sistema, e o celular como controle.

Versão atual: **0.5.0**.

```
Shell (Electron + SolidJS) ⇄ Core (Rust) ⇄ Providers (Steam · Apps Linux · Streaming · Emuladores · Google Drive) ⇄ Steam / .desktop / navegador / RetroArch / rclone
       ▲                       ▲
       │ JSON-RPC (socket Unix)│ HTTP+WS (rede local, só o Remote)
       └── Celular (PWA): controle, mouse, teclado, envio de ROMs
```

**Continuando o desenvolvimento (outra sessão/agente): leia [HANDOFF.md](HANDOFF.md).**
Plano completo: [docs/architecture/ARCHITECTURE.md](docs/architecture/ARCHITECTURE.md) · estado atual: [docs/STATUS.md](docs/STATUS.md)

## O que tem hoje

- **Início, Biblioteca, Loja, Busca e Hub:** jogos da Steam (com a loja da Steam por dentro do Nexus), apps Linux que você
  adicionar, streaming (YouTube TV e outros sites pelo navegador) e os jogos de console.
- **Emuladores:** RetroArch com os núcleos de 42 consoles; ROMs em `~/ROMs/<console>`, em pendrive/HD externo ou no Google Drive.
- **Mídia:** filmes, séries e músicas da pasta `Nexus` do Google Drive (e das pastas Vídeos e Música da TV), tocados em tela cheia pelo mpv.
- **Google Drive:** login pela própria TV, pasta `Nexus` criada no seu Drive, ROMs na nuvem que abrem sem ocupar espaço até você jogar.
- **Central rápida (Guia, Select+Start ou F1):** volume, Wi-Fi, Bluetooth, perfil, retomar, início, reiniciar e desligar — por cima do jogo.
- **Barra de status e notificações:** rede, Bluetooth, som e bateria dos controles; avisos curtos, com "silenciar durante jogos".
- **Ajustes de sistema:**
  - Rede (Wi-Fi, rede oculta, IP fixo com PIN) e Bluetooth (controles, fones, teclados);
  - áudio (saída, volume, teste de microfone) e tela (resolução, taxa, escala, rotação, margem e VRR), com confirmação de 15 s;
  - idioma, fuso e teclado; energia (suspender, desligar, apagar a tela por inatividade, o que o botão físico faz);
  - armazenamento: uso do disco, discos USB (abrir, ejetar com segurança, formatar com PIN), limpeza de cache;
  - TV por HDMI-CEC (ligar e desligar a TV junto, quando há adaptador);
  - atualizações dos apps (Flatpak, Snap e apt) e do Nexus; recuperação (reiniciar a sessão, voltar ao desktop, reset) atrás do PIN;
  - **SSH:** liga e desliga o servidor SSH (com PIN) e mostra, na própria TV, o passo a passo para entrar pelo computador;
  - sobre e diagnóstico (modelo, CPU, memória, GPU, temperatura) com relatório de suporte para enviar pelo celular;
  - aparência (texto grande, fundo), acessibilidade (leitor de tela), privacidade (o que vai pela rede; limpar histórico de jogo).
- **Perfis e controle parental:** perfil infantil com limite de tempo e classificação, PIN dos responsáveis com código de recuperação,
  relatório de uso por perfil.
- **Celular como controle:** QR code, gamepad para dois jogadores, mouse, teclado e rolagem; instalável na tela inicial do celular;
  página de envio de ROMs (celular ou computador, arrastar e soltar).
- **Primeiro uso:** assistente com boas-vindas, idioma, rede, controle, perfil, PIN, jogos, celular, Google Drive e Steam.

**Fases 2, 3 e 4 das telas de sistema** estão implementadas (ver [docs/STATUS.md](docs/STATUS.md) para o que foi testado e o que
depende de hardware real, como Raspberry Pi, TV com CEC, pendrive e Wi-Fi de verdade).

## Como instalar

Funciona em **PC** (Debian/Ubuntu x86_64) e no **Raspberry Pi 4/5** (Raspberry Pi OS 64 bits ou Debian/Ubuntu arm64).
O instalador já traz **tudo**: o Nexus, a Steam (só no PC — não existe Steam para ARM), o RetroArch com os emuladores,
um navegador para o streaming (se não houver), o Bluetooth, o mpv, o rclone para o Google Drive e as bibliotecas do sistema.

Requisitos: 4 GB de memória e 6 GB livres no PC; 2 GB e 3 GB no Raspberry Pi; internet durante a instalação.

**Raspberry Pi com Raspberry Pi OS Lite (sem interface):** use a opção 1 com `--autologin` (o instalador põe sozinho o greetd, a tela
de login leve). Passo a passo completo e **roteiro de testes no aparelho real** em [docs/TESTE-REAL.md](docs/TESTE-REAL.md).

### Opção 1 — um comando (recomendado)
Para **instalar** (ou atualizar — é o mesmo comando):
```sh
curl -fsSL https://raw.githubusercontent.com/GuiVicS/nexus/main/tools/update.sh | sh
```
Opções da primeira instalação (depois de `| sh -s --`): `--autologin SEU_USUARIO` (liga direto no Nexus),
`--sem-steam`, `--sem-emuladores`. Ex.: `curl -fsSL .../update.sh | sh -s -- --autologin gui`.

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
- **ROMs:** coloque em `~/ROMs/<console>` (cada pasta tem um `LEIA-ME.txt` com as extensões aceitas), ou envie pelo celular.
- **Google Drive (opcional):** Ajustes → Armazenamento → Google Drive → Entrar. Os arquivos ficam na pasta `Nexus` do seu Drive.
- **Steam (PC):** na primeira vez o Nexus mostra o QR code — escaneie com o app Steam no celular.
- **Celular como controle:** Ajustes → Controle no celular → escaneie o QR (mesma rede Wi-Fi).
- **SSH (opcional):** Ajustes → Acesso por SSH; a própria TV mostra os passos para entrar pelo computador.
- **Emergência:** Ctrl+Alt+F3 abre um terminal; `sudo touch /etc/nexus/force-desktop` volta ao desktop normal no próximo login.
- **Desinstalar:** `sudo nexus-installer revert-autologin ; sudo apt remove nexus`

## Atualizar
O mesmo comando da instalação (só troca o pacote; mantém perfis, jogos e configurações):
```sh
curl -fsSL https://raw.githubusercontent.com/GuiVicS/nexus/main/tools/update.sh | sh
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
cargo test --workspace   # testes unitários e de ponta a ponta do Core, instalador e tipos
make e2e                 # cenários de interface contra o Core real (Chromium headless)
```

## Estrutura
`core/` Core em Rust · `apps/shell` Shell · `apps/remote` controle no celular · `packages/` protocolo, foco e tema ·
`installer/` instalador · `system/` sessão, sway, systemd, polkit, udev, AppArmor, pacote · `tools/` · `tests/`
