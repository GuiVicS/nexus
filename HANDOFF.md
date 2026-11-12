# HANDOFF — Nexus

> Documento para continuar o desenvolvimento em **outra sessão ou outro agente** sem perder contexto.
> Atualize a seção "Estado atual" e "Próximos passos" **ao fim de cada sessão**. Última atualização: 2026-11-12 (versão 0.4.1 publicada; **0.5.0 em preparação**: fases 2, 3 e 4 das telas de sistema e aba Mídia, ver §4b e a seção final de `docs/STATUS.md`).

**Repositório (público desde a 0.4.1):** https://github.com/GuiVicS/nexus · releases: https://github.com/GuiVicS/nexus/releases
· este arquivo em texto puro: https://raw.githubusercontent.com/GuiVicS/nexus/main/HANDOFF.md

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

## 4. Estado atual (0.4.1)

> **Atualização para 0.5.0 (em preparação, 2026-11-12):** as fases 2, 3 e 4 da §4b e a aba Mídia (Google Drive parte 2)
> estão implementadas. O que cada item cobre e o que **não** foi testado em hardware real está em `docs/STATUS.md`
> (seção "Telas de sistema" e o resumo final). O que ainda falta de fato: login do Google Drive por QR no celular,
> controle remoto da TV via CEC, terminal embutido na Recuperação, outros idiomas da interface, e validação de tudo
> no aparelho real (Raspberry Pi, TV com HDMI-CEC, pendrive, Wi-Fi, Bluetooth, boot).

**0.4.0 "carenagem de console" (release `v0.4.0`, 2026-11-12, plano em `~/.claude/plans/jiggly-forging-ullman.md`):** a Steam e o RetroArch são só motor, nunca aparecem.
- **Steam escondida:** janelas da classe `steam` vão para o *scratchpad* do sway (`nexus-kiosk.conf`); `no_focus` impede janela nova de trocar a tela;
  o Core só mostra o jogo quando a **janela do jogo** existe há ~1,2 s (`compositor::game_window_present`, fallback 12 s) e, ao sair, reafirma o Nexus por 1,6 s.
  Login/compra: `core.show_steam_ui()` (Ajustes → Steam). Aviso "A Steam está preparando o jogo…" na Shell em vez de mostrar a Steam.
- **Menu rápido:** F1 (teclado), Guia ou **Select+Start** (controle, `guide.rs` via evdev), Super. Mesma tecla abre/fecha; jogo desfocado atrás;
  item "Ir ao Início" (jogo continua). Os botões flutuantes do `nexus-overlay` saíram; o overlay agora é só a **capa** "Instalando…".
- **Instalar sem ver a Steam:** `rpc.rs::steam_confirm_install` acha o diálogo (Install 540×480 / EULA 640×600, `compositor::classify_steam_dialog`),
  cobre com a capa (região de entrada vazia via ctypes) e clica pelo cursor do sway. **EULA é perguntada ao usuário na Shell** (`steam.eula`), nunca aceita sozinha.
- **RetroArch sem "RETROARCH":** `~/.config/nexus/retroarch-nexus.cfg` via `--appendconfig` (sem animação/avisos, menu só L3+R3, Select+Start livre).
- **Aba Emuladores** (`Emu.tsx`): consoles → jogos com capa; sem capa = arte do console + nome. ROMs saíram da Biblioteca.
- **Adicionar jogos** (`Roms.tsx` + `roms.rs`): pendrive (`lsblk`/`udisksctl`), Downloads/pasta pessoal, marcar e copiar para `~/ROMs/<console>/`;
  QR + link para `/roms?t=<token>` (página `roms_upload.html`, `POST /remote/upload`, 8 GB, mesmo token do controle no celular).
