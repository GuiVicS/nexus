# HANDOFF — Nexus

> Documento para continuar o desenvolvimento em **outra sessão ou outro agente** sem perder contexto.
> Atualize a seção "Estado atual" e "Próximos passos" **ao fim de cada sessão**. Última atualização: 2026-11-10 (versão 0.3.2).

## 1. O que é

**Nexus** é um software instalável (não é uma distro) que transforma uma distro Linux comum numa experiência de console:
sessão Wayland própria, interface de TV em tela cheia navegável por controle, biblioteca unificada (Steam + apps Linux),
execução de jogos reais que assumem a tela e devolvem ao Nexus ao fechar, e o **celular como controle/mouse/teclado** (QR code).

Plano de arquitetura completo (14 seções, aprovado pelo usuário): [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md).
Estado por área, com legenda de confiança: [`docs/STATUS.md`](docs/STATUS.md). Decisões: [`docs/decisions/`](docs/decisions).

## 2. Sobre o usuário e como trabalhar com ele

- Fala **português (pt-BR)**; respostas curtas e diretas, sem enrolação. Toda a interface do produto é em pt-BR.
- Deu autonomia total ("faça bem feito", modo automático), mas **quer saber o que não foi testado**. Seja honesto sobre o que
  só foi validado aqui (sem GPU, sem Steam real) e o que depende do aparelho dele.
- Testa numa **VM VirtualBox com Ubuntu** (a VM usa repositório `resolute` = Ubuntu mais novo que o 24.04 alvo; aceleração 3D da VM pode estar desligada).
  Depois continuará direto no aparelho que receberá o sistema.
- **Não faça commit, push, deploy ou publicação sem pedir** (ele já autorizou explicitamente o que está em "Distribuição").
- Preferiu **recomendações tecnicamente honestas** a seguir hábitos; decisões de arquitetura estão justificadas em `docs/`.

## 3. Arquitetura em uma tela

```
Shell (Electron + SolidJS, apps/shell) ⇄ Core (Rust, core/crates/nexus-core) ⇄ Providers ⇄ Steam / .desktop / demo
        ▲  JSON-RPC por socket Unix (linhas)       ▲ HTTP+WebSocket na rede local, SÓ o Remote (celular)
        └─ Remote (PWA, apps/remote): gamepad P1/P2, mouse, teclado → uinput (ou eventos p/ a Shell sem uinput)
Sessão: nexus.desktop → nexus-session → sway kiosk (ws1 = Shell, ws2 = jogos) + units systemd --user
```

- **Core** é a única parte com lógica de sistema. A Shell só envia IDs/intenções; permissões (perfil infantil, PIN, limite de tempo) são decididas no Core.
- **Protocolo**: tipos Rust em `core/crates/nexus-protocol` → TypeScript gerado por `ts-rs` (`pnpm gen`) em `packages/protocol/src/generated`. Cliente/API tipada em `packages/protocol/src/{client,api,events}.ts`. Códigos de erro de aplicação 1001–1011 em `rpc.rs` (espelhados em `client.ts`).
- **Providers** (`core/crates/nexus-core/src/providers/`): `steam.rs` (ACF/VDF, progresso, loja pública, Web API opcional), `native.rs` (apps Linux **registrados pelo usuário**), `demo.rs` (catálogo fictício, só com `--demo`).
- **Shell**: `apps/shell/src` — `lib/` (estado `state.ts`, ações `actions.ts`, entrada `input.ts`, sessão/eventos `session.ts`), `components/` (telas). Motor de foco XY em `packages/focus`, tema/ícones/sons em `packages/ui`.
- **Supervisor de jogos**: `game.rs` — lança em sessão própria (`setsid`), acompanha por processos (`/proc`; Steam via `reaper … SteamLaunch AppId=N`), fecha com SIGTERM→SIGKILL, registra tempo de jogo e limite diário infantil.
- **Segurança**: socket 0600; HTTP local só com `Host` loopback (anti DNS-rebinding); Remote com token de 96 bits + bloqueio por IP; PIN com Argon2 e bloqueio por tentativas; imagens personalizadas validadas por conteúdo e restritas a pastas permitidas (`customart.rs`); Shell sem Node (`contextIsolation`, sandbox, CSP).

## 4. Estado atual (0.3.0)

