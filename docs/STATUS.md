# Estado da implementação

Legenda: ✔ feito e testado aqui · ◐ feito, **não validado em hardware real** · ✗ não feito

| Área | Estado | Observações |
|---|---|---|
| Core: perfis, PIN (Argon2), bloqueio por tentativas, controle parental, tempo diário | ✔ | testes unitários + e2e |
| Core: supervisor de jogos (sessão própria, fim por processos, fechar/forçar, tempo de jogo) | ✔ | e2e com processos reais |
| Provider Apps Linux (.desktop/Flatpak) | ✔ | parser da spec XDG, filtros, lançamento por argv |
| Provider Steam (ACF/VDF, bibliotecas, progresso, Web API opcional) | ✔ dados · ◐ cliente | testado com uma "Steam falsa"; `steam://install` sem diálogo **não confirmado (spike S1)** |
| Provider Demo (catálogo do protótipo, loja, carteira) | ✔ | só com `--demo` |
| Celular: QR, token, gamepad P1/P2, mouse, teclado, rolagem | ✔ | e2e com celular emulado; uinput real ◐ (sem /dev/uinput aqui) |
| Fallback sem uinput: celular controla a Shell (cursor virtual) | ✔ | |
| Shell: Início, Biblioteca, Mídia, Loja, Hub, Busca, Central, Notificações, Ajustes, Execução | ✔ | |
| Shell: perfis, perfil infantil (tema, limites, "hora de descansar"), PIN, primeiro uso | ✔ | |
| Shell dentro do Electron (ponte segura, protocolo de arte) | ✔ | validado sob Xvfb; **RAM medida: ~750 MB** (software GL) — acima da meta de 400 MB, reavaliar com GPU real (S4) |
| Sessão: nexus.desktop, sway kiosk, units, guarda de loop | ◐ | escritos, **nunca executados num compositor real** |
| Instalador: detectar, requisitos, plano, autologin SDDM/GDM/greetd com backup e reversão | ✔ lógica · ◐ execução | testado em diretórios temporários |
| Pacote único (`tools/bundle.sh`: .deb + install.sh) | ✔ gerado · ◐ execução | `install.sh` e os comandos de apt/flatpak/pacman **nunca foram executados** num sistema real |
| Instalador: multiverse, Flatpak+Flathub, multilib, RPM Fusion | ◐ | escritos e testados só como plano |
| Pacote .deb (`tools/build-deb.sh`) | ✔ gerado · ◐ instalação | binários do pacote extraído rodam (Core + Electron); `apt install` e a sessão no login **não testados** |
| AppArmor, polkit, udev | ◐ | incluídos no .deb, nunca carregados num sistema real |
| Nexus Sysd (ações privilegiadas via D-Bus + polkit) | ✗ | só a política polkit existe; o serviço não foi implementado |
| Botão Guia com jogo aberto (evdev) | ◐ | código pronto, sem controle físico para testar |
| gamescope, NVIDIA, Fedora/Arch, emulação (RetroArch), atualização in-app | ✗ | V2+ conforme o plano |

## Atualização 0.2.0
| Área | Estado | Observações |
|---|---|---|
| Apps Linux só se adicionados (nome, tipo, ícone, capa do card, fundo) + navegador de imagens | ✔ | e2e Core + UI |
| Loja da Steam (mais vendidos, ofertas, grátis, Linux, lançamentos) com preços em R$ | ✔ | testado ao vivo na rede pública; pago → "Ver na Steam" |
| Jogos da conta não instalados | ◐ | exige chave da Web API; sem chave só jogos já abertos (localconfig) |
| Abrir jogo Steam (caminho /usr/games/steam, Flatpak, janela da Steam, motivo do erro) | ◐ | corrigido, **não reproduzido nem confirmado no aparelho** |
| Provedor de apps web, emulação, Sysd | ✗ | ver HANDOFF.md §10 |

## Desempenho (Nexus leve: o máximo para os jogos)

Medição: `tools/measure.sh [s]` na sessão real (RAM = PSS somado dos processos de cada serviço; CPU = uso do cgroup
no intervalo, em % de **um** núcleo). Tempos de abrir/fechar: RPC `item.launch` → jogo em foco no sway; `game.close`
→ Nexus em foco (média de 3 rodadas). Máquina: VM VirtualBox, 5 núcleos, 11 GB, sem GPU (Shell com `--disable-gpu`),
**sem swap**. App de teste: YouTube (Chrome em modo TV). **Raspberry Pi 4/5: ainda não medido** (sem aparelho aqui).

