# NexusOS / Nexus — Plano técnico de ação

Legenda de confiança: **✔** confirmado em fonte nesta pesquisa · **◐** parcial/fonte fraca · **?** não confirmado (vira *spike*, nunca decisão). Licenças sem ✔ vêm de conhecimento prévio: **conferir no repositório antes de adotar.**
Nota: sua mensagem tinha dois arquivos (protótipo + plano `anexos/plano (1).html`) e depois pediu só o *celular como controle e mouse/teclado* do RetroPad. Tudo isso está incorporado.

---

## 1. RESUMO EXECUTIVO

**O que é o Nexus.** Um *software instalável* (não distro) que transforma uma distro Linux comum em experiência de console: sessão própria, interface de TV por controle, biblioteca unificada (Steam primeiro), instalação sem terminal, execução de **jogos reais** que assumem a tela e **devolvem ao Nexus** quando fecham. Nunca renderiza o jogo dentro de si.

**Arquitetura recomendada (resumo):**
- **Sessão (item 11 → resposta: B+D, com um "E" refinado):** o Nexus é uma **sessão Wayland própria selecionável no gerenciador de login (B)** cujo comando sobe **um compositor kiosk existente (D)**, tudo gerido por `systemd --user`. Autologin é opcional. **Não** é app sobre GNOME/KDE (A), **não** é ambiente gráfico novo (C).
- **Compositor (item 12):** **sway (wlroots)** no MVP. `gamescope` entra na V2 como backend "Gaming Mode" e como wrapper por jogo. Nenhum compositor é escrito do zero.
- **Camadas:** Shell (TV) ⇄ **Core** (daemon Rust) ⇄ Providers ⇄ Linux/Steam/emuladores. Shell **só manda IDs**; Core valida e executa.
- **Shell:** web (Electron + SolidJS + TypeScript) — porque reaproveita o protótipo aprovado e o design system do celular. **Gate objetivo** (RAM/tempo/FPS): se reprovar, troca para Qt/QML sem mexer no resto (o contrato Shell⇄Core é a fronteira).
- **Remote (celular):** web PWA servida pelo Core → dispositivos virtuais `uinput` (gamepad P1/P2, mouse, teclado). Lógica herdada do RetroPad, redesenhada no tema NexuOS.

**Estratégia inicial (honesta):** o risco do projeto **não é a interface** (já existe protótipo), é a **ida e volta com jogo real** e a **instalação Steam sem a UI da Steam**. Por isso: primeiro validar isso em hardware real (spikes), com **uma distro só** (Ubuntu 24.04 LTS, GPU AMD/Intel, x86_64). Fedora, Arch, NVIDIA, emulação, celular e ARM vêm depois.

**Onde discordo de hábitos/suposições comuns:**
1. *"Wayland = gamescope"* — gamescope é o melhor "modo console", mas em modo embutido é **fortemente acoplado à Steam; outros apps precisam emular o que ela faz ✔**. Custo alto demais para o MVP.
2. *"Electron é pesado, use Tauri"* — Tauri usa WebKitGTK, que **varia por distro e tem problemas com NVIDIA ✔**. Para um launcher que roda em qualquer distro, o Chromium *fixado* do Electron é mais previsível.
3. *"Instale via PackageKit (um backend p/ todas as distros)"* — os backends têm **qualidade desigual ✔**. Use adaptadores diretos (apt/dnf/pacman) num helper privilegiado.
4. *"Atualize o Nexus com o auto-updater do Electron"* — numa instalação por pacote, o dono do arquivo é o gerenciador de pacotes. **Atualizar pelo repositório** é mais seguro.

---

## 2. ANÁLISE DOS ARQUIVOS

### 2.1 Protótipo (`/root/retropad/console-os.html`, 1.350 linhas, vanilla JS) × arquitetura necessária

| Aspecto | O que o protótipo já tem | O que a arquitetura exige | Veredito |
|---|---|---|---|
| Telas/fluxo | standby → boot → perfis → **Início (Dock)**, Biblioteca, Mídia, Loja, **Hub do jogo**, Central de controle, Notificações, Busca (teclado na tela), Ajustes, execução/player | Mesmas telas ligadas a dados reais, estados de erro/vazio/carregando | **Reaproveitar** design e fluxo |
| Navegação | Foco espacial próprio (`move/setFocus`), **anel que desliza** (`ringLoop`), pilha de overlays (`pushOv/popOv`), `back()`, barra de comandos (`hints`), detecção de modo **pad/key/ptr**, Gamepad API (`poll`), B em ≤3 apertos, sem armadilhas (fuzz de 900 teclas) | + *engajamento* de listas (A entra, B sai), overrides direcionais, container/grupos, virtualização | **Reaproveitar e extrair** para `@nexus/focus` |
| Componentes | `tl()` (tile), chips, painéis, toasts, linhas de ajuste, tema claro/escuro por variáveis CSS, cursor temático, sons Web Audio, `decorate()` (ARIA) | Biblioteca de componentes tipada, compartilhada com o Remote | **Reaproveitar** CSS/tokens; **refazer** como componentes |
| Dados | Arrays globais (`GAMES/APPS/STORE…`), IDs por letra | IDs `provider:nativo`, modelo `Item` com estado, paginação | **Refazer** |
| Execução/download | `launch()`, `startDl()`, `acquire()` **simulados** | Core + Providers reais, eventos de progresso | **Descartar** (viram chamadas ao Core) |
| Estado | Globais `st`/`S`, `localStorage` | Stores reativos; persistência no Core (SQLite) | **Refazer** |
| Renderização | Templates `innerHTML` por string | Componentes (Solid) com escape automático | **Refazer** — com metadados reais, `innerHTML` vira **risco de XSS** |
| Desempenho | 54–60 fps medidos; miniaturas 640 px; sem `backdrop-filter` | Bibliotecas Steam de 500+ jogos: **virtualizar lista** (a grade atual renderiza tudo) | Reaproveitar regras; **adicionar** virtualização |
| Testes | Harness CDP (`/root/cos-test`: `cdp.mjs`, `v2t.mjs`, `fuzz2.mjs`, `audit.mjs`, `fps.mjs`) | e2e contra Core mock | **Reaproveitar** |
| Limitações do HTML | Sem acesso a processos, arquivos, D-Bus, sistema; áudio só após gesto; um arquivo monolítico | Shell com ponte (`contextBridge`) para o Core | Resolve com Core + Electron |