**Funciona e tem teste automatizado** (ver §6): perfis/PIN/controle parental/tempo diário, perfil infantil, supervisor de jogos,
provedor Steam (dados locais + loja pública + detalhes), apps Linux registrados com nome/tipo/ícone/capa/fundo personalizados e navegador de imagens,
celular (QR, gamepad, mouse, teclado, rolagem), Shell completa (Início, Biblioteca, Mídia, Loja, Hub, Busca, Central, Notificações, Ajustes), instalador (plano), pacote `.deb`.

**Novidades da 0.3.0 (2026-11-10):** foto de perfil escolhida do disco (`avatars.rs`: `profiles.avatar_import`,
avatar `photo:<arquivo>` validado e servido em `/art/photo:<arquivo>/photo`; na tela de perfis sem sessão o navegador de imagens pede o PIN dos
responsáveis), item "Voltar ao Início" nos Ajustes, modo simulação (§5). Ambiente da VM do usuário: Rust/Node 24/pnpm/gh instalados no usuário
(`~/.cargo/bin`, `~/.local/bin`), sem sudo pelo Claude; `chromium` é um link para o Google Chrome.

**Teste real na VM (2026-11-10, Steam real logada, sessão sway aninhada com `WLR_BACKENDS=wayland nexus-session`):**
- Validado: sessão sobe no Nexus; Steam (login no modo TV com QR, depois `-silent` em escopo próprio); busca/instalação/abertura/fechamento de jogo real
  (AdVenture Capitalist); Central por cima do jogo (quadro congelado via `grim`, tecla Super = Guia via `nexus-core guide`); botões do Nexus
  sobrepostos a qualquer jogo/app (`nexus-overlay`, Python+GTK+gtk-layer-shell, camada OVERLAY); streaming (YouTube TV abre); RetroArch (ROM de GB
  abriu, núcleo baixado sozinho, fechar envia SIGTERM à árvore → sem SIGKILL); mapeamento de controle com o celular emulado (uinput) e perfil
  RetroArch gerado.
- Corrigido no teste: `StopWhenUnneeded` no target; `[class=".*"]` no sway 1.11 casava a Shell; Steam "não encontrada" sem pasta de dados;
  pergunta "Install" do launcher Debian (shim de zenity); diálogo "Install" da Steam escondido atrás da janela em tela cheia; `steam://install`
  pede confirmação (spike S1 **invalidado**: o Core traz a Steam à frente até o download começar); busca não achava a loja; Instalar a partir da
  busca não fazia nada; reiniciar o Core matava Steam/jogo (agora escopos systemd) e perdia o jogo (agora `runtime/game.json` + readoção);
  grim do Ubuntu sem JPEG; `install.sh --plano`; testes viam a Steam real (isolado em `for_test`); `run.mjs` não contava exceções.
- **Não testado**: Bluetooth (VM sem adaptador — código em `controllers.rs`, `bluetoothctl` com argv fixo); reservas de jogador no RetroArch
  (`input_playerN_reserved_device`, formato não confirmado na prática); Netflix/Prime com DRM; sessão pelo login real (só aninhada).
- **Corrigidos depois (mesma data):** apport ao parar a Shell (SIGTERM → `app.quit()` + `KillMode=mixed`); capas da loja (cliente HTTP único
  com conexões reaproveitadas, um download por arquivo, pré-carga de 6 em 6 ao abrir a loja: 104 capas em ~6 s); reiniciar o Core mantém o perfil
  (`runtime/session`, tmpfs 0600, some ao reiniciar o aparelho); efeitos mínimos sozinhos sem GPU (Shell lê o renderizador WebGL: llvmpipe/SwiftShader;
  Core reconhece GPU virtual e de ARM pelo driver: vc4/v3d = Pi → "reduzidos"); anel de foco não aparece fora da camada do topo e não desliza com
  efeitos mínimos (antes ficava na linha atrás do teclado na tela); títulos longos no Hub com fonte menor (não invadem as abas); diálogo
  "<Jogo> Configuration" do Unity confirmado sozinho (Enter pelo uinput na sessão real + clique em "Play!" pelo cursor do sway — **validado** no
  AdVenture Capitalist); dica "Guia [Tab]" trocada por "Guia, Super ou ☰".
