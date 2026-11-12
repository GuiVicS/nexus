# Nexus 0.4 — "carenagem de console": estudo + plano de ação

## Contexto
Nos testes, o usuário encontrou problemas:
- Ao sair de um jogo da Steam, aparece a Steam em vez do Nexus.
- É difícil voltar ao Nexus ou à tela anterior.
- Ao instalar um jogo, a janela da Steam aparece.
- O RetroArch mostra uma tela grande "RETROARCH" antes do jogo.
- As ROMs aparecem misturadas na Biblioteca, como cards coloridos sem nome nem capa (são os 3 cards vazios que vimos).
- Os botões flutuantes ficam sempre visíveis no YouTube.

O que ele quer:
- Comportamento de console: a Steam e o RetroArch funcionam só como "motor" e nunca aparecem.
- Abrir e fechar qualquer coisa sempre volta ao Nexus.
- Um menu rápido com foco só nele, sobre o fundo desfocado do jogo, aberto e fechado pela mesma tecla.
- Uma aba **Emuladores** no estilo do EmulationStation: escolher o console e ver os jogos dele com capa.
- A Loja com cara de Play Store.
- Uma tela para gerenciar ROMs: pendrive, envio pelo celular com QR code e link.

Decisões do usuário:
- **F1** no teclado abre e fecha o menu rápido.
- No controle, a tecla fica a meu critério. Decidi: **botão Guia (PS/Xbox)** e, para controles sem ele, **Select+Start juntos**.
- Jogo sem capa: arte do console + nome do jogo.

Estado atual: há uma edição pela metade, sem commit, em `core/.../compositor.rs` e `rpc.rs` (confirmar sozinho o "Install" da Steam). Ela será reaproveitada e terminada na Fase 3.

---

## PARTE 1 — Estudo (o que pesquisei e encontrei no código)

### 1.1 Por que a Steam aparece ao sair do jogo (causa no código)
- `system/sway/nexus-kiosk.conf` manda **toda** janela X11 para o workspace 2.
- A regra `for_window [floating shell="xwayland"] fullscreen enable, focus` faz o seguinte: quando o jogo fecha, a Steam reabre a janela principal ou diálogos de nuvem, e esse `focus` **leva a tela ao workspace 2**, por cima do `show_shell()` que `game.rs::track_game` acabou de fazer.
- O mesmo acontece com o "Updating Steam" ao ligar, o bug que vimos 2 vezes.