### Item 1 — liberar recursos durante o jogo (2026-11-12)

| Com o YouTube aberto | Antes (0.4.1) | Depois | |
|---|---|---|---|
| Shell — CPU | 8,59 % | **0,00 %** (congelada) | ✔ |
| Shell — RAM (PSS) | 182 MB | 168–193 MB | ✗ sem ganho sem swap (112 MB são anônimos); ver item 2 (zram) |
| Core — CPU | 1,70 % | **0,25–0,53 %** | ✔ |
| Nexus inteiro — CPU | 10,3 % | **0,6 %** | ✔ |
| Abrir (até o app em foco) | 3,8–4,2 s | 3,2–3,7 s | ≈ (dominado pelo próprio app) |
| Fechar (até o Nexus em foco) | 0,35–0,58 s | **0,17–0,22 s** | ✔ |
| Menu rápido (F1) com a Shell congelada | — | 0,36 s até o menu | ✔ |

O que mudou:
- `freeze.rs`: a Shell é **congelada** (`cgroup.freeze`) 1,5 s depois de o jogo aparecer e descongelada antes de
  aparecer de novo (F1, fim do jogo, "Ir ao Início"); `memory.reclaim` devolve o que der. Congelar em vez de encerrar
  mantém o menu rápido instantâneo e o estado (foco, tela) sem precisar mover nada para o Core.
- `nexus.slice` (CPUWeight/IOWeight 30) para Core, Shell e overlay; escopos do jogo e da Steam com peso 1000.
- gamemode: o Core registra os processos do jogo no `gamemoded` (D-Bus) quando ele aparece e tira ao fechar
  (`gamemode` nas Recomendações do `.deb`). Aqui o registro funciona ("gamemode is active"); a VM não tem cpufreq,
  então o ganho de governador não é mensurável aqui.
- Jogo aberto: varredura da biblioteca a cada 60 s (era 6 s), sem busca de atualizações de apps; o rastreador do
  jogo lê o `cgroup.procs` do escopo (era varrer o /proc inteiro a cada 0,4 s) e checa a cada 1 s, acordando na hora
  ao fechar.
- `grim` (quadro do menu rápido) com limite de 1,5 s: com a tela bloqueada ele travava o F1 para sempre.

Pendente do item 1: `apt-daily` fora do horário de jogo (precisa de root: vai no item 2); modo "encerrar a Shell"
para aparelhos de ≤ 2 GB (exige mover estado para o Core — proposta, não feito).

Metas (a confirmar no Pi): com jogo aberto, Nexus ≤ 1 % de CPU e ≤ 100 MB de RAM residente (com zram); fechar ≤ 0,5 s.

### Item 2 — sistema base enxuto (2026-11-12)

`system/sysd/nexus-slim` (root, reversível, `--slim` por padrão no `install.sh`; `--sem-otimizar` tira) + greetd
quando não há gerenciador de login (base Debian netinst) + `docs/BASE.md` (base oficial: Debian estável netinst sem
desktop). Teste: `installer/tests/slim.rs` (aplica, idempotente, cups fica com impressora, `idle` com jogo aberto,
revert só apaga o que criou) e plano `slim_and_greetd_on_a_bare_debian`.

| RAM ociosa do sistema (VM Ubuntu 26.04 + GNOME + sessão Nexus aninhada) | Antes | Depois |
|---|---|---|
| `free` usada | 3955 MB | **pendente**: aplicar exige sudo (`sudo /home/vboxuser/nexus/system/sysd/nexus-slim apply`) |
| Serviços dispensáveis rodando (memória do cgroup) | cups+cups-browsed 24 MB, ModemManager 13 MB, colord 20 MB, avahi 1 MB, indexador localsearch | — |
| Swap | nenhuma | zram (metade da RAM, zstd) |

A VM de desenvolvimento não é a base oficial (tem GNOME): o número que importa é o da base Debian sem desktop,
**ainda não medido** (precisa de uma VM/aparelho limpo).

### Item 3 — peso da Shell (2026-11-12)