- **Raspberry Pi (arm64):** `tools/build-deb.sh arm64` / `tools/bundle.sh arm64` (compilação cruzada: `gcc-aarch64-linux-gnu` + `rustup target add
  aarch64-unknown-linux-gnu`; Electron arm64 oficial baixado e conferido por SHA-256 em `~/.cache/nexus-build`). `glibc_compat.rs` nos binários
  (`pidfd_spawnp`/`pidfd_getpid` locais) baixa a exigência de glibc de 2.39 para **2.34** (Pi OS Bookworm = 2.36; vale também para o PC: Ubuntu 22.04
  / Debian 12). Instalador aceita aarch64 (Debian/Pi OS/Ubuntu = experimental; 32 bits não), pula a Steam (não existe Steam Linux para ARM),
  mínimo 2 GB de RAM/3 GB de disco no ARM, autologin no **LightDM** (Pi OS). Núcleos do RetroArch vêm de `linux/aarch64` no buildbot (os 36 existem).
  Sem Steam possível a aba Loja some. **Validado só com QEMU** (`qemu-aarch64 -L /usr/aarch64-linux-gnu`): testes do Core (56 + 13 e2e) e do
  instalador (12) passam em ARM; **nunca rodou num Pi de verdade** (GPU V3D, desempenho, sway/Electron no Pi OS).
- Abertos: o 1º e2e completo sob QEMU falhou 1 vez por tempo (passou em 3 repetições seguintes); `systemctl --user stop nexus-shell` derruba a sessão
  inteira (target com `Requires=`; travamentos reais não propagam); na sessão aninhada o teclado virtual vai para a sessão de fora (por isso o Enter
  do Unity só é usado com saídas reais, `compositor::sway_nested`).

**Novos módulos:** `providers/web.rs` (streaming, perfil de navegador em `<dados>/web-profile`, `NEXUS_WEB_APPS=0` desliga), `providers/emu.rs`
(42 consoles, `~/ROMs/<console>`, núcleos do buildbot, capas libretro-thumbnails, `NEXUS_EMULATION=0` desliga), `controllers.rs` (lista, captura,
cadastro com apelido/jogador 1–4, mapa de 25 ações, autoconfig RetroArch, Bluetooth), `system/overlay/nexus-overlay` + `nexus-overlay.service`.
Cards 208×312 (2:3, capas da Steam). Ajustes ganhou "Emuladores" e "Controles" (conectados, Bluetooth, USB).

**Comportamentos decididos pelo usuário (não reverter sem perguntar):**
- A **Steam não aparece como jogo**; só os jogos dela.
- Apps do sistema (calculadora, Firefox…) **não entram sozinhos**: só se adicionados em *Ajustes → Apps e jogos* (ou instalados pelo Nexus). Cada um pode ter nome, tipo (Jogo/App), ícone, **capa do card** e **capa de fundo** próprios.
- **Loja** mostra jogos da Steam para instalar (mais vendidos, ofertas, grátis, nativos Linux, lançamentos). Pago → abre a página na Steam ("Ver na Steam"); grátis ou da conta → instala (`steam://install/N`). A Steam é o "pano de fundo" controlado pelo Nexus.
- Lista de jogos **da conta ainda não instalados**: precisa de **chave da Web API** (Ajustes → Steam → Chave) com perfil público; sem chave, só aparecem jogos já abertos antes (lidos de `localconfig.vdf`, nome resolvido via loja). A Steam fechou os caminhos sem chave (testado).

**NÃO validado em hardware/ambiente real** (principais riscos, nesta ordem):
1. **Sessão Nexus no login** (`nexus-session`, sway kiosk, units): já corrigido o que dava para achar (config inválida do sway, `WAYLAND_DISPLAY` no systemd, log em `~/.local/state/nexus/session.log`), mas **nunca confirmado funcionando no Ubuntu do usuário**. Último relato: ficava em tela cinza/preta do sway sem a Shell (versão 0.1.1); a 0.1.2 passou o ambiente Wayland ao systemd — resultado ainda não informado.
2. **Abrir jogo Steam real** (usuário relatou "não abre"): corrigido caminho `/usr/games/steam`, Flatpak, janela da Steam visível após 7 s, motivo do erro no aviso. Não reproduzido aqui. Jogos de Windows exigem Steam Play/Proton ativado.
3. `steam://install/N` sem diálogo de confirmação (spike S1 do plano).
4. `/dev/uinput` (celular como controle **dentro dos jogos**) — sem `/dev/uinput` aqui; testado só o modo "celular controla o menu".
5. RAM do Electron: ~750 MB medidos sob Xvfb/GL por software (meta 400 MB; gate S4 do plano → plano B Qt/QML).
6. Botão Guia com jogo aberto (evdev), NVIDIA, Fedora/Arch, `install.sh`/apt/flatpak num sistema real.