- **Loja estilo Play Store** (`Views.tsx::Store`): abas em pílula ("Para você" + categorias, `data-group` no foco faz quem entra cair na aba ativa),
  banners largos, prateleiras horizontais com "Ver tudo", atalho para adicionar ROMs. Página do jogo da loja: aba **Imagens** (capturas de `appdetails`
  servidas pelo Core em `/art/<id>/shot0..3`, a CSP da Shell não deixa carregar imagem externa) e descrição/gêneros buscados ao abrir.
- Validado na VM (sessão aninhada, screenshots): tudo acima, inclusive envio de ROM pela página do celular e cópia de pasta. **Não validado:** pendrive real,
  controle físico com botão Guia, jogo que pede EULA até o fim (Spacewar ficou sem resposta), Pi.
- **Atualizações dos apps (0.4.1):** `appupdates.rs` descobre a origem de cada app adicionado (Flatpak pelo
  `Exec=flatpak run`, Snap pela pasta `/var/lib/snapd/desktop` ou `/snap/bin`, apt por `dpkg-query -S`), procura a cada 6 h (`flatpak remote-ls
  --updates`, `snap refresh --list`, `apt list --upgradable`), avisa uma vez por versão (notificação → página do app) e marca o item com `update`.
  Página do app: botão **Atualizar**; Opções: **Restaurar versão anterior** e **Procurar atualizações**; Ajustes → Apps e jogos lista tudo.
  RPCs `apps.updates`, `apps.check_updates`, `apps.upgrade`, `apps.restore` (eventos `apps.updates`, `apps.job`).
  **Root:** Flatpak *do sistema* só atualiza sem senha; voltar versão (`--commit`) exige root (testado na VM: "Can't update to a specific commit
  without root permissions"). Por isso existe o ajudante `system/sysd/nexus-app-helper` + unidade `nexus-app@<update|restore>:<flatpak|snap|apt>:<nome>.service`
  + regra `system/polkit/50-nexus-apps.rules` (libera só essa unidade, verbo start, sessão local ativa). O ajudante escolhe a versão: Flatpak = commit
  pai, Snap = `snap revert`, apt = versão guardada por ele em `/var/lib/nexus/apps/<pkg>.prev` antes de atualizar. Flatpak do usuário vai direto.
  Sem o ajudante instalado (desenvolvimento), Snap/apt só avisam. Testes: `appupdates::tests` (inclui o script do ajudante com snap/apt falsos),
  e2e `app_updates_flatpak_update_and_restore`, cenário de interface `apps_atualizacoes`.
- **Dependências travadas (pedido do usuário):** RetroArch e núcleos só mudam de versão nos aparelhos quando o usuário atualiza aqui
  na máquina de desenvolvimento e sai uma release. `system/packaging/deps.lock` (gerado por `tools/pin-deps.sh`, nunca à mão) guarda o
  commit Flatpak do RetroArch (x86_64 = o instalado aqui; aarch64 = o commit do Flathub com o mesmo assunto) e o ID + SHA-256 dos pacotes
  de núcleos `dist/nexus-cores-<ID>-linux-{x86_64,arm64}.tar.gz` (PC = os núcleos desta máquina; ARM = baixados do buildbot no momento do
  pin e congelados). No aparelho, `/usr/lib/nexus/nexus-deps-apply` (root; postinst, instalador e `update.sh`) põe o RetroArch no commit
  travado e faz `flatpak mask` (nem `flatpak update` nem a Central de Programas mexem); `--cores ARQ` confere o SHA-256 e instala em
  `/usr/lib/nexus/cores`. O Core (`emu.rs::fetch_core`/`sync_pinned`) copia os núcleos dali para a pasta do RetroArch e troca os antigos
  quando o ID muda; só sem esse pacote (desenvolvimento) baixa do buildbot noturno. `appupdates::is_dependency` tira RetroArch, Steam e
  navegador das notificações e do Atualizar/Restaurar (o ajudante root também recusa). Steam se atualiza sozinha (Valve, não dá para
  travar); o navegador fica com as atualizações de segurança do sistema.
- Pendências pequenas: o diálogo de configuração do Unity ainda pisca; a capa "Instalando" fica sem título para jogo fora da biblioteca.


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
- **0.3.1:** `install.sh` instala tudo por padrão (Steam no PC, RetroArch, navegador se faltar); `tools/update.sh` baixa da release do GitHub.
- **0.3.2 (release `v0.3.2`, commit `c55285f`):** capas da Loja resolvidas pela API do catálogo da Steam (0 faltando na VM, validado na tela);
  adoção de jogo Steam só com Steam presente (corrigiu e2e instável); `nexus-session` fecha app órfão da sessão anterior. Testes: 57 unidade + 13 e2e.
  ROMs de teste na VM: `~/ROMs/{nes,gb,gba}` (240p Test Suite, homebrew livre).
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

## 4b. Telas de sistema — fases (o Nexus substitui o desktop)

Pedido completo em `docs/prompts/telas-de-sistema.md`; o detalhe de cada item (o que existia, o que foi feito, o que
falta, capturas, memória) fica em `docs/STATUS.md`. **Regras do pedido, valem para todo item:**
1. Uma seção por vez: código → testes → STATUS → próxima. Nada de fazer tudo de uma vez.
2. Só controle (D-pad, A, B; teclado = setas/Enter/Esc). Texto: teclado na tela **e** teclado do celular (Remote).
3. Legível a 3 m (texto grande, foco sempre visível). Configurações escondidas em Ajustes (engrenagem / Guia).
4. Perigoso (formatar, reset, modo técnico, IP fixo, desligar autologin) = PIN dos responsáveis + confirmação explícita.
5. Leve: sem blur caro, vídeo ou animação pesada; tela de sistema só gasta enquanto aberta.
6. Toda ação de sistema passa pelo Core (Rust) → serviços do Linux (NetworkManager, BlueZ, PipeWire/wpctl, logind,
   udisks2, UPower, apt, sway). A Shell nunca roda nada privilegiado. Permissões por polkit, no mínimo
   (`system/polkit/51-nexus-system.rules`).
7. Toda tela tem estados carregando / vazio / erro, em português simples, com uma ação ("Tentar de novo", "Voltar").

**Entregáveis por item:**
- cenário de UI em `tests/e2e/scenarios.mjs`, navegando só por teclas;
- serviço do Linux simulado em `tests/e2e/fake/` (nmcli, bluetoothctl, wpctl, pw-play, swaymsg, timedatectl);
- capturas `ui.screens('<item>')` em 1080p e 720p (`NEXUS_SHOTS=<pasta>`);
- memória da Shell com a tela aberta (`tools/measure.sh`);
- entrada no STATUS;
- commit próprio.

Sempre que possível, conferir também na sessão real da VM.

### Feito (2026-11-12)
| Item | Commit | Resumo |
|---|---|---|
| Auditoria | 370b263 | Tabela ✅/🟡/❌ de A1–A7 e B1–B19 no STATUS |
| A6 Boot e transições | 2b3d73b | Plymouth com a marca, splash no fundo do sway, tela "O Nexus encontrou um problema" (Core fora), página reserva da Shell, target com `Wants=` |
| A7 Primeiro uso | d76c04f | Assistente: boas-vindas → idioma/fuso/teclado → rede → controle → perfil → PIN → ROMs → celular → Steam (x86_64) → pronto |
| B1 Rede | d76c04f | `sysnet.rs` (nmcli): Wi-Fi, oculta, esquecer, teste, IP fixo com PIN |
| B8 Idioma e hora | d76c04f | `locale.rs`: fuso, NTP, teclado (sway + localectl). Falta: outros idiomas, 12/24 h |
| B2 Bluetooth | b398d4e | `bluetooth.rs`: liga/desliga, tipo, bateria, conectar, parear, esquecer, ajuda por modelo |
| B3 Áudio | 67285d7 | `audio.rs` (wpctl): volume, mudo, saída lembrada por perfil, teste, microfone; Central rápida com volume real |
| B4 Tela e vídeo | 6db0878 | `display.rs` (sway): resolução/Hz/escala/rotação com confirmação de 15 s feita pelo Core, VRR, HDR (se houver), margem + assistente |
| Navegação | (este commit) | Zonas no foco (`data-zone`): ↑/↓ travam no fim da página; só ←/→ trocam de coluna; volta à categoria atual e ao último item |
| Upload de ROMs | 67285d7 | Página responsiva, com versão de PC (arrastar e soltar) |
| Controle PWA | a4df496 | Manifesto, ícones, código guardado no celular, botão "Instalar na tela inicial" |
| Google Drive (1/2) | (este commit) | `drive.rs` (rclone): login na TV, pasta `Nexus`, montagem com cache, ROMs da nuvem nos Emuladores, etapa no primeiro uso. Falta: aba Mídia (mpv), login por QR no celular |
| Google Drive (2/2) — Mídia | (0.5.0) | Aba Mídia com filmes, séries e músicas da pasta `Nexus` do Drive (e Vídeos/Música da TV), tocados pelo mpv em tela cheia. **Feito.** Falta: login por QR no celular |
| Fase 2 (A1–A4, B9, B10) | (0.5.0) | Barra de status, Central rápida completa, overlay no jogo, notificações, Energia, Armazenamento e discos USB. **Feito** (ver STATUS) |
| Fase 3 (B6, B7, B11, B15, B16, SSH) | (0.5.0) | Controles, celular (revisão), atualizações, recuperação, sobre/diagnóstico e acesso por SSH com passo a passo na TV. **Feito** (ver STATUS) |
| Fase 4 (A5, B5, B12–B14, B17–B19) | (0.5.0) | Tela de energia, CEC, PIN esquecido e relatório de uso, aparência, acessibilidade, apps instalados (.jar/AppImage/.sh), pastas e núcleos de ROMs, privacidade. **Feito** (ver STATUS) |

### Fase 2 — feita em 0.5.0 (lista original, nesta ordem; ver STATUS)
1. **A1 Barra de status:**
   - mostrar hora, Wi-Fi/cabo com sinal (`net.status`), Bluetooth, volume/mudo (`lib/audio.ts`), bateria dos
     controles (`bluetooth::status` → `battery`) e perfil;
   - alertas só quando há problema (sem internet, controle com bateria ≤ 20 %, disco com menos de 10 %);
   - some durante jogo e vídeo;
   - um evento do Core (`status.changed`) em vez de várias consultas periódicas.
2. **A2 Central rápida:** já tem Retomar, Início, Fechar, volume real, tema, perfil, reiniciar e desligar. Falta:
   - saída de áudio (alternar);
   - Wi-Fi e Bluetooth liga/desliga;
   - **suspender** (logind);
   - brilho quando houver backlight (`/sys/class/backlight`, via Core);
   - medir quanto tempo leva para abrir sobre o jogo.
3. **A3 Overlay no jogo:**
   - volume e bateria do controle;
   - aviso de notificação importante por cima do jogo;
   - **saída de emergência**: segurar Guia ou Select+Start por 3 s pede confirmação e mata o escopo do jogo.
4. **A4 Notificações:**
   - fila curta (no máximo 3 visíveis, sem acumular repetidas);
   - "silenciar durante jogos" (preferência);
   - novos avisos: controle desconectado, disco cheio, erro ao abrir jogo.
5. **B10 Energia:**
   - suspender, reiniciar e desligar com confirmação;
   - inatividade para apagar a tela (`swaymsg output * power off` / swayidle) e para suspender;
   - o que o botão físico de energia faz (logind `HandlePowerKey`, polkit);
   - "abrir direto no Nexus" (autologin, já existe: mostrar o estado).
6. **B9 Armazenamento:**
   - uso por categoria (jogos Steam, ROMs, capas/cache, sistema);
   - discos USB: listar, montar, **ejetar com segurança** (`udisksctl unmount` + `power-off`), usar como pasta de ROMs;
   - limpar cache e temporários;
   - formatar com PIN + confirmação dupla;
   - alerta de pouco espaço (ligado ao A1/A4).

### Fase 3 — feita em 0.5.0 (lista original; ver STATUS)
- **B6 Controles:**
  - teste de botões e analógicos ao vivo;
  - zona morta;
  - vibração de teste;
  - ordem dos jogadores (já existe);
  - perfil de controle por emulador;
  - ajuda para controle sem suporte.
- **B7 Celular:** quase pronto. Revisar o texto "o que o celular pode fazer" e a validade do código (feito no item 8).
- **B11 Atualizações:**
  - versão do Nexus e do sistema;
  - verificar e instalar (apt do Nexus + pacotes do sistema, via Core/polkit);
  - histórico;
  - reverter o Nexus para a versão anterior;
  - nunca durante um jogo;
  - avisar se vai reiniciar.
- **B15 Recuperação (atrás do PIN):**
  - reiniciar a sessão;
  - modo de recuperação (reparar a Shell, reverter atualização, `force-desktop`, diagnóstico);
  - SSH liga/desliga com IP;
  - terminal opcional com aviso;
  - restaurar padrões;
  - apagar dados do perfil;
  - reset de fábrica com confirmação dupla.
- **B16 Sobre e diagnóstico:**
  - modelo, versões, CPU, memória, GPU;
  - temperatura e throttling (Pi: `vcgencmd get_throttled`);
  - uso atual e tempo ligado;
  - o que consome recursos;
  - "Gerar relatório de suporte" (logs + informações em um arquivo; entregar pelo celular).

### Fase 4 — feita em 0.5.0 (lista original; ver STATUS)
- **A5:** tela de energia única (suspender, reiniciar, desligar, trocar perfil).
- **B5 CEC:** `cec-client`/libcec, quando houver hardware; ligar/desligar a TV junto e usar o controle remoto da TV.
- **B12 Perfis/parental:** recuperar o PIN e relatório de uso.
- **B13 Aparência:** tamanho do texto e fundo.
- **B14 Acessibilidade:** texto maior; leitor de tela, se viável.
- **B17 Apps instalados:** tamanho, desinstalar, ocultar, categoria.
- **B18 ROMs:** escolher pastas, reescanear, emulador padrão por console.
- **B19 Privacidade:** o que vai pela rede, telemetria (padrão desligada, o Nexus não tem) e limpar histórico/busca.

### Encerramento (depois da fase 4, ou antes, se o usuário pedir)
1. Remover os ajustes temporários de desenvolvimento da VM:
   - `~/.config/systemd/user/nexus-shell.service.d/zz-repo-electron.conf`;
   - a cópia `~/.config/systemd/user/nexus-session.target`.
2. Zerar o app na VM (dados do Core e perfis) e mostrar ao usuário o assistente de primeiro uso.
3. Resumo final do "prompt de melhoria" (itens 1–9, no STATUS): ganhos medidos, riscos e próximos passos.

### Decisões e aprovações pendentes do usuário
- `sudo` na VM para medir o nexus-slim/zram.
- Chave GPG, segredo no GitHub e Pages para o repositório apt.
- Gamescope (`docs/decisions/0004`).
- Modo "encerrar a Shell" em aparelhos com ≤ 2 GB.
- Suporte a outras distros (hoje só Debian/Ubuntu, por causa do apt).

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
  confere sha256). Sem o Nexus instalado roda o `install.sh` completo; com ele, só `apt install` do `.deb`. O repositório é **público**
  (desde a 0.4.1, pedido do usuário): `curl -fsSL https://raw.githubusercontent.com/GuiVicS/nexus/main/tools/update.sh | sh`, sem login.
  O script ainda cai no `gh release download` se o link aberto falhar (caso o repositório volte a ser privado). O site da Vercel ficou parado na 0.2.0.
