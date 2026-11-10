# Nexus

Interface de console (TV + controle) para jogos no Linux. **Não é uma distro**: é um software instalável que
traz uma sessão própria, uma Shell fullscreen, um Core que cuida de biblioteca/jogos e o celular como controle.

```
Shell (Electron + SolidJS) ⇄ Core (Rust) ⇄ Providers (Steam · Apps Linux · Demo) ⇄ Steam / .desktop
       ▲                       ▲
       │ JSON-RPC (socket Unix)│ HTTP+WS (rede local, só o Remote)
       └── Celular (PWA): controle, mouse, teclado
```

Plano completo: [docs/architecture/ARCHITECTURE.md](docs/architecture/ARCHITECTURE.md) · estado atual: [docs/STATUS.md](docs/STATUS.md)

## Instalar (Debian/Ubuntu x86_64)
```sh
tools/build-deb.sh                       # gera dist/nexus_0.1.0_amd64.deb (precisa de Rust, Node 24, pnpm)
sudo apt install ./dist/nexus_0.1.0_amd64.deb
sudo nexus-installer install --steam --yes   # opcional: Steam; acrescente --autologin SEU_USUARIO
```
Na tela de login escolha a sessão **Nexus**. Saída de emergência: Ctrl+Alt+F3 (terminal) ou `touch /etc/nexus/force-desktop`.

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