**Não implementado** (planejado): Nexus Sysd (ações privilegiadas via D-Bus+polkit — só existe a política polkit), atualização dentro da interface, gamescope, SteamGridDB/IGDB, multiusuário avançado.

## 5. Como rodar (desenvolvimento)

Requisitos: Rust (rustup), Node 24, pnpm, Chromium (para os testes). Para o pacote do Raspberry Pi: `gcc-aarch64-linux-gnu` e o alvo `aarch64-unknown-linux-gnu`. O ambiente onde isto foi feito é headless (sem GPU/Steam).

```sh
pnpm install
pnpm gen                 # regenera tipos TS a partir do Rust (rode ao mudar nexus-protocol)
make build               # cargo build + build da Shell e do Remote
make dev-core            # Core em modo demonstração + Shell em http://127.0.0.1:8799 (PIN dos responsáveis na demo: 1234)
```

**Modo simulação (preferido para testar à mão, pedido do usuário):** `make sim` (ou `tools/sim.sh`; `make sim-reset` apaga tudo).
Roda o Core real **sem** `--demo`: começa no primeiro uso, Steam simulada (`tools/sim/steam`: conta, 4 jogos instalados, 3 já jogados
para instalar, download com progresso nos `.acf`, jogo = processo `SteamLaunch AppId=N`), loja = vitrine pública real, apps = os da máquina.
Dados em `~/.local/share/nexus-sim`, log da Steam simulada em `~/.local/share/nexus-sim/steam.log`. O `make dev-core` (catálogo fictício) continua para os testes.

Variáveis úteis do Core (`config.rs`): `NEXUS_DEMO=1`, `NEXUS_DEV_HTTP`, `NEXUS_STEAM_ROOT`, `NEXUS_STEAM_BIN`, `NEXUS_STEAM_API_KEY`, `NEXUS_APPS_DIRS`, `NEXUS_FS_ROOTS`, `NEXUS_UINPUT=0`, `NEXUS_COMPOSITOR=sway|none`, `NEXUS_REMOTE_BIND`.
Ganchos de teste: `NEXUS_OFFLINE=1` (sem rede) + `NEXUS_STEAM_FIXTURE=arquivo.json` (vitrine fictícia `{categoria:[entradas]}`).

## 6. Testes (rode tudo antes de dar algo como pronto)

```sh
cargo test --workspace                       # ~49 unitários + 12 e2e do Core + 10 do instalador + 28 de tipos
node --experimental-strip-types packages/focus/test.mjs
pnpm -r typecheck
node tests/e2e/run.mjs [filtro]              # 7 cenários de interface (Chromium headless contra o Core real)
```
Cenários: `adulto_demo`, `crianca`, `perfis`, `primeiro_uso`, `celular`, `apps_personalizados`, `steam_loja`.
Os cenários de UI são **sensíveis a carga** (máquina de 2 núcleos): um deles falhou 1 vez em 3 rodadas completas e passa 6/6 isolado; se falhar, repita antes de supor bug real. Prefira `until(...)`/esperas ao invés de `sleep` fixo.

## 7. Build, empacotar e distribuir