- **Instalação completa por padrão (pedido do usuário, 0.3.1):** `install.sh` = Steam (PC) + RetroArch (`--sem-steam`/`--sem-emuladores`
  para tirar) + navegador se não houver (plano: `chromium`, ou `chromium-browser` no Ubuntu) + `bluez`/`curl` nas dependências do `.deb`.
- Ao mudar versão: `Cargo.toml`, `README.md`; depois bundle → copiar → `latest.txt` → deploy.
- **Quando o usuário atualizar o RetroArch ou os núcleos aqui:** `tools/pin-deps.sh` (ou `retroarch` / `cores`), publicar os núcleos
  numa release própria `gh release create cores-<ID> dist/nexus-cores-<ID>-linux-*.tar.gz* --title "Núcleos <ID>"` (só quando mudarem;
  ~130 MB cada) e então a release do Nexus de sempre (o `deps.lock` vai dentro do `.deb` e do `.tar.gz`). `update.sh` baixa os núcleos
  só se o ID instalado (`/usr/lib/nexus/cores/ID`) for diferente.

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
- No sway kiosk, **toda janela nova (menos a Steam) vai para o workspace 2**; a Steam vai para o scratchpad (invisível). O Core só troca de tela quando a janela do jogo está estável; para mostrar a Steam use `show_steam_ui`/`peek_window`.
- **Testes na sessão aninhada com `wtype`:** depois de usar `wtype`, a Shell (Electron com `--disable-gpu`) **morre com SEGV ao reiniciar** (`xkb_state_update_mask`: o teclado virtual do wtype deixa um keymap no seat). Não afeta o aparelho real. Reinicie a sessão aninhada inteira em vez de só `nexus-shell`. F1 no kiosk é `bindsym` **sem** `--to-code` (senão o wtype não aciona).
- `grim` trava com a proteção de tela do GNOME ativa: `gdbus call --session --dest org.gnome.ScreenSaver --object-path /org/gnome/ScreenSaver --method org.gnome.ScreenSaver.SetActive false`.
- Electron único (`requestSingleInstanceLock`): uma instância antiga sobrando faz a nova sair **em silêncio**.
- `pkill -f padrão` dentro de um `bash -c` mata o próprio shell (o padrão aparece na linha de comando); use `pgrep -f "[p]adrão" | xargs kill`.
- Estado `Solid`: o foco do anel é recalculado quando o elemento focado some do DOM (`onLost` em `packages/focus`); overlays restauram o foco anterior (`ret`).
- A Steam **não** oferece API aberta para "jogos que possuo" sem chave (testado: `GetAppList`, perfil em XML, etc.). A vitrine pública (`featuredcategories`, `search/results?json=1`, `appdetails`) funciona sem chave; a busca não traz `appid` (extraído da URL da imagem).
- **Capas da loja (0.3.2):** jogos novos da Steam usam endereço com hash (`store_item_assets/steam/apps/<id>/<hash>/library_capsule_2x.jpg`); o endereço fixo dá 404. O Core resolve pela API pública `IStoreBrowseService/GetItems/v1` (`include_assets`, sem chave) em `providers/steam.rs` (`resolve_assets`/`parse_assets`, `art_late` quando o download falha); DLC sem capa vertical cai em `hero_capsule`/`header`.
- Testes e2e em paralelo: um Core adotava o processo falso "SteamLaunch AppId" de outro teste (launch dava Busy). A adoção de jogo Steam em execução só acontece se o Core tem uma Steam (`root()`).
- `nexus-session` (0.3.2) encerra o grupo do app que sobrou de uma sessão anterior (`game.json` → `kill -- -sid`) antes de subir o Core.
- Teste de interface: o campo de PIN/texto precisa de uma pausa depois de abrir; setas ◀▶ em seletores alteram valores (navegue na vertical em formulários).

