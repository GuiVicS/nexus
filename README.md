# Nexus

Interface de console (TV + controle) para jogos no Linux. **Não é uma distro**: é um software instalável que
traz uma sessão própria, uma Shell fullscreen, um Core que cuida de biblioteca/jogos e o celular como controle.

```
Shell (Electron + SolidJS) ⇄ Core (Rust) ⇄ Providers (Steam · Apps Linux · Demo) ⇄ Steam / .desktop
       ▲                       ▲
       │ JSON-RPC (socket Unix)│ HTTP+WS (rede local, só o Remote)
       └── Celular (PWA): controle, mouse, teclado
```

**Continuando o desenvolvimento (outra sessão/agente): leia [HANDOFF.md](HANDOFF.md).**
Plano completo: [docs/architecture/ARCHITECTURE.md](docs/architecture/ARCHITECTURE.md) · estado atual: [docs/STATUS.md](docs/STATUS.md)

## Instalar (Debian/Ubuntu x86_64)
Gere o pacote único e leve-o ao Linux:
```sh
tools/bundle.sh          # gera dist/nexus-0.2.0-linux-x86_64.tar.gz (precisa de Rust, Node 24, pnpm)
```
No Linux:
```sh
tar xzf nexus-0.2.0-linux-x86_64.tar.gz && cd nexus-0.2.0
sudo ./install.sh                                # Nexus + Steam
sudo ./install.sh --autologin SEU_USUARIO        # opcional: entrar direto no Nexus
sudo ./install.sh --emulation                    # opcional: RetroArch (Flatpak)
```
Na tela de login escolha a sessão **Nexus**. Emergência: Ctrl+Alt+F3 (terminal) ou `sudo touch /etc/nexus/force-desktop`.

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
cargo test --workspace   # 50+ testes (unitários e de ponta a ponta do Core)
make e2e                 # cenários de interface (adulto, criança, perfis, primeiro uso, celular)
```

## Estrutura
`core/` Core em Rust · `apps/shell` Shell · `apps/remote` controle no celular · `packages/` protocolo, foco e tema ·
`installer/` instalador · `system/` sessão, sway, systemd, polkit, udev, AppArmor, pacote · `tools/` · `tests/`