**Problemas de arquitetura a corrigir:** estado global único; renderização por string; arte embutida/gerada offline (precisa de pipeline de cache no Core); sem i18n (tudo pt-BR fixo); sem camada de erro de provider; sem noção de *capabilities* (efeitos reduzidos em GPU fraca).

### 2.2 Documento arquitetural (`anexos/plano (1).html`)

| Item | Avaliação |
|---|---|
| **Conceitos corretos (manter)** | Shell→Core→Providers→HAL; "não faça tudo numa aplicação" (Batocera); UI só conhece *capabilities*; base Linux existente primeiro; não reinventar emulação/Steam/input/áudio/rede; *Settings* normal×avançado + "Nexus Control"; estrutura de repositório e ADRs; ciclo por módulo validado em hardware real |
| **Decisões já tomadas** | Android **não** é fundação (Waydroid opcional, condicional ao hardware); base universal = Linux existente; Buildroot só p/ appliance; Yocto só se virar multi-placa |
| **Ajustar** | (1) **Steam sobe** para o caminho crítico (o doc deixa em "Fase 3"); (2) falta **Native Linux Provider** (apps `.desktop`); (3) **IPC não especificado** → Unix socket JSON-RPC; (4) HAL só na Fase 4 → o MVP precisa de um **mínimo de capabilities** desde cedo; (5) "Pi 3 como teste de disciplina" conflita com Shell web → trocar por **orçamento de RAM/FPS** em x86 modesto e deixar Pi como alvo de uma Shell nativa futura; (6) "ChimeraOS: não depender da UI da Steam" é verdade, mas a *instalação* pode exigir passar pela Steam (ver seção 6) |
| **Riscos técnicos que o doc subestima** | instalação Steam sem UI; fim de processo com Proton/Wine; autologin por gerenciador; AppArmor do Ubuntu 24.04 bloqueando Electron ✔; NVIDIA + wlroots |

### 2.3 RetroPad (só o que você pediu)

Reaproveitar **a semântica** de `server.py` (WebSocket `join/state/move/scroll/click/key/text`; gamepad virtual Xbox 360 `0x045E:0x028E`; `apply_state` envia só o que mudou; `release()` ao desconectar), `inputmap.py` (caractere→tecla, layout US), limitador de PIN, `tunnel.py` (HTTPS opcional), `systems.json` (seed da Emulação). **Não** reaproveitar: transmissão de tela (`stream.py`), gerenciador de ROMs/Flask.

---

## 3. ARQUITETURA FINAL RECOMENDADA

```
 CELULAR (PWA) ── Wi-Fi ──┐                               TV + controle
                          ▼
┌──────────────────────── SESSÃO NEXUS (Wayland) ───────────────────────────┐
│  Display Manager (SDDM/GDM/greetd)  →  nexus.desktop  →  nexus-session     │
│                                                                            │
│  COMPOSITOR KIOSK: sway (MVP) | gamescope (V2)   ← controlado via IPC      │
│        │ janelas                                                           │
│  ┌─────▼──────────┐  JSON-RPC 2.0 / Unix socket 0600   ┌─────────────────┐ │
│  │ NEXUS SHELL    │◄──────────────────────────────────►│ NEXUS CORE      │ │
│  │ Electron+Solid │   comandos (só IDs) + eventos      │ Rust, systemd   │ │
│  └────────────────┘                                    │ --user          │ │
│                                                        │ ├ Library/DB    │ │
│       NEXUS SYSD (root, polkit, D-Bus-ativado) ◄───────┤ ├ Supervisor    │ │
│       fluxos privilegiados com argumentos fixos        │ ├ Hardware/HAL  │ │
│                                                        │ ├ Remote(uinput)│ │
│                                                        │ └ Providers     │ │
│                                                        └───────┬─────────┘ │
└────────────────────────────────────────────────────────────────┼───────────┘
        Steam client · RetroArch/emuladores · .desktop/Flatpak · D-Bus (NetworkManager, BlueZ, udisks2, UPower, logind) · PipeWire
```

**Camadas**
- **Shell:** só apresentação e foco. Sem Node na página (`contextIsolation`, sandbox, CSP). Fala com o Core pelo *main process* via `contextBridge`. Fica **oculta** enquanto o jogo roda.
- **Core:** único com lógica de sistema. Roda **como usuário**. Módulos: `library` (SQLite), `supervisor` (sessões de jogo), `hardware` (capabilities), `remote` (HTTP+WS→uinput), `settings`, `providers/*`, `updates`.
- **Sysd (helper):** processo root mínimo, ativado por D-Bus, protegido por **polkit**, só executa **ações nomeadas** (instalar pacote X, mudar perfil de energia, escrever drop-in de autologin) com **argv fixo**.
- **Providers:** módulos Rust atrás de um contrato único (seção 9). MVP: Steam. V2: Native Linux, Emulation.

---

## 4. STACK TECNOLÓGICA (escolha → motivo → alternativas → por que não)