## 10. Próximos passos sugeridos (prioridade)

0. ~~**Google Drive, parte 2**~~ (**feito**: aba Mídia; falta só o login por QR no celular, ver abaixo). Pedido original: aba "Mídia" (filmes, séries e músicas de `Nexus/Filmes|Séries|Músicas` pelo `mpv` em tela cheia, com o menu rápido por cima; `mpv` entra em `Depends`). Depois, login por QR no celular: o usuário cria um projeto no Google Cloud e passa o ID do cliente + página no GitHub Pages que repassa o código (PKCE) para a TV. Decisões já tomadas: login na TV agora (cliente do rclone), rclone do apt (1.60), Drive antes das fases.
1. ~~**Telas de sistema**~~ **feitas** (fases 2, 3 e 4 da §4b, em 0.5.0). Pendências abertas estão em `docs/STATUS.md` (resumo final) e na nota de "Atualização para 0.5.0" da §4.

1. **Fechar o ciclo no aparelho do usuário:** confirmar a sessão (`session.log`), abrir um jogo Steam real, ajustar o que aparecer. Tudo o resto depende disso.
2. **Testar num Raspberry Pi real** (pacote arm64 só validado com QEMU): sessão, GPU V3D, RAM, emuladores leves, Chromium com DRM.
   O usuário vai testar com o **roteiro em `docs/TESTE-REAL.md`** (Pi OS Lite + greetd + `--autologin`; checklist por área). Peça o resultado item a item.