- `tools/build-deb.sh [amd64|arm64]` → `dist/nexus_<versão>_<arq>.deb` (Core + Shell/Electron + Remote + sessão + units + polkit + udev + AppArmor). A versão vem do `Cargo.toml` (workspace).
- `tools/bundle.sh [amd64|arm64]` → `dist/nexus-<versão>-linux-{x86_64|arm64}.tar.gz` (+ `.sha256`; `update.sh` escolhe pela arquitetura) contendo o `.deb` e `install.sh`. **`dist/` não vai para o git.**
- `install.sh` (Debian/Ubuntu): instala o `.deb` e chama `nexus-installer install --steam --yes` (habilita 32 bits + multiverse, instala Steam; `--emulation` → Flatpak+Flathub+RetroArch; `--autologin USUARIO`; `--plano` só mostra).
- **GitHub Releases (pedido do usuário, a partir da 0.3.0):** `gh release create vX.Y.Z` com os `.tar.gz` + `.sha256` de PC e Raspberry Pi; o README aponta para `releases/latest`.
- **Hospedagem do download (decidida pelo usuário):** projeto Vercel `nexus-download` na conta `serafim-company`, pasta local `/root/nexus-download` (fora do repo): `index.html`, `latest.txt`, `update.sh` (cópia de `tools/update.sh`), o `.tar.gz`, `.sha256` e o código-fonte. Publicar: copiar os arquivos novos, atualizar `latest.txt`/`index.html` e `vercel deploy --prod --yes`. URL: `https://nexus-download-psi.vercel.app`.
- **Instalar/atualizar no aparelho (0.3.1+):** `tools/update.sh` baixa da **release mais recente do GitHub** (arquitetura pela `dpkg`,
  confere sha256). Sem o Nexus instalado roda o `install.sh` completo; com ele, só `apt install` do `.deb`. O repositório é **privado**:
  o link aberto dá 404, então o script usa `gh release download` (precisa de `gh auth login`); o próprio script é obtido com
  `gh api repos/GuiVicS/nexus/contents/tools/update.sh -H "Accept: application/vnd.github.raw" | sh`. O site da Vercel ficou parado na 0.2.0.
- **Instalação completa por padrão (pedido do usuário, 0.3.1):** `install.sh` = Steam (PC) + RetroArch (`--sem-steam`/`--sem-emuladores`
  para tirar) + navegador se não houver (plano: `chromium`, ou `chromium-browser` no Ubuntu) + `bluez`/`curl` nas dependências do `.deb`.
- Ao mudar versão: `Cargo.toml`, `README.md`; depois bundle → copiar → `latest.txt` → deploy.

## 8. Diagnóstico no aparelho (peça ao usuário nesta ordem)

```sh
dpkg -l nexus | tail -1                  # versão instalada
nexus-installer doctor                   # ✓/✗ de sessão, core, uinput, steam, sway
cat ~/.local/state/nexus/session.log     # o que o nexus-session fez (sway abriu? renderer? falhou onde?)
journalctl --user -b -u nexus-shell -u nexus-core --no-pager | tail -40
cat ~/.cache/nexus/logs/steam_*.log      # saída ao iniciar um jogo da Steam
# Teste da interface sem a sessão especial (janela comum):
nexus-core & sleep 2; NEXUS_KIOSK=0 /opt/nexus/shell/nexus-shell --no-sandbox --disable-gpu
```
Saídas de emergência: Ctrl+Alt+F3 (terminal; na VirtualBox Right-Ctrl+F3), `sudo touch /etc/nexus/force-desktop` (próximo login volta ao desktop normal), a sessão desfaz o autologin sozinha após 3 falhas em 5 min.

## 9. Armadilhas já encontradas (economize tempo)