### 1.2 Como esconder a Steam (pesquisa)
- O SteamOS esconde a Steam porque ela **é** a interface, rodando dentro do gamescope.
- Para quem usa a Steam como motor, as opções da Valve (`-silent`, `-nofriendsui`, `-nochatui`) só valem para a inicialização. Elas **não** impedem a janela principal de voltar quando o jogo fecha ([Valve: Command line options](https://developer.valvesoftware.com/wiki/Command_line_options_(Steam))).
- O Lutris usa um script que vigia o jogo e fecha a Steam ([fórum Lutris](https://forums.lutris.net/t/launch-steam-games-from-lutris-for-purists/22150)).
- **Conclusão:** quem tem que esconder é o compositor. No sway, as janelas da **Steam** (classe `steam`) vão para o **scratchpad**, que fica sempre invisível. Os **jogos** têm classe própria (`steam_app_N` ou a do jogo) e continuam no workspace 2.
- A Steam só aparece quando o Core quiser, por baixo de uma capa do Nexus.
- O diálogo "Install" (540×480, botões em posição fixa, medido na VM) é confirmado por clique do sway por baixo de uma capa do overlay que deixa o clique passar.
- A API interna `SteamClient.Installs` (via depuração CEF, como faz o Decky Loader) não tem documentação pública confiável ([Decky wiki](https://wiki.deckbrew.xyz/en/user-guide/install)). Fica fora por ser frágil.

### 1.3 Como o EmulationStation usa o RetroArch (pesquisa)
- O ES-DE abre `retroarch -L <núcleo> <rom>` por regra de sistema (`%EMULATOR_RETROARCH% -L %CORE_RETROARCH%/x_libretro.so %ROM%`, em `es_systems.xml`) ([RetroDECK docs](https://retrodeck.readthedocs.io/en/latest/wiki_experiments/external-emulator/external-emulator/), [ES-DE guia](https://repo.retrodeck.net/Xargon/ES-DE/src/commit/58fd5b8f101b52b99653028100502beabb9b43fc/USERGUIDE.md)).
- O ES-DE fica com a própria tela por cima enquanto o emulador carrega e só sai da frente quando o jogo já está desenhando.
- A navegação é: **lista de sistemas → lista de jogos do sistema com capa**, com capas do libretro-thumbnails (o Nexus já baixa essas capas em `providers/emu.rs::art`).
- O Nexus hoje faz o mesmo `-f -L` (`emu.rs::launch`), mas mostra o workspace 2 assim que o processo existe (`track_game`). Por isso aparece o "RETROARCH" do menu ou da tela de carga.
- **Conclusão:**
  - Uma config própria do Nexus (`--appendconfig`) desliga a animação de carga, os avisos e o menu ao iniciar.
  - O Core só troca para o jogo quando a **janela** do jogo existe há cerca de 1,2 s.
  - Até lá, o Nexus mostra a capa "Abrindo…".

### 1.4 Como os consoles fazem menu, volta e saída (pesquisa)
- **PS5:** o botão PS abre a **Central de Controle** por cima do jogo, sem fechá-lo. Segurar leva à tela inicial com o jogo suspenso. Só um jogo fica suspenso ([GamesRadar](https://gamesradar.com/ps5-ui-control-center-activities-impressions-opinion), [SeekingTech](https://seekingtech.com/how-to-close-apps-or-games-on-ps5/), [PopSci](https://www.popsci.com/reviews/xbox-series-x-vs-ps5/)).
- **Xbox:** o botão Guia abre o **Guia lateral**. O Quick Resume suspende jogos, mas não apps de mídia ([Screen Rant](https://screenrant.com/how-ps5s-switcher-is-different-from-xbox-series-xs-quick-resume/)).
- **Regra comum:**
  1. Abrir um jogo ou app sempre passa por uma tela de "abrindo" do console.
  2. Fechar volta sempre à tela inicial, no card do jogo.
  3. Uma tecla única abre o menu sobre o jogo desfocado, com foco preso no menu, e a mesma tecla devolve ao jogo.
  4. Não há botões flutuantes permanentes.
  5. Ir ao Início deixa o jogo rodando, com "Retomar" no topo.
- No código, a Central já mostra um quadro do jogo (`guide.rs::pressed` captura com `grim`, `Overlays.tsx::Guide` mostra `.gframe`). Ela não funcionava no teste porque a tecla Super é capturada pelo GNOME e porque os botões flutuantes sempre visíveis (`system/overlay/nexus-overlay`) competiam com ela.

---

## PARTE 2 — Plano de ação (fases em ordem; cada fase testada antes da próxima)

### Fase 1 — Steam e apps nunca aparecem; sempre volta ao Nexus
- `system/sway/nexus-kiosk.conf`:
  - `for_window [class="^steam$"] move scratchpad` (janela principal, updater, nuvem e amigos).
  - Remover o `focus` automático dos flutuantes, para que só o Core decida a tela.
  - `bindsym F1 exec /usr/bin/nexus-core guide`, além do Super já existente.
- `core/.../compositor.rs`: funções `steam_peek(con)` e `steam_hide()`, que tiram e devolvem janelas do scratchpad.
- `game.rs::track_game`:
  - Antes de `show_game()`, esperar a **janela** do jogo existir há cerca de 1,2 s (nova função `game_window_ready(tracker)`, que procura no `sway_windows()` uma janela que não seja da Steam nem do Nexus).
  - Ao sair, chamar `show_shell()` e repetir a chamada por 3 s, para nada roubar a tela.
- `game.rs` também trata o "jogo Steam esperando" (o aviso dos 7 s): em vez de mostrar a Steam, a Shell mostra "A Steam está se preparando…". Se a Steam pedir login, a Shell oferece o login por QR que já existe.
- Apps de streaming (`providers/web.rs`): conferir que o app fecha e volta ao Nexus quando o "Sair" do YouTube TV chama `window.close()`. Se não fechar, o Core faz isso pelo menu rápido.

### Fase 2 — Menu rápido (F1, Guia, Select+Start)
- `core/.../guide.rs`:
  - Ler também **Select+Start** apertados juntos (BTN_SELECT 0x13a + BTN_START 0x13b) pelo evdev passivo que já existe.
  - `pressed()` vira um **alternar**: se a Shell já está na frente com o menu sobre o jogo, emite `input.guide {close:true}` e volta ao jogo.
- `system/overlay/nexus-overlay`: **remover** a pílula de botões sempre visível. O overlay passa a servir só para a capa da Fase 3.
- Shell:
  - `Overlays.tsx::Guide` vira o "Menu rápido" lateral, com `.gframe` desfocado (`filter: blur(18px) brightness(.55)` em `packages/ui/src/theme.css`) e foco preso no painel (padrão `ret` de `packages/focus`).
  - Itens do menu: **Retomar**, **Ir ao Início** (o jogo continua rodando e o Início mostra o card "Retomar" no topo), **Fechar jogo**, volume, controles, notificações, trocar perfil e energia.
  - `lib/session.ts` trata `input.guide` com `close`.
  - `Overlays.tsx::Launch`: trocar a dica do rodapé para "Menu: botão Guia, F1 ou Select+Start".
- Configuração do RetroArch do Nexus: tirar o Select+Start dele, para não sair do jogo junto.

### Fase 3 — Instalar sem ver a Steam (terminar a edição pela metade)
- `compositor.rs`: `steam_install_dialog`, `steam_install_points` e um teste, todos já escritos.
- `rpc.rs::steam_confirm_install`, já escrito:
  1. Emitir `steam.dialog {open,title}`.
  2. Tirar o diálogo do scratchpad, focar e clicar em Install.
  3. Se o download não começar (falta de espaço), clicar em Cancel e avisar "A Steam não começou a instalação · confira o espaço livre".
  4. Devolver o diálogo ao scratchpad.
- `nexus-overlay`: nova janela de capa em tela cheia na camada OVERLAY ("Instalando *Jogo*…" com a cara do Nexus).
  - A região de entrada fica vazia via ctypes, chamando `gdk_window_input_shape_combine_region` com `cairo_region_create`, porque o `python3-gi-cairo` não vem instalado. Assim o clique atravessa a capa.
  - Some sozinha depois de 60 s.
- A Shell mostra o progresso do download no card, como já faz.

### Fase 4 — RetroArch sem a tela "RETROARCH"
- `providers/emu.rs::launch` grava e passa `--appendconfig=<config>/nexus-retroarch.cfg` com:
  - `menu_show_load_content_animation = "false"`, `notification_show_*` = false, `video_font_enable = "false"`, `config_save_on_exit = "false"`, `video_fullscreen = "true"`;
  - menu do RetroArch só pela combinação definida (`input_menu_toggle_gamepad_combo`) e Select+Start livre (ver Fase 2).
- A espera pela janela da Fase 1 esconde o resto.

### Fase 5 — Aba "Emuladores" estilo EmulationStation
- `lib/actions.ts::TABS`: adicionar a aba `['emu','Emuladores']`, visível quando o RetroArch existe ou há ROMs. As ROMs **saem** da Biblioteca "Todos" (filtro por provedor `emu:`).
- Novo `apps/shell/src/components/Emu.tsx`:
  - **Tela 1:** grade de consoles que têm jogos (arte ou cor do console de `SYSTEMS[].hue`, nome e quantidade), mais o card "Adicionar jogos".
  - **Tela 2:** grade de jogos do console com capa do libretro-thumbnails. Sem capa, o card mostra a arte do console com o nome do jogo (componente `Tile` com novo modo `fallback="system"`).
  - B ou Esc volta para os consoles.
- Core: nova chamada `emu.systems` reaproveitando `emu.rs::systems_status()`. Os itens já têm `system` nas tags, ou será adicionado em `emu.rs` (`it.tags`).

### Fase 6 — Gerenciar ROMs: pendrive e celular
- Core, novo `core/.../roms.rs`:
  - `roms.media`: lista pendrives. Monta com `udisksctl mount -b`, sem sudo (o polkit permite na sessão ativa); senão lê `/media/$USER` e `/run/media/$USER`.
  - `roms.browse(path)`: lista ROMs reconhecidas pelas extensões de `SYSTEMS`.
  - `roms.import([paths])`: copia para `~/ROMs/<console>/`, com o console detectado pela extensão ou pasta (ambíguos, como `.iso` e `.bin`, perguntam). Emite progresso.
- Envio pelo celular, em `net.rs::remote_router`:
  - Rota `POST /remote/upload?t=<token>&system=<dir>`, protegida pelo token do QR que já existe (`remote.rs::Hub::token_ok`).
  - Grava em streaming com limite de 8 GB, nome higienizado e extensão permitida.
  - Página simples em `apps/remote`: escolher arquivo → enviar.
- Shell, tela "Adicionar jogos" (aba Emuladores, com atalho também na Loja):
  - Esquerda: pendrives e arquivos com seleção múltipla e "Copiar".
  - Direita: **QR code + link** (`remote.rs::qr_svg`/`url`) com o texto: **"Faça o upload de outro dispositivo: aponte a câmera do celular para o QR code ou abra o link abaixo no navegador de um aparelho na mesma rede Wi-Fi."**

### Fase 7 — Loja com cara de Play Store
- `Views.tsx::Store` é refeita:
  - **Carrossel de destaque** no topo (hero da Steam, já resolvido pela API de catálogo), depois **prateleiras horizontais** por categoria (Mais vendidos, Ofertas, Grátis, Nativos Linux, Lançamentos), cada uma com "Ver tudo".
  - Cards com capa, preço e desconto.
  - Busca no topo.
  - A coluna da esquerda vira chips de categoria.
- Reaproveitar `loadShop`, `Tile`, `PriceTag` e o `api.store(c)` já existentes.
- O Hub do jogo da loja ganha capturas e descrição; os dados vêm de `appdetails`, que já existe em `steam.rs::details`.

---

## Verificação (em cada fase, na sessão aninhada da VM + testes)
- `cargo test -p nexus-core`:
  - novos testes: regras do diálogo (`steam_install_points`), detecção de janela do jogo, Select+Start, `roms.import` (pasta temporária), upload com token inválido (403) e válido.
  - testes e2e existentes.
- `pnpm -C apps/shell build` + cenários de UI em `tests/ui` (`run.mjs`).
  - novos cenários: aba Emuladores (console → jogos → voltar) e menu rápido (abrir, Retomar, Ir ao Início).
- Teste real na VM com screenshots (`grim`):
  1. Ligar com a Steam atualizando → a tela fica no Nexus.
  2. Abrir e fechar o AdVenture Capitalist → volta ao Nexus sem ver a Steam.
  3. F1 durante o jogo → menu desfocado; F1 de novo → volta ao jogo.
  4. Instalar um jogo grátis pequeno → só a capa do Nexus aparece; sem espaço → aviso.
  5. Abrir a ROM de GB → sem tela "RETROARCH".
  6. Enviar ROM pelo navegador (link do QR) e por uma pasta simulando pendrive.
- Ao final:
  - Atualizar o `HANDOFF.md`.
  - Publicar a versão 0.4.0 (commit, push e release) **somente quando o usuário pedir**.
- Limites de teste: na VM não há pendrive real (vou simular com uma imagem montada via `udisksctl loop-setup`) nem controle com botão Guia (o celular emulado manda BTN_MODE e Select+Start).