3. ~~Apps web~~ e ~~emulação~~ feitos na 0.3.0; falta URL personalizada em *Apps e jogos* e identificar console por hash.
4. **Nexus Sysd** (D-Bus + polkit, ações com argv fixo): instalar/atualizar provedores pela interface sem terminal.
5. Spike S1 (instalar jogo Steam sem diálogo; talvez via porta CEF 8080 da Steam — não documentado, tratar como experimento) e medir RAM/FPS reais (gate S4).
6. Repositório apt próprio (hoje a distribuição é `.deb` solto), imagem/Pi só depois (V4).

## 11. Mapa rápido de arquivos

```
core/crates/nexus-core/src/  accounts.rs appupdates.rs db.rs game.rs rpc.rs net.rs remote.rs vdev.rs inputmap.rs customart.rs art.rs hardware.rs procs.rs guide.rs compositor.rs roms.rs roms_upload.html
                             providers/{mod,steam,native,demo,emu,web}.rs   tests/e2e.rs (Core de ponta a ponta)
apps/shell/src/              main.tsx styles.css lib/{state,actions,input,session,api,format,flows}.ts components/{App,Views,Emu,Roms,Hub,Overlays,Settings,Profiles,Apps,Entry,ui}.tsx
apps/shell/electron/         main.cjs preload.cjs   (ponte segura com o Core; protocolo nexus-art://)
apps/remote/src/             App.tsx Gamepad.tsx Touchpad.tsx link.ts remote.css   (celular)
packages/{protocol,focus,ui} installer/src/{main,detect,plan,autologin}.rs
system/                      session/ sway/ systemd/ sysd/ (ajudante root) polkit/ udev/ apparmor/ packaging/
tools/                       build-deb.sh bundle.sh update.sh extract-demo-catalog.mjs gen-protocol-index.mjs
tests/e2e/                   lib.mjs scenarios.mjs run.mjs cdp.mjs
assets/demo/                 catálogo e imagens fictícias do modo demonstração
```

## 12. Origem do projeto (contexto)

Nasceu de um protótipo visual em HTML único (`console-os.html`, repositório separado *retropad*, ainda hospedado como PWA em `https://nexuos-console.vercel.app`) e reaproveita do **RetroPad** apenas a ideia do celular como gamepad/mouse/teclado (redesenhada no tema do Nexus). A identidade visual (turquesa sobre grafite, modo claro, sons, cursor temático) vem desse protótipo e está em `packages/ui/src/theme.css`.
