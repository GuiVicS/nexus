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

## Baixar e instalar
Pacotes prontos em **[Releases](https://github.com/GuiVicS/nexus/releases/latest)**:

| Aparelho | Arquivo |
|---|---|
| PC (Debian/Ubuntu x86_64) | `nexus-0.3.0-linux-x86_64.tar.gz` |
| Raspberry Pi 4/5 (Raspberry Pi OS 64 bits, Debian/Ubuntu arm64) | `nexus-0.3.0-linux-arm64.tar.gz` |

```sh
tar xzf nexus-0.3.0-linux-<arquitetura>.tar.gz && cd nexus-0.3.0
sudo ./install.sh                                # Nexus (+ Steam no PC; no ARM a Steam não existe)
sudo ./install.sh --emulation                    # opcional: RetroArch (emuladores, 42 consoles)
sudo ./install.sh --autologin SEU_USUARIO        # opcional: entrar direto no Nexus
```
Na tela de login escolha a sessão **Nexus**. Emergência: Ctrl+Alt+F3 (terminal) ou `sudo touch /etc/nexus/force-desktop`.
ROMs vão em `~/ROMs/<console>` (cada pasta tem um `LEIA-ME.txt`).

Gerar os pacotes a partir do código (precisa de Rust, Node 24, pnpm):
```sh
tools/bundle.sh          # PC  → dist/nexus-0.3.0-linux-x86_64.tar.gz
tools/bundle.sh arm64    # Pi  → dist/nexus-0.3.0-linux-arm64.tar.gz (requer gcc-aarch64-linux-gnu e o alvo aarch64 do Rust)
```

## Atualizar
```sh
curl -fsSL https://nexus-download-psi.vercel.app/update.sh | sh
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