| Camada | Escolhido | Motivo | Alternativas e por que não |
|---|---|---|---|
| **UI Shell** | **Electron** (Chromium fixado) + **SolidJS** + TypeScript + Vite | Reuso do protótipo e do harness; design system único com o Remote; Chromium igual em todas as distros; Wayland nativo **a partir do Electron 38.2 ✔** | **Tauri/WebKitGTK**: desempenho/NVIDIA/versão variam por distro ✔. **CEF**: pesado e quebra entre versões ✔. **Qt/QML** (como Pegasus ✔): melhor p/ hardware fraco, mas reescreve tudo e fragmenta o design system → **plano B** se o gate falhar. **Godot** (OpenGamepadUI ✔): bom p/ gamepad, mas binário grande e ecossistema fechado p/ web. **React**: reconciliador mais caro que Solid em listas grandes. **Svelte 5**: empate técnico; Solid vence por JSX próximo do React |
| **Foco/navegação** | **`@nexus/focus` próprio**, extraído do protótipo | Já cobre espacial, overlays, memória, anel; falta só engajamento/overrides | **Norigin Spatial Navigation** (núcleo agnóstico ✔, MIT): plano B; hooks são React e a API baseada em árvore de `focusKey` não encaixa no protótipo. **js-spatial-navigation**: sem manutenção clara ◐ |
| **Gamepad na Shell** | **Gamepad API do Chromium** | Só entrega à janela em foco ⇒ Shell **não rouba** controle do jogo (a confirmar no S3) | **SDL3** na Shell: exigiria binding nativo; **evdev direto**: precisa de mapeamento próprio |
| **Core** | **Rust** (tokio, serde, rusqlite, zbus, notify, rustix) | Daemon de longa vida: pouca RAM, sem runtime, tipos gerados p/ TS (ts-rs/specta); crates de Steam e `.desktop` existem ✔ | **Go** (runner-up: mais rápido de escrever, binário único). **Node**: RAM e fragilidade num daemon que vigia processos. **Python** (RetroPad): bom protótipo, ruim p/ concorrência e deploy |
| **IPC Shell⇄Core** | **JSON-RPC 2.0 em Unix socket** (`$XDG_RUNTIME_DIR/nexus/core.sock`, 0600) + tipos gerados | Local, sem porta, bidirecional (eventos de progresso) | **HTTP local**: exposto a qualquer processo/CSRF. **gRPC**: peso + proxy p/ navegador. **D-Bus como transporte**: Chromium não fala D-Bus (D-Bus fica para *consumir* serviços do sistema) |
| **IPC privilegiado** | **D-Bus + polkit** (Core→Sysd) | Padrão do Linux p/ ações privilegiadas com autorização | `sudo`/`pkexec` com comandos livres: viola o requisito de segurança |
| **Sessão** | `nexus.desktop` em `/usr/share/wayland-sessions` + **units systemd --user** | Funciona em qualquer distro sem tocar no desktop do usuário | Autostart sobre GNOME/KDE (A): desktop aparece; DE próprio (C): reinvenção |
| **Compositor** | **sway** (wlroots) | Leve, empacotado em todas, **IPC de janelas**, XWayland, regras kiosk | **gamescope**: embutido acoplado à Steam ✔ → V2. **cage**: 1 app, sem layer-shell ✔. **GNOME Kiosk (Mutter)**: sólido e empacotado (Debian/Fedora ✔), mas preso ao ciclo do GNOME e foco single-app ✔. **KWin kiosk**: ótimo p/ NVIDIA/HDR, mas arrasta KDE. **labwc**: sem IPC rico. **Hyprland**: rolling-release. **Compositor próprio (smithay/wlroots)**: reinventa |
| **Gestão de janelas/jogo** | **sway IPC** (`swayipc`) + `workspace` dedicado p/ jogos | Determinístico: jogo abre → workspace 2; fim → workspace 1 + foco na Shell | `wlr-foreign-toplevel` direto (mais código); X11 `xdotool` (não vale em Wayland) |
| **Rastreio de processos** | **scope do systemd por lançamento + `cgroup.events` (inotify, `populated=0`) ✔**; p/ Steam: `reaper SteamLaunch AppId=` ✔ + `pidfd` | Pega filhos reparentados; sem polling de PID | Só PID: falha com launchers e Proton ✔. Polling da árvore (Pegasus #442 ✔) é pouco confiável |
| **Banco** | **SQLite (WAL)** | Embutido, zero serviço | Postgres (excessivo); JSON solto (sem consulta) |
| **Steam (arquivos)** | `steamlocate` + parser VDF (`keyvalues-parser` / `steam-vdf-parser`) ✔ | Encontra Steam, bibliotecas, ACF | Parser próprio |
| **Metadados/arte** | Steam CDN → **SteamGridDB** → IGDB → ScreenScraper → libretro-thumbnails (cadeia à la RomM ✔) | Cada fonte cobre um tipo de conteúdo | Uma fonte só: lacunas |
| **Emulação** | **RetroArch** (`-L core rom -f` ✔) + dados de sistemas do **ES-DE** | Não recriar emuladores | Embutir Pegasus/ES-DE (GPLv3 ✔ / licença?) |
| **Apps Linux** | `.desktop` (XDG) + exports Flatpak, via `freedesktop-desktop-entry` ✔ | Padrão do Linux | Varredura de binários |
| **Input físico** | **kernel (xpad, hid-playstation, hid-nintendo) + evdev (`evdev`/`gilrs`)** só p/ Guia; **SDL** como referência de mapeamentos | Maduro; leitura evdev **não-exclusiva** não rouba input do jogo | **libinput**: não é p/ gamepads (teclado/mouse/touchpad). **InputPlumber**: ótimo p/ handhelds (**GPLv3+ ✔**), pesado p/ o MVP → V3 |
| **Notificações** | Eventos do Core → toasts da Shell | Sem daemon extra | `notification-daemon` (mostraria janelas fora da Shell) |
| **Instalador** | **CLI Rust `nexus-installer`** + pacotes nativos | Detecta, instala, configura | Script bash gigante (frágil); PackageKit (qualidade desigual ✔) |
| **Pacotes** | **`.deb` / `.rpm` / PKGBUILD** (gerados com `nfpm`) + repositórios próprios | Sessão e autologin são do sistema | Flatpak/AppImage do Nexus: não instalam sessão |
| **Atualizações** | **Repositório da distro** (apt/dnf/pacman) + Flatpak p/ providers | Dono do arquivo = gerenciador de pacotes | `electron-updater` (suporta AppImage/deb/rpm ✔, pacman ◐): conflita com o gerenciador |
| **Hardware** | `/sys/class/drm`, `/proc`, `lspci`, `vulkaninfo`, flags de CPU | Sem dependências | Bibliotecas pesadas |
| **Áudio/Rede/BT/Disco** | **PipeWire (`wpctl`)**, **NetworkManager**, **BlueZ**, **udisks2** (D-Bus) | Padrões de fato | Reimplementar |
| **Energia/Desempenho** | `power-profiles-daemon` (D-Bus) + **GameMode** | Maduros | Ajustes manuais de governor |

---

## 5. BIBLIOTECAS E PROJETOS OPEN SOURCE

| Nome | Função | Maturidade | Licença | Vantagens | Limitações |
|---|---|---|---|---|---|
| **sway / wlroots** | compositor kiosk / base | Estável, muito usado | MIT | Leve, IPC, XWayland | NVIDIA historicamente delicado (validar S5) |
| **gamescope** | compositor de jogos (Valve) | Estável na SteamOS/Bazzite | BSD-2 | VRR, upscaling, HDR, foco p/ jogos | Embutido acoplado à Steam ✔; Flatpak Steam problemático ✔ |
| **ChimeraOS gamescope-session** | sessão gamescope | Ativa | (conferir) | Padrão de sessões `.d` ✔ | Só distros que o empacotam |
| **OpenGamepadUI** | launcher em Godot sobre gamescope | Ativa (v0.45 ✔) | (conferir) | Prova que shell de terceiros roda no gamescope | Depende de InputPlumber |
| **GNOME Kiosk** | compositor kiosk (Mutter) | Mantido (2025 ✔) | GPL-2.0+ | NVIDIA/HDR via Mutter | Single-app; acoplado ao GNOME |
| **cage** | kiosk 1 app | Estável | MIT | Simples | Sem layer-shell ✔; 1 app |
| **Electron** | runtime da Shell | Muito maduro | MIT | Chromium fixado; Wayland nativo ≥38.2 ✔ | RAM maior; precisa de perfil AppArmor no Ubuntu 24.04 ✔ |
| **SolidJS / Vite** | UI reativa / build | Maduros | MIT | Atualização fina de DOM | Ecossistema menor que React |
| **Norigin Spatial Navigation** | navegação espacial | Mantida ✔ | MIT | Núcleo agnóstico, `pause/resume` ✔ | Hooks são React |
| **Tokio / zbus / rusqlite / notify / rustix** | async, D-Bus, SQLite, inotify, syscalls | Maduros | MIT/Apache | Núcleo do Core | — |
| **steamlocate** | achar Steam/bibliotecas | Mantida ✔ | MIT | Linux/Win/mac ✔ | Só leitura |
| **keyvalues-parser / steam-vdf-parser** | parser VDF | 0.2.0 (2023) / 2026 ✔ | MIT/Apache | Texto e binário (appinfo) | `keyvalues-parser` parado |
| **freedesktop-desktop-entry** | `.desktop` | Pop!_OS ✔ | MPL-2.0 | Eficiente | — |
| **evdev / gilrs (crates)** | gamepad/uinput | Maduros | MIT/Apache | uinput e eventos | Mapeamento próprio |
| **SDL3 / SDL_GameControllerDB** | gamepad + base de mapeamentos | Muito maduro | zlib | Melhor suporte a DualSense/Xbox/Switch | Binding nativo |
| **InputPlumber** | roteador/remapper de input | Ativo (0.78–0.79 ✔) | **GPLv3+ ✔** | Handhelds, Xbox360/DS4 virtuais ✔ | GPL; pesado p/ MVP |
| **RetroArch / libretro** | emulação | Muito maduro | GPLv3 / MIT (API) | CLI simples ✔ | Cores variam por distro (usar Flatpak) |
| **ES-DE** | dados de sistemas/regras de busca | Ativo | MIT (conferir) | Formato rico | Só como referência de dados |
| **Pegasus Frontend** | frontend Qt | Maduro | **GPLv3 ✔** | Prova de modelo | Não embutir (GPL) |
| **Heroic / legendary / gogdl / nile** | Epic/GOG/Amazon | Ativos ✔ | GPL-3.0 ✔ | CLIs prontas p/ Providers futuros | GPL: chamar como processo, não linkar |
| **Decky Loader (CEF 8080)** | técnica p/ API interna da Steam | Ativo ✔ | GPL (conferir) | Referência de como ler/instalar | **Não documentado/pode quebrar** |
| **SteamGridDB / IGDB / ScreenScraper** | arte e metadados | Estáveis | Termos de serviço | Cobertura ampla ✔ | Exigem chave/conta ✔ |
| **nfpm** | gerar deb/rpm | Maduro | MIT | Um YAML → vários formatos | — |
| **polkit** | autorização privilegiada | Padrão | LGPL-2+ | Ações nomeadas | Requer desenho cuidadoso |
| **Flatpak** | provedores (emuladores) | Muito maduro | LGPL-2.1+ | Versão consistente; rollback | Sandbox/permissões |
| **GameMode / MangoHud** | desempenho/overlay | Maduros | BSD-3 / MIT | Perfil de jogo | — |
| **PackageKit** | pacotes multi-distro | Antigo | GPL/LGPL | API única | Backends desiguais ✔ → **não usar** |

---

## 6. ARQUITETURA DA STEAM

1. **Detecção.** `which steam`, `~/.steam/steam`/`~/.local/share/Steam`, Flatpak `com.valvesoftware.Steam`. Instalador instala **Steam nativa** (Fedora: RPM Fusion ✔, Arch: *multilib* ✔, Ubuntu: `multiverse`+i386 ◐). Flatpak só como fallback (**problemas com gamescope aninhado ✔**).
2. **Cliente em segundo plano.** `nexus-steam.service` roda `steam -silent` ✔ (`-nofriendsui` pode não funcionar ✔). **Primeiro login** por *handoff*: abrir a Steam em modo controle (`-tenfoot` ✔) para login/QR/Steam Guard; depois silenciar.
3. **Biblioteca instalada.** `steamlocate` + `libraryfolders.vdf` + `appmanifest_*.acf`; `inotify` em `steamapps/`.
4. **Biblioteca completa (inclui não instaladas).** (a) **Steam Web API** com login OpenID por QR e chave do projeto (**perfil privado não retorna ✔**); (b) alternativa: **API CEF de depuração** (arquivo `.cef-enable-remote-debugging` e porta 8080 ✔; técnica do Decky; **não documentada, pode quebrar**).
5. **Metadados e arte.** CDN da Steam → SteamGridDB (chave ✔) → cache local servido por protocolo `nexus-art://`.
6. **Instalação sem terminal — ponto mais arriscado (S1).** Candidatos: (i) `steam://install/<id>` (consta na man page ✔; **não sei se abre diálogo de confirmação ?**); (ii) API CEF (?); (iii) steamcmd com login (jogos com DRM exigem cliente ✔); (iv) *handoff* à Steam só no passo de instalar. A decisão sai do S1; o resto da arquitetura **não depende** dela.
7. **Download/progresso.** Independe do gatilho: lê o **ACF a cada 1 s** — `BytesToDownload`/`BytesDownloaded` ✔, `StateFlags=4` = instalado ✔ (demais flags a mapear). Velocidade = delta/tempo. Pausar/cancelar só se a opção escolhida permitir (sem promessa).
8. **Execução.** `steam steam://rungameid/<id>`. Core cria a **Sessão** e muda p/ o workspace de jogos.
9. **Fim do jogo (robusto).** A Sessão termina quando **(a)** não há mais processo com `SteamLaunch AppId=N`/env `SteamGameId=N` (? confirmar no S1) **e (b)** a janela do jogo sumiu. `reaper` é *subreaper* e `pv-adverb` espera todos os descendentes ✔ — por isso o reaper some só quando tudo acabou, **mas** helpers do Wine podem nunca sair ✔ → há *grace period* e botão "Forçar encerramento" (SIGTERM→SIGKILL ao cgroup).
10. **Retorno.** `game.exited` → Core: `workspace 1` + foco na Shell → Shell reaparece com o item atualizado (tempo de jogo, estado).

---

## 7. NEXUS SESSION / WAYLAND

**Como o computador liga:** firmware → bootloader → kernel → systemd → **Display Manager** com **autologin** (opcional) na sessão `nexus` → `nexus-session`.

**Como o Nexus inicia:**
```
/usr/share/wayland-sessions/nexus.desktop   Exec=/usr/bin/nexus-session
nexus-session:  define XDG_SESSION_TYPE=wayland, XDG_CURRENT_DESKTOP=Nexus
                importa o ambiente p/ systemd --user
                systemctl --user start nexus-session.target
                exec sway -c /usr/share/nexus/sway-kiosk.conf   (líder da sessão)
units: nexus-core.service (Restart=always) → nexus-shell.service (Restart=on-failure)
       nexus-steam.service (sob demanda) ; nexus-sysd.service (system bus, root)
```
Sway em modo kiosk: sem barra, sem atalhos do usuário, Shell em tela cheia no workspace 1, regra "toda janela que não é da Shell → workspace 2 em tela cheia".

**Como jogos assumem a tela:** Core lança no escopo systemd → janela nova aparece no workspace 2 (fullscreen); Core troca o workspace por IPC. **Gamescope por jogo** (opcional): `gamescope -f -- <cmd>` p/ upscaling/limite de FPS em apps não-Steam.

**Como o Nexus retorna:** fim da Sessão (seção 6.9/17) → `workspace 1` + foco. Se a Shell caiu, o systemd a reinicia e o Core reconcilia.

**Autologin por gerenciador (item 11/13):**
- **SDDM:** `[Autologin] User=… Session=nexus.desktop` — o valor é o **nome do arquivo `.desktop` ✔**; diretórios padrão `/usr/share/wayland-sessions` ✔.
- **GDM:** `/etc/gdm/custom.conf` `[daemon] AutomaticLoginEnable=True` + `AutomaticLogin=<user>` ✔; sessão padrão vem do **AccountsService** (`/var/lib/AccountsService/users/<user>`) ✔; há relatos de autologin ignorando a sessão preferida ◐ → testar (S5).
- **greetd:** `initial_session` ✔. **LightDM:** `autologin-session` (V2).

**MODO AVANÇADO (sem sair do conceito):** dentro do Nexus, protegido por PIN: **Terminal** (abre `foot` como janela comum no workspace 2), **Logs** (journal lido via Core), **Diagnóstico** (`nexus doctor`), **Recuperação**, **Abrir Desktop** (helper grava "próxima sessão = desktop original" e encerra; o DM mostra o ambiente anterior).
**Saídas de emergência:** troca de VT sempre ativa ✔ (Ctrl+Alt+F3), arquivo `/etc/nexus/force-desktop`, e **guarda de loop**: ≥3 falhas da sessão em 5 min → o Core/`nexus-recovery.service` desfaz o autologin e volta ao desktop.

**Decisão sobre as opções A–E:** A é só para desenvolvimento no seu PC (mais rápido, mas o desktop aparece, há notificações/atalhos do DE, 2 gerenciadores de janela); **B+D** é o caminho de produto; C é reinvenção; E (refinamento) = B+D com **backend de compositor intercambiável**, e *Gaming Mode* via gamescope na V2. **Critérios para trocar o sway:** se o spike S5 mostrar instabilidade em NVIDIA → testar **GNOME Kiosk/KWin** como backend; se S2 mostrar que a Steam rende melhor sob gamescope → promovê-lo a padrão em AMD/Intel.

---

## 8. INSTALADOR

**Formato:** pacotes **nativos** (`.deb`, `.rpm`, `PKGBUILD`) em **repositórios próprios** + o binário `nexus-installer` (orquestra). Electron empacotado em `/opt/nexus`. Atualização = `apt/dnf/pacman upgrade`.

**Distros suportadas:**
| Quando | Distro | Observação |
|---|---|---|
| **MVP** | **Ubuntu 24.04 LTS**, x86_64, GPU **AMD/Intel** | Estabilidade e base de usuários; exige **perfil AppArmor** p/ o Electron (userns ✔) |
| **V2** | **Fedora (atual)**, **Arch** (+CachyOS/EndeavourOS), **NVIDIA** | Fedora: RPM Fusion p/ Steam ✔; Arch: multilib ✔ |
| **V3** | Debian estável, derivados do Ubuntu (Mint/Pop) | Validar um a um |
| **Futuro** | Imutáveis (Bazzite/SteamOS/Silverblue), NixOS, **ARM64** | V4 / imagem própria |

**Fluxo do instalador (`nexus-installer`):**
```
1 DETECTAR   /etc/os-release (ID, ID_LIKE, VERSION_ID) · uname -m · GPU (/sys/class/drm/*/device/{vendor,device}, driver)
             Vulkan (ICD em /usr/share/vulkan/icd.d, vulkaninfo) · RAM (/proc/meminfo) · disco (statvfs)
             Steam (binário/Flatpak/~/.steam) · Flatpak · gerenciador de pacotes (pela distro)
2 REQUISITOS  x86_64 · Wayland-capaz · ≥4 GB RAM (8 recomendado) · ≥6 GB livres · GPU com Vulkan p/ Steam. Falhou → explica e para.
3 BASE        nexus-core, nexus-shell(/opt), nexus-session (sway+xwayland), nexus-sysd, regra udev (uinput), perfil AppArmor, polkit
4 PROVIDERS   (opcionais, perguntados) Steam: steam + libs 32-bit (habilitar multiverse/i386, RPM Fusion ou multilib COM consentimento)
              Emulação (V2): RetroArch/cores via Flatpak
5 SESSÃO      arquivos já vêm no pacote; `nexus doctor` valida
6 AUTOLOGIN   opcional: escreve drop-in do DM (com backup e `nexus revert-autologin`)
```
**Dependências (item 14):**
```
NEXUS BASE            Shell · Core · Input(uinput/udev) · Session(sway, xwayland) · sysd · polkit
STEAM PROVIDER        steam (nativa) + libs 32-bit + Vulkan do GPU (Mesa/NVIDIA já existentes)
EMULATION PROVIDER    RetroArch + cores; emuladores extras = Flatpak individual
NATIVE LINUX PROVIDER nenhuma
OPCIONAL (V2+)        gamescope · GameMode · MangoHud · foot (terminal avançado)
```
Regra: **o instalador nunca instala "tudo"**; cada Provider declara suas dependências e só entra se o usuário aceitar.

**Atualizações (item 19):**
- **Nexus:** repositório apt/dnf/pacman. A Shell mostra "atualização disponível" (Core checa `apt-get -s`, `dnf check-update`, `pacman -Qu`) e aplica via **Sysd/polkit**. Rollback: cache de pacotes anteriores / `dnf history undo`.
- **Providers:** Steam se atualiza sozinha; Flatpak (`flatpak update`, rollback por commit); cores do RetroArch pelo Flatpak/distro.
- **Componentes:** Proton é da Steam; gerenciador de Proton-GE só na V3.
- **V4 (imagem):** `bootc`/OSTree ou A/B (`systemd-sysupdate`) — só se virar NexusOS.

---

## 9. PROVIDER SYSTEM

```rust
// core/providers/src/lib.rs  (contrato único; a Shell nunca vê detalhes internos)
#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;                       // "steam" | "native" | "emulation"
    fn capabilities(&self) -> Capabilities;             // {install, pause, cancel, uninstall, progress...}
    async fn list_library(&self) -> Result<Vec<Item>>;
    async fn get_item(&self, id: &ItemId) -> Result<Item>;
    async fn install(&self, id: &ItemId) -> Result<JobId>;
    async fn pause(&self, job: &JobId) -> Result<()>;    // Err(Unsupported) se não houver
    async fn resume(&self, job: &JobId) -> Result<()>;
    async fn cancel(&self, job: &JobId) -> Result<()>;
    async fn uninstall(&self, id: &ItemId) -> Result<()>;
    async fn launch(&self, id: &ItemId) -> Result<LaunchPlan>; // o Provider MONTA o comando; o Supervisor executa
    fn events(&self) -> BoxStream<'static, ProviderEvent>;     // item.changed, install.progress
}
pub struct LaunchPlan { argv: Vec<OsString>, env: Vec<(String,String)>, tracker: Tracker }
pub enum Tracker { Scope, SteamApp(u32), Window(AppIdMatch) }   // como saber que acabou
```
```ts
// packages/protocol (gerado por ts-rs a partir do Rust)
type ItemState = "not_installed"|"installing"|"installed"|"updating"|"running"|"error";
interface Item { id:string /* "steam:570" */; kind:"game"|"app"|"rom"; title:string; state:ItemState;
  sizeBytes?:number; art:{cover?:string;hero?:string;logo?:string;icon?:string};
  playtimeSec?:number; lastPlayed?:string; caps:{install:boolean;pause:boolean;cancel:boolean;uninstall:boolean} }
```
**JSON-RPC (Shell → Core):**
```json
{"jsonrpc":"2.0","id":7,"method":"library.list","params":{"provider":"steam","filter":"installed","page":{"cursor":null,"limit":60}}}
{"jsonrpc":"2.0","id":8,"method":"item.launch","params":{"id":"steam:570"}}
{"jsonrpc":"2.0","id":9,"method":"item.install","params":{"id":"steam:1245620"}}
```
**Eventos (Core → Shell):** `item.changed`, `install.progress {id, bytes, total, bps, etaSec}`, `game.started {id}`, `game.exited {id, code, playSec}`, `session.error`, `hardware.changed`.
**Regras:** Shell envia **IDs, nunca comandos**; Core resolve `id → LaunchPlan` do próprio banco; `argv` é vetor (sem `sh -c`); códigos `%u/%f` do `.desktop` expandidos pela spec.

**Hardware Profiles (item 15) — saída do módulo `hardware`:**
```json
{ "architecture":"x86_64", "performanceTier":"high",
  "display":{"max":"3840x2160","vrr":true,"hdr":false},
  "memoryMb":16384, "gpu":{"vendor":"amd","vulkan":"1.3","driver":"radv"},
  "effects":"full",
  "capabilities":{"steam":true,"vulkan":true,"android":false,"ps2":true,"ps3":false,"emulation":true},
  "providersAvailable":["steam","native","emulation"] }
```
Tier: por classe de GPU (iGPU antiga × dGPU), Vulkan, RAM e núcleos. `ps2` exige AVX2 e Vulkan; `android` exige módulos de kernel do Waydroid e ≥4 GB. A Shell só lê `capabilities` e `effects` (`full|reduced|minimal` — troca animação/blur/resolução de arte), **nunca o nome do hardware**.

---

## 10. ESTRUTURA DE REPOSITÓRIOS

Um **monorepo** (`/root/nexus/`; o `retropad` fica intacto):
```
nexus/
├── apps/
│   ├── shell/            Electron + SolidJS (TV)
│   └── remote/           PWA do celular (gamepad, mouse/teclado) — tema NexuOS
├── packages/
│   ├── ui/               tokens CSS (claro/escuro), componentes compartilhados Shell+Remote
│   ├── focus/            Focus Engine (extraído do protótipo)
│   └── protocol/         tipos TS gerados do Rust + JSON Schema
├── core/                 workspace Cargo
│   └── crates/ core · protocol · supervisor · hardware · remote · db · sysd
│                providers/{steam,native,emulation}
├── installer/            nexus-installer (Rust CLI)
├── system/               session/, systemd/, polkit/, udev/, apparmor/, sway/, packaging/{nfpm,PKGBUILD,spec}
├── tools/                mock-core (serve os dados do protótipo), harness e2e (CDP)
├── docs/{architecture,decisions(ADR),specifications}
└── tests/                e2e, contrato de providers, fixtures ACF/VDF
```
Ferramentas: `pnpm` workspaces + Cargo workspace + `just`. CI: build/test Rust, build Shell, e2e contra mock-core, empacotamento deb/rpm.

---

## 11. ROADMAP

> Mapeamento: **MVP = Fases 0–5** (Ubuntu 24.04, AMD/Intel) · **V2 = Fase 6 + Remote + Fedora/Arch/NVIDIA + Native Provider** · **V3 = Fase 7** · **V4 = NexusOS (imagem)**.

**FASE 0 — Preparação**
- *Objetivo:* decisões e contratos fixos. *Tecnologias:* Markdown/ADR, JSON Schema, ts-rs.
- *Tarefas:* criar monorepo; `ARCHITECTURE.md`; ADRs 001–010; esquema do protocolo; **mock-core** com os dados do protótipo; rodar **spikes S1–S5 em hardware real**.
- *Entregáveis:* repositório, ADRs, mock-core, relatório de spikes.
- *Conclusão:* Shell roda contra o mock; spikes S1 (instalação Steam), S2 (ida e volta), S4 (gate da Shell) com decisão registrada.

**FASE 1 — Shell**
- *Objetivo:* Shell de produção. *Tecnologias:* Electron, SolidJS, TS, Vite.
- *Tarefas:* portar tokens/tema, `@nexus/focus` (+ engajamento, overrides), telas (Início/Biblioteca/Hub/Loja-Steam/Ajustes/Central), **virtualização** da grade, i18n, estados de erro/vazio/carregando, hardening (CSP, `contextIsolation`), sons e cursor.
- *Entregáveis:* app Shell + testes.
- *Conclusão:* e2e e fuzz de 900 teclas sem foco preso; ≥55 fps; Shell ≤400 MB ocioso; 1º frame ≤3 s.

**FASE 2 — Core**
- *Objetivo:* daemon real. *Tecnologias:* Rust, tokio, rusqlite, zbus.
- *Tarefas:* servidor JSON-RPC/Unix socket; SQLite + migrações; módulo `hardware`; `supervisor` (escopo systemd + `cgroup.events`); `sysd` com polkit; reconciliação após crash.
- *Entregáveis:* binários + units systemd.
- *Conclusão:* Shell conversa com o Core real; abrir um app qualquer e voltar (com `xterm`/app de teste).

**FASE 3 — Steam**
- *Objetivo:* o caminho completo do MVP. *Tecnologias:* steamlocate, VDF, Web API/CEF, inotify.
- *Tarefas:* detectar; listar instaladas; arte/metadados; lançar; **fim de jogo** (reaper/cgroup); instalar + progresso (conforme S1); primeiro login por handoff.
- *Entregáveis:* `providers/steam`.
- *Conclusão:* **jogo Steam real (nativo e Proton) abre, assume a tela, fecha e volta ao Nexus**, sem desktop aparecer; instalação com barra de progresso.

**FASE 4 — Sessão**
- *Objetivo:* boot direto. *Tecnologias:* sway, systemd, SDDM/GDM.
- *Tarefas:* `nexus.desktop`, `sway-kiosk.conf`, units, **modo avançado**, guarda de loop, autologin com backup/reversão.
- *Conclusão:* VM e PC real: login automático chega ao Nexus; "Abrir Desktop" e recuperação funcionam.

**FASE 5 — Installer**
- *Objetivo:* instalação real. *Tecnologias:* Rust, nfpm, apt.
- *Tarefas:* detecção, requisitos, instalação de Base/Steam, perfil AppArmor, repositório apt, `nexus doctor`, atualização pelo apt.
- *Conclusão:* **instalação limpa em Ubuntu 24.04** chega ao Nexus em fullscreen com controle, vê a biblioteca Steam, instala e abre um jogo (**= MVP**).

**FASE 6 — Emulação (V2)**
- *Objetivo:* ROMs. *Tecnologias:* RetroArch, `systems.json`/ES-DE, SteamGridDB/ScreenScraper.
- *Tarefas:* varredura, hash, metadados, lançar RetroArch; **Native Linux Provider**; **Remote** (QR, uinput P1/P2/mouse/teclado); Fedora/Arch; NVIDIA.
- *Conclusão:* ROM abre e volta; celular controla Shell e jogo; matriz Fedora/Arch verde.

**FASE 7 — Expansão (V3+)**
- Múltiplos Providers (Epic/GOG via Heroic CLIs), HAL completo (tiers/efeitos), Media, Android (Waydroid) **se fizer sentido**, gamescope "Gaming Mode", InputPlumber, Shell Qt p/ hardware fraco/ARM, **NexusOS** (imagem imutável).

---

## 12. MVP

**Menor produto funcional (exatamente o seu fluxo):**
```
INSTALAR (Ubuntu 24.04, AMD/Intel) → sessão Nexus em fullscreen → CONTROLE físico (Gamepad API)
→ DETECTAR Steam → MOSTRAR jogos (instalados + dona-mas-não-instalados) → INSTALAR com progresso
→ ABRIR jogo real → RETORNAR ao Nexus
```
**Dentro:** Shell (Início, Biblioteca, Hub, Ajustes básicos, Central), Core, Steam Provider, sway-kiosk, instalador Ubuntu, modo avançado mínimo (terminal, desktop, recuperação), capabilities mínimas.
**Fora (de propósito):** emulação, apps Linux, **celular**, Fedora/Arch/NVIDIA, ARM, Android, Media, múltiplos usuários, atualização in-app, imagem própria.

---

## 13. RISCOS

| Área | Risco | Mitigação |
|---|---|---|
| **Steam** | Instalar sem a UI da Steam pode não ser possível de forma estável; API interna (CEF) não é suporte oficial; Web API exige chave e perfil público ✔ | **S1**; *handoff* como plano B; Provider isola; ler ACF independe disso |
| **Steam/ToS** | Uso de CDN/API/CEF fora do suporte oficial; marca da Valve | Sem redistribuir assets; aviso "não afiliado"; Web API só com chave própria |
| **Proton/Wine** | Helpers do Wine nunca saem ✔; launchers (Epic/Battle.net) abrem/fecham; apps de instância única | Fim de sessão por (processos **e** janela), *grace period*, "forçar encerramento" via cgroup |
| **Wayland** | Comportamento de janela fullscreen/foco varia; sandbox do Chromium; Electron em fullscreen já teve congelamentos em KDE ✔ (resolvido "works-for-me") | sway com regras fixas; testar no S2/S4 |
| **NVIDIA** | wlroots historicamente delicado; WebKitGTK problemático ✔ | MVP só AMD/Intel; S5; plano B GNOME Kiosk/KWin |
| **Ubuntu 24.04** | AppArmor bloqueia sandbox do Electron ✔ | Perfil `userns` no pacote (carregado no postinst) |
| **Autologin** | Comportamento por gerenciador (SDDM `Session=` é o nome do arquivo ✔; GDM/AccountsService; relatos de bugs ◐) | Matriz de testes (S5), backup/reversão, guarda de loop |
| **Input** | Steam aberta pode "sequestrar" controle ✔ (relatos); conflito Nexus×jogo | S3; leitura evdev não-exclusiva; Shell só com foco |
| **Distros** | Pacotes/repos/multilib divergem | 1 distro no MVP; adaptadores finos |
| **ARM** | Steam é x86 (?; FEX/Box64 não pesquisado); Electron arm64 existe; GPU/drivers variam | Fora do MVP; Shell Qt futura |
| **Desempenho** | Electron ocioso pesa; Pi-class inviável | Gate S4; `effects` por tier; matar/reiniciar Shell em RAM baixa |
| **Segurança** | Metadados externos na UI (XSS); LAN do Remote | Componentes com escape, CSP, sem URLs remotas; pareamento por token |
| **Licença** | GPL (Pegasus/InputPlumber/Heroic) contamina se linkada | Chamar como processo; licença do Nexus a definir |
| **Projeto** | Escopo e mantenedor único | MVP estreito; ADRs; testes automáticos |

---

## 14. DECISÕES CRÍTICAS

**O QUE FAZER AGORA**
1. Criar o monorepo e **congelar contratos**: protocolo JSON-RPC, trait `Provider`, esquema de `capabilities`.
2. **mock-core** com os dados do protótipo → desenvolver a Shell em paralelo ao Core.
3. Rodar os **spikes em hardware real** (este ambiente é headless; sem GPU/Steam não valida S1–S5).
4. Escolher **uma** distro/GPU alvo (Ubuntu 24.04 + AMD/Intel) e perfil AppArmor.

**O QUE NÃO FAZER AGORA**
- Distro própria, Buildroot/Yocto, Android/Waydroid, compositor novo, gamescope como sessão, InputPlumber, Epic/GOG, emulação, celular, ARM, atualizador in-app, múltiplos usuários, suporte a "todas as distros".

**O QUE VALIDAR PRIMEIRO (ordem)**
- **S1 — Instalar jogo Steam sem a UI:** testar `steam://install` (abre diálogo?), API CEF, steamcmd, *handoff*; mapear `StateFlags`; confirmar `SteamGameId`/`RunningAppID`.
- **S2 — Ida e volta** sway + jogo Steam/Proton real (nativa × Flatpak): fim de sessão, foco, workspace.
- **S3 — Controle:** Gamepad API só com foco; Steam aberta atrapalha? uinput aparece como Xbox 360 p/ SDL/Steam/Proton?
- **S4 — Gate da Shell:** Electron em RAM ociosa ≤400 MB, 1º frame ≤3 s, ≥55 fps (senão → Qt/QML).
- **S5 — Autologin** SDDM/GDM/greetd + NVIDIA; perfil AppArmor em Ubuntu 24.04.

**Remote (pós-MVP, já desenhado):** pareamento por QR, P1/P2/mouse/teclado via `uinput` (regra udev), protocolo herdado do RetroPad, UI no tema NexuOS compartilhando `@nexus/ui`; HTTPS/PWA e *wake lock* exigem contexto seguro → HTTP na LAN + NoSleep + túnel opcional (de `tunnel.py`). Teclado via uinput segue layout US (ç/ã ignorados).

## O que não foi confirmado
`steam://install` (diálogo), `SteamGameId`/`RunningAppID`, demais `StateFlags`, cache de arte local da Steam, `cgroup.kill`, comportamento da Gamepad API do Chromium sem foco, uinput como Xbox 360 em SDL/Proton, licenças marcadas "conferir", números de RAM do Electron, Steam em ARM. As buscas foram parciais; **nada disso foi tratado como decidido.**

## Execução após aprovação
Criar `/root/nexus/` com `docs/architecture/ARCHITECTURE.md` + ADRs (conteúdo deste plano), esqueleto do monorepo, `mock-core` e o port inicial da Shell (foco + tema) rodando contra o mock, com o harness e2e existente. Os spikes S1–S5 ficam como checklist para hardware real.

## Fontes
[ChimeraOS gamescope-session](https://github.com/ChimeraOS/gamescope-session) · [Bazzite](https://www.answeroverflow.com/m/1429583688622145710) · [OpenGamepadUI](https://github.com/ShadowBlip/OpenGamepadUI) · [InputPlumber](https://inputplumber.readthedocs.io/) · [gamescope/Steam nested](https://github.com/ValveSoftware/gamescope/wiki) · [Steam ACF](https://docs.rs/game-detector/0.2.1/src/game_detector/steam.rs.html) · [Steam CLI](https://developer.valvesoftware.com/wiki/Command_line_options_(Steam)) · [steam-runtime](https://gitlab.steamos.cloud/steamrt/steam-runtime-tools/-/blob/0cc0c2cea1234ece877629343a0955fd2606fbb8/docs/steam-compat-tool-interface.md) · [Steam Reaper](https://www.github.com/sonic2kk/steamtinkerlaunch/wiki/Steam-Reaper) · [Decky CEF](https://wiki.deckbrew.xyz/plugin-dev/cef-debugging) · [Heroic](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher) · [Pegasus](https://github.com/mmatyas/pegasus-frontend) · [Norigin](https://devportal.noriginmedia.com/docs/Norigin-Spatial-Navigation/) · [Tauri Linux](https://v2.tauri.app/develop/debug/linux-graphics/) · [Electron Wayland](https://electronjs.org/blog/tech-talk-wayland) · [Ubuntu userns/Electron](https://kicey.github.io/blog/2025/06/04/fix-the-electron-fiddle-issue-about-user-namespace-privileges-on-ubuntu-2404) · [electron-updater](https://www.electron.build/docs/api/electron-updater) · [GNOME Kiosk](https://blogs.gnome.org/shell-dev/2025/09/10/gnome-kiosk-updates/) · [greetd](https://wiki.archlinux.org/title/Greetd) · [cage](https://man.archlinux.org/man/cage.1.en) · [sddm.conf](https://manpages.debian.org/trixie/sddm/sddm.conf.5.en.html) · [GDM autologin](https://docs.redhat.com/en/documentation/Red_Hat_Enterprise_Linux/7/html/desktop_migration_and_administration_guide/user-sessions) · [PackageKit status](https://blogs.gnome.org/hughsie/?p=248) · [cgroup v2](https://grok.openbmc.org/xref/openbmc/linux/Documentation/admin-guide/cgroup-v2.rst?r=113094f7) · [RomM metadata](https://docs.romm.app/5.1.0/getting-started/metadata-providers/) · [RetroArch CLI](https://docs.libretro.com/guides/cli-intro/) · [steamlocate](https://docs.rs/crate/steamlocate/2.0.1/source/src/lib.rs) · [Steam on Linux (Valve)](https://developer.valvesoftware.com/wiki/Steam_under_Linux)