- `sway` rejeita diretivas desconhecidas e **sai** (volta ao login). Valide sempre: `XDG_RUNTIME_DIR=/tmp/xr WLR_BACKENDS=headless sway -C -c system/sway/nexus-kiosk.conf` (a linha `titlebar_padding` era inválida).
- Serviços `systemd --user` **não herdam** `WAYLAND_DISPLAY`/`SWAYSOCK`/`PATH`: o `exec` do kiosk faz `import-environment` antes de iniciar `nexus-session.target`. No Debian/Ubuntu a Steam é `/usr/games/steam` (fora do PATH de serviços).
- Sandboxing agressivo no `.service` de usuário (`ProtectSystem`, `MemoryDenyWriteExecute`…) pode impedir o serviço de subir; foi removido.
- No sway kiosk, **toda janela nova vai para o workspace 2**; o Core só troca de workspace quando detecta o jogo (ou após 7 s na Steam). Janelas de login/diálogo da Steam ficam invisíveis se o Core não trouxer o ws2 à frente.
- Electron único (`requestSingleInstanceLock`): uma instância antiga sobrando faz a nova sair **em silêncio**.
- `pkill -f padrão` dentro de um `bash -c` mata o próprio shell (o padrão aparece na linha de comando); use `pgrep -f "[p]adrão" | xargs kill`.
- Estado `Solid`: o foco do anel é recalculado quando o elemento focado some do DOM (`onLost` em `packages/focus`); overlays restauram o foco anterior (`ret`).
- A Steam **não** oferece API aberta para "jogos que possuo" sem chave (testado: `GetAppList`, perfil em XML, etc.). A vitrine pública (`featuredcategories`, `search/results?json=1`, `appdetails`) funciona sem chave; a busca não traz `appid` (extraído da URL da imagem).
- **Capas da loja (0.3.2):** jogos novos da Steam usam endereço com hash (`store_item_assets/steam/apps/<id>/<hash>/library_capsule_2x.jpg`); o endereço fixo dá 404. O Core resolve pela API pública `IStoreBrowseService/GetItems/v1` (`include_assets`, sem chave) em `providers/steam.rs` (`resolve_assets`/`parse_assets`, `art_late` quando o download falha); DLC sem capa vertical cai em `hero_capsule`/`header`.
- Testes e2e em paralelo: um Core adotava o processo falso "SteamLaunch AppId" de outro teste (launch dava Busy). A adoção de jogo Steam em execução só acontece se o Core tem uma Steam (`root()`).
- `nexus-session` (0.3.2) encerra o grupo do app que sobrou de uma sessão anterior (`game.json` → `kill -- -sid`) antes de subir o Core.
- Teste de interface: o campo de PIN/texto precisa de uma pausa depois de abrir; setas ◀▶ em seletores alteram valores (navegue na vertical em formulários).

## 10. Próximos passos sugeridos (prioridade)

1. **Fechar o ciclo no aparelho do usuário:** confirmar a sessão (`session.log`), abrir um jogo Steam real, ajustar o que aparecer. Tudo o resto depende disso.
2. **Testar num Raspberry Pi real** (pacote arm64 só validado com QEMU): sessão, GPU V3D, RAM, emuladores leves, Chromium com DRM.
3. ~~Apps web~~ e ~~emulação~~ feitos na 0.3.0; falta URL personalizada em *Apps e jogos* e identificar console por hash.
4. **Nexus Sysd** (D-Bus + polkit, ações com argv fixo): instalar/atualizar provedores pela interface sem terminal.
5. Spike S1 (instalar jogo Steam sem diálogo; talvez via porta CEF 8080 da Steam — não documentado, tratar como experimento) e medir RAM/FPS reais (gate S4).
6. Repositório apt próprio (hoje a distribuição é `.deb` solto), imagem/Pi só depois (V4).

## 11. Mapa rápido de arquivos

```
core/crates/nexus-core/src/  accounts.rs db.rs game.rs rpc.rs net.rs remote.rs vdev.rs inputmap.rs customart.rs art.rs hardware.rs procs.rs guide.rs
                             providers/{mod,steam,native,demo}.rs   tests/e2e.rs (Core de ponta a ponta)
apps/shell/src/              main.tsx styles.css lib/{state,actions,input,session,api,format,flows}.ts components/{App,Views,Hub,Overlays,Settings,Profiles,Apps,Entry,ui}.tsx
apps/shell/electron/         main.cjs preload.cjs   (ponte segura com o Core; protocolo nexus-art://)
apps/remote/src/             App.tsx Gamepad.tsx Touchpad.tsx link.ts remote.css   (celular)
packages/{protocol,focus,ui} installer/src/{main,detect,plan,autologin}.rs
system/                      session/ sway/ systemd/ polkit/ udev/ apparmor/ packaging/
tools/                       build-deb.sh bundle.sh update.sh extract-demo-catalog.mjs gen-protocol-index.mjs
tests/e2e/                   lib.mjs scenarios.mjs run.mjs cdp.mjs
assets/demo/                 catálogo e imagens fictícias do modo demonstração
```

## 12. Origem do projeto (contexto)

Nasceu de um protótipo visual em HTML único (`console-os.html`, repositório separado *retropad*, ainda hospedado como PWA em `https://nexuos-console.vercel.app`) e reaproveita do **RetroPad** apenas a ideia do celular como gamepad/mouse/teclado (redesenhada no tema do Nexus). A identidade visual (turquesa sobre grafite, modo claro, sons, cursor temático) vem desse protótipo e está em `packages/ui/src/theme.css`.