Shell ociosa no Início (sessão real, VM sem GPU): **146–171 MB (PSS), ~1,1–1,3 % de CPU, 5–7 processos**. Por processo,
quase toda a CPU é o processo de GPU compondo por software (`--disable-gpu` na VM); o renderizador fica em 0–2 ticks
em 30 s. Com jogo aberto a Shell fica congelada (item 1): 0 %.

Tentativas medidas e **desfeitas por não terem ganho**:
- Opções do Chromium (`disable-features` MediaRouter/Translate/…, `renderer-process-limit=1`, `disable-background-networking`,
  `js-flags=--max-old-space-size=160`…): 146 MB → 146 MB, CPU igual.
- Laço do controle (`requestAnimationFrame` sempre ligado) só com controle conectado: renderizador 0 → 0–2 ticks/30 s,
  GPU 31 → 34 ticks: sem diferença mensurável (o Chromium já não desenha quadros sem mudança).

Comparação de motores com a **mesma** interface (`tools/compare-engines.sh`: Core de demonstração, sway headless com
pixman, mesma tela, 90 s depois de abrir):

| Motor | RAM (PSS) | CPU | Processos | Observação |
|---|---|---|---|---|
| Electron 45 (atual) | 171 MB | 19,3 % | 5 | ponte segura com o Core (preload) |
| Chromium/Chrome `--kiosk --app` | 169 MB | 18,1 % | 11 | precisaria da Shell falando com o Core por WebSocket (exposto em TCP) e de um navegador instalado |
| WebKitGTK 4.1 (motor do Tauri) | 256 MB | 14,6 % | 3 | inclui ~15 MB do Python do teste; Tauri exigiria reescrever a ponte em Rust |

A CPU alta dos três é do compositor headless sem limite de quadros (na sessão real a mesma Shell fica em ~1 %); serve
só para comparar entre eles. **Recomendação: manter o Electron.** Nenhuma alternativa ganha em RAM de forma clara
(Chromium empata, WebKitGTK gasta mais) e todas custam reescrever a ponte. O ganho grande já veio de congelar a Shell
durante o jogo. Repetir a comparação no Raspberry Pi (com GPU V3D) antes de fechar a questão.

### Item 4 — compositor e lançamento de jogos (2026-11-12)

Feito e testado:
- **Shell volta a ter a tela quando sobe sem jogo aberto** (`core.hello` → `show_shell`). Visto nesta sessão: a Shell
  reiniciou com a área dos jogos em foco e a TV ficaria preta. Teste real: área 2 em foco → Shell reinicia → área 1.
- **Jogo travado**: "Fechar jogo" pede com SIGTERM e força com SIGKILL depois de 5 s; agora também mata o escopo
  systemd inteiro do jogo (`systemctl --user kill --signal=SIGKILL nexus-game-*.scope`), pegando processos que saíram
  da árvore. O menu rápido abre mesmo com o jogo travado (botão Guia/Select+Start lidos direto do evdev, F1 pelo
  sway; nada depende da janela do jogo responder).
- e2e `hung_game_is_killed_and_crash_returns`: jogo que ignora SIGTERM com filho em outra sessão (`setsid`) é encerrado
  entre 4 e 9 s sem sobrar processo; jogo que cai com SIGSEGV volta sozinho; dá para abrir outro em seguida.
- Já existia: regras do sway (`system/sway/nexus-kiosk.conf`: Steam e avisos de falha no scratchpad, sem foco
  automático de janelas novas), tela "Abrindo…" até a janela do jogo estabilizar, `show_shell` repetido por 3 s ao
  fechar.

**gamescope — avaliação (não implementado, decisão de arquitetura):** ver `docs/decisions/0004-gamescope.md`.

### Item 5 — distribuição pelo apt (2026-11-12)

Pronto e testado localmente, **publicação aguardando o dono** (precisa gerar a chave, criar o segredo no GitHub e
ligar o Pages — ações fora desta máquina): `tools/apt-repo.sh`, `.github/workflows/apt-repo.yml`, `docs/APT.md`,
`update.sh` configura o repositório sozinho quando ele estiver no ar. Teste com `apt-get update` real: assinatura
aceita, candidato 0.4.1, download ok; índice adulterado e chave errada recusados. Falta o aceite final ("instalar e
atualizar só com apt numa máquina limpa"), que depende do repositório publicado e de uma VM limpa.
