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

### Item 6 — Steam no Debian e no Ubuntu (2026-11-12)

- **Bug corrigido:** o instalador só ligava `contrib non-free` em `/etc/apt/sources.list`. No Debian 13 o padrão é o
  formato deb822 (`/etc/apt/sources.list.d/debian.sources`), então a Steam não aparecia. Agora os dois formatos são
  tratados (só linhas do Debian, sem duplicar, comentários e repositórios de terceiros intactos) — teste
  `debian_components_in_both_apt_formats`.
- **Checagem antes de instalar:** depois do `apt-get update`, `apt-cache policy steam-installer` precisa ter
  candidato; senão o instalador para com uma explicação em pt-BR (contrib/non-free no Debian, multiverse no Ubuntu,
  i386, ou `--sem-steam`) em vez de um erro cru do apt. Ordem garantida por `steam_steps_order_on_debian`.
- Ubuntu: já ligava `multiverse` + i386 (sem mudança). A licença da Steam continua interativa (não é pré-aceita).
- **Não testado aqui:** instalação real em VM Debian 12/13 e Ubuntu limpos (esta máquina é Ubuntu com a Steam já
  instalada). Fica como teste de aceite manual.

### Item 7 — matriz de GPU e detecção (2026-11-12)

- `docs/GPU.md`: matriz por fabricante/driver (recomendada, limitada, experimental, precisa de driver) e passo a passo
  da NVIDIA (driver, `nvidia-drm.modeset=1`, saída de emergência).
- `installer/src/gpu.rs`: detecta fabricante **e driver em uso** pelo /sys (nouveau × proprietário, versão e modeset
  da NVIDIA, VM, ARM), prefere a GPU dedicada em notebook híbrido e imprime avisos com o comando certo para Debian ou
  Ubuntu. Testes: `matrix`, `hybrid_laptop_prefers_the_dedicated_gpu`, `arm_board_by_driver`.
- **Mudança de comportamento:** NVIDIA deixou de bloquear a instalação (antes: "não validada"); agora é aviso. A
  sessão já usava `sway --unsupported-gpu` e cai para pixman se o sway falhar.
- **Validado só na VM** (detectou `virtual (vmwgfx)`). AMD, Intel e NVIDIA reais: a validar em hardware.

### Item 8 — segurança do acesso remoto (2026-11-12)

Modelo de ameaça e controles em `SECURITY.md`. Mudanças no Remote (`:8787`):

| Controle | Antes | Agora |
|---|---|---|
| Validade do token | nunca vencia | vence após 7 dias sem uso; uso renova; QR vencido é trocado ao abrir o pareamento |
| Rede de origem | qualquer IP | só loopback, privadas, link-local e CGNAT/Tailscale (403 para IP público) |
| `Origin` | ignorado | precisa ser a própria página (403 para outro site) |
| Conexões | sem limite antes do pareamento | 16 simultâneas (503), 20 s para parear, 240 msg/s, quadro ≤ 16 KB |
| Tentativas erradas | 8/min por IP | igual, e a tabela de IPs não cresce sem limite |

- **TLS:** avaliado e adiado (certificado autoassinado assusta no celular; válido exige domínio). Risco aceito e
  descrito no `SECURITY.md`.
- **Testes:** 3 unitários novos + e2e `remote_rejects_wrong_origin_expired_token_and_floods` (origem errada 403,
  mesma origem ok, 17ª conexão recusada, token vencido 403 com aviso e novo QR funcionando).
- **Verificado no Core real** da sessão (10.0.2.15:8787): página 200, hello 200, origem errada 403, mesma origem
  200, `/roms` 200, token errado 403. Instalações antigas não perdem o pareamento (prazo começa na atualização).

### Item 9 — lacunas do uso diário (auditoria, 2026-11-12)

| Área | Hoje | Lacuna | Prioridade |
|---|---|---|---|
| **Volume / saída de áudio** | O slider de volume (Configurações e menu rápido) só muda os **sons da interface** (`prefs.vol` → sfx). | Não controla o volume do sistema/jogo nem escolhe a saída (HDMI × fone × Bluetooth). O usuário espera que o volume do menu rápido mude o jogo. Proposta: `wpctl` (PipeWire, já instalado) no Core — `audio.status/set_volume/set_sink`, mudo, e trocar para HDMI ao conectar. | **Alta** |
| **Bluetooth** | Parear/remover controles (`bt.scan/pair/remove`, `controllers.rs`, tela de Controles). | Só controles: fones e caixas de som não aparecem (filtro `looks_like_pad`). Sem ligar/desligar o adaptador pela interface. | Média |
| **Suspender / retomar** | "Suspender" chama `systemctl suspend`. | Nada trata a volta: jogo aberto não é pausado antes, controles BT podem não reconectar, a Shell não sabe que o sistema dormiu (relógio/rede). Proposta: escutar `PrepareForSleep` do logind, congelar o jogo (`cgroup.freeze`) e reconectar ao voltar. Não testável na VM (sem S3). | Média |
| **Controle conectado/desconectado com jogo aberto** | Botão Guia: varredura de `/dev/input` a cada 3 s. Shell: eventos `gamepadconnected`. Controles registrados por jogador (`players_cfg`). | Sem aviso na tela quando o controle desliga no meio do jogo (bateria); demora até 3 s para o Guia funcionar num controle novo. Proposta: inotify em `/dev/input` + toast "Controle 1 desconectado". | Média |
| **HDMI-CEC** (controle remoto da TV) | Nada. | Ligar/desligar a TV junto com o Nexus e navegar com o controle da TV. Depende de hardware: placas de vídeo de PC quase nunca têm CEC (só Raspberry Pi e adaptadores Pulse-Eight). Proposta: `cec-client` opcional quando existir `/dev/cec*`. | Baixa |
| **Resolução / HDR / taxa** | Saída padrão do sway. | Sem tela para escolher resolução ou 60/120 Hz (TV 4K com PC fraco). | Média (prompt 2) |

Essas lacunas viram telas do prompt "Telas de sistema" (Som, Bluetooth, Energia, Tela), que é o próximo passo.

## Telas de sistema (prompt "o Nexus substitui o desktop") — auditoria (2026-11-12)

Legenda: ✅ existe · 🟡 parcial · ❌ falta. Serviços disponíveis na VM: NetworkManager (`nmcli`), BlueZ
(`bluetoothctl`), PipeWire (`wpctl`), logind, udisks2 (`udisksctl`), UPower, `timedatectl`, `localectl`.

| Item | Estado | O que existe | O que falta |
|---|---|---|---|
| A1 Barra de status | 🟡 | Abas, relógio, chip do jogo, busca, sino, engrenagem, avatar, tempo restante (criança) | Wi-Fi/cabo, Bluetooth, volume/mudo, bateria dos controles, alertas (sem internet, disco cheio) |
| A2 Central rápida | 🟡 | Guia/F1/Select+Start: Retomar, Início, Fechar, volume **do sistema** (desde o B3), tema, trocar perfil, reiniciar, desligar | Volume real do sistema, saída de áudio, Wi-Fi/BT liga-desliga, suspender, brilho |
| A3 Overlay no jogo | 🟡 | Menu rápido sobre o jogo; "Forçar encerramento"; kill do escopo após a carência | Volume e bateria do controle no overlay; aviso de notificação importante por cima do jogo |
| A4 Notificações | 🟡 | Toasts e central de notificações (updates, downloads, ROM recebida) | Fila curta com limite, "silenciar durante jogos", controle desconectado, disco cheio |
| A5 Bloqueio / desligar | 🟡 | Standby "Pressione A", escolha de perfil com PIN, desligar/reiniciar | Tela de energia única (suspender/reiniciar/desligar/trocar perfil) |
| A6 Boot e transições | ✅ (ver A6) | Tela "Carregando" da Shell, "Reconectando ao Core…", Shell recarrega se o renderer cai | Plymouth com a marca, tela de erro amigável (reiniciar sessão / recuperação) se Core ou Shell falham |
| A7 Primeiro uso | ✅ (ver A7) | Boas-vindas → criar perfil → PIN (opcional) → pronto | Idioma/fuso/teclado, rede, parear controle, pasta de ROMs, parear celular, Steam |
| B1 Rede | ✅ (ver B1) | — | Tudo (Wi-Fi, cabo, IP/DNS, oculta, esquecer, teste, IP fixo) |
| B2 Bluetooth | ✅ (ver B2) | Buscar/parear/remover **controles** (Controles → Bluetooth) | Ligar/desligar, fones/teclados, conectar, bateria, ajuda por modelo |
| B3 Áudio | ✅ (ver B3) | Só volume dos sons da UI | Saída, volume geral, teste, microfone, saída por perfil |
| B4 Tela e vídeo | ✅ (ver B4) | Mostra a resolução | Resolução, Hz, escala, margem, rotação, HDR/VRR, confirmação de 15 s |
| B5 HDMI-CEC | ❌ | — | Tudo (depende de hardware) |
| B6 Controles | 🟡 | Lista, apelido, jogador, mapeamento, exporta para RetroArch, vibração/inverter Y (prefs) | Teste de botões/analógicos, zona morta, ajuda para controles sem suporte |
| B7 Celular | ✅ | QR, aparelhos, desconectar, trocar código, endereço, explicação | (validade do código: feito no item 8) |
| B8 Idioma/hora | 🟡 (ver B8) | — | Idioma, teclado, fuso, formato, NTP |
| B9 Armazenamento | 🟡 | Espaço livre total; pendrives na tela de ROMs | Uso por categoria, ejetar, usar como biblioteca, limpar cache, formatar |
| B10 Energia | 🟡 | Desligar, reiniciar, suspender (RPC) | Inatividade para suspender/apagar tela, botão físico, comportamento ao ligar |
| B11 Atualizações | 🟡 | Atualizações de apps com reverter; `update.sh` | Versão do Nexus/sistema, verificar/instalar pela UI, histórico, reverter o Nexus |
| B12 Perfis/parental | ✅ | Limite de tempo, classificação, apps liberados, PIN | Recuperar PIN, relatório de uso |
| B13 Aparência | 🟡 | Tema, sons e volume, reduzir animações | Tamanho do texto, fundo |
| B14 Acessibilidade | 🟡 | Reduzir animações, foco alto contraste, legendas | Texto maior, leitor de tela |
| B15 Recuperação | ❌ | Rotas fora da UI (Ctrl+Alt+F3, force-desktop, revert-autologin) | Tudo dentro da UI |
| B16 Sobre/diagnóstico | 🟡 | SO, arquitetura, memória, GPU, nível de desempenho, versão | CPU, temperatura, uso atual, uptime, throttling, relatório de suporte |
| B17 Apps instalados | 🟡 | Adicionar app/atalho, imagens, remover personalizados | Tamanho, desinstalar jogos, ocultar, categoria |
| B18 ROMs | 🟡 | Pasta fixa `~/ROMs`, núcleos, capas automáticas, importar | Escolher pastas, reescanear, emulador padrão por console |
| B19 Privacidade | ❌ | (o Nexus não tem telemetria) | Tela explicando o que vai pela rede, limpar histórico/busca |

Ordem de implementação: A6, A7, B1–B4 → A1–A4, B10, B9 → B6, B7, B11, B15, B16 → resto.

### A6 — boot e transições (2026-11-12) ✅

- **Boot:** tema Plymouth `system/plymouth/nexus` (marca + três pontos; pede a senha do disco se houver). O instalador
  aplica no modo console (Debian/Ubuntu, quando o Plymouth existe) e regenera o initramfs. Imagens geradas por
  `tools/make-brand.py`. **Não testado num boot real** (a VM de desenvolvimento não reinicia nesta sessão).
- **Iniciando:** o fundo do sway é a marca (`/usr/share/nexus/splash.png`), visto enquanto a Shell sobe ou
  reinicia, em vez de tela vazia. Verificado na sessão aninhada.
- **Core fora do ar:** "Reconectando…" e, depois de 12 s, a tela **"O Nexus encontrou um problema"** com *Tentar de
  novo* (reinicia o Core), *Reiniciar o Nexus* (sessão) e *Reiniciar o aparelho* (com confirmação). B não fecha;
  some sozinha quando o Core volta. Quem executa é o processo do Electron, só com 3 ações fixas (`nexus:recover`),
  sem root.
- **Shell quebrando em série** (renderer caiu 3 vezes em 1 min ou a página não carrega): página reserva
  `electron/fallback.html`, sem dependências, navegável por controle (D-pad + A), teclado e mouse.
- **Bug corrigido no caminho:** `nexus-session.target` usava `Requires=` no Core; cada queda do Core reiniciava a
  sessão inteira (a Shell junto, 7 vezes em 15 s numa queda em série). Agora `Wants=` + `Restart=` de cada serviço.
- **Testes:** cenário de UI `falha_do_core` (Core morto → tela em 12 s; foco, direita, B não fecha, A executa; some
  quando o Core volta). Na sessão real: Core em queda contínua → tela aparece; ponte recusa ação desconhecida e
  reinicia o Core; renderer derrubado 3× → página reserva → "Tentar de novo" volta à interface.
- **Capturas:** `NEXUS_SHOTS=<pasta> node tests/e2e/run.mjs falha` gera `a6-falha-1080p.png` e `-720p.png`
  (legível nas duas).
- **Memória da Shell:** 153 MB no Início × 154 MB com a tela de falha aberta (PSS, `tools/measure.sh`). CPU com a
  tela aberta 18 % de um núcleo na VM (software), contra 11 % no Início: são as tentativas de reconexão; só
  acontece enquanto o Core está fora.

### A7 — primeiro uso (2026-11-12) ✅

- **Passos:** boas-vindas → idioma/fuso/teclado → rede → controle → perfil → PIN dos responsáveis → ROMs → celular →
  Steam (só no PC x86_64) → pronto. Rodapé com um botão principal (Continuar/Pular) e Voltar; B volta um passo.
  Depois de criar o perfil não dá para voltar para antes do PIN (o perfil já existe).
- Rede, Bluetooth e controles funcionam **antes de existir perfil** (o Core libera essas chamadas só enquanto não há
  nenhum perfil; depois exigem sessão, e as avançadas exigem o PIN).
- **Limitações:** idioma só pt-BR (a interface ainda não é traduzida); a pasta de ROMs é fixa em `~/ROMs` (o passo
  mostra como adicionar por pendrive/celular; escolher pastas é o B18).
- **Testes:** cenário `primeiro_uso` percorre tudo só com setas/Enter/Esc. Capturas `a7-boas-vindas`, `a7-idioma`,
  `a7-rede`, `a7-celular` (1080p e 720p; legíveis nas duas).

### B1 — rede (2026-11-12) ✅

- Core `sysnet.rs` sobre o NetworkManager (`nmcli`, saída fixa em `LC_ALL=C`, erros traduzidos): estado (cabo/Wi-Fi,
  sinal, IP, gateway, DNS, internet), lista de redes (sem duplicatas, conhecidas primeiro), conectar (senha pelo
  teclado na tela, com mostrar/ocultar, ou pelo celular), rede oculta, esquecer, Wi-Fi liga/desliga, teste de conexão
  (internet, DNS, latência) e IP fixo/automático — **este atrás do PIN dos responsáveis**.
- Permissão: regra polkit `system/polkit/51-nexus-system.rules` (só sessão local ativa; só ações de rede, fuso e
  teclado). A Shell não roda nada privilegiado.
- **Testes:** testes de unidade dos parsers/validadores; cenário de UI `rede` com um `nmcli` falso
  (`tests/e2e/fake/nmcli`): detalhes, teste de conexão, IP automático com PIN (confere o comando enviado), esquecer.
  Na VM real (NetworkManager de verdade): estado e teste corretos (cabo, 10.0.2.15/24, 30 ms).
- **Não testado:** conectar a Wi-Fi de verdade (a VM não tem placa Wi-Fi) e a regra polkit instalada (só vai no .deb).
- **Captura:** `b1-rede` 1080p/720p. **Memória da Shell:** 159 MB com Ajustes → Rede aberto (153 MB no Início).

### B8 — idioma, teclado, data e hora (2026-11-12) 🟡

- Ajustes → **Idioma e hora** (e o passo 2 do assistente): fuso (lista curada com nomes em português), hora
  automática (NTP via `timedatectl`), teclado físico (aplicado na hora no sway e lembrado pelo Core; `localectl`
  quando o polkit deixa).
- **Falta:** outros idiomas (sem tradução da interface) e formato de hora 12/24 h.

### B2 — Bluetooth (2026-11-12) ✅

- Ajustes → **Bluetooth** (nova categoria): liga/desliga, "Meus aparelhos" com tipo (controle, fone/caixa, teclado,
  mouse), conectado ou não e **bateria** (avisa quando ≤ 20 %), conectar/desconectar (sem PIN: é do dia a dia),
  esquecer (confirmação + PIN), procurar e parear qualquer aparelho, e **ajuda por modelo** (Xbox, DualSense,
  DualShock 4, Switch, 8BitDo, outros) trocada com ←/→. Ajustes → Controles e o assistente usam o mesmo painel,
  filtrado para controles.
- Core: novo `bluetooth.rs` (substitui o código em `controllers.rs`): tipo pelo ícone do BlueZ (e pelo nome),
  bateria do `info`, erros em português (bloqueado/rfkill, sem adaptador, recusa de pareamento). Sem adaptador não chama o
  `bluetoothctl` (ele trava esperando). Estados: verificando, indisponível + "Tentar de novo", desligado, vazio.
- **Bugs corrigidos no caminho:**
  - Voltar (B) em diálogos de duas opções contava como "não": no IP fixo, apertar B aplicava "Automático".
    Agora B devolve "nada escolhido" (`askChoice`).
  - Quando o item focado sumia (ex.: aparelho pareado sai da lista "encontrados"), o foco ia para o primeiro item da
    tela, a categoria Rede, e trocava de tela. Agora vai para o vizinho mais próximo (`packages/focus`).
- **Testes:** unidade (ícone → tipo, bateria) e cenário de UI `bluetooth` com um `bluetoothctl` falso: tipo/bateria,
  procurar (pareado não repete, controle primeiro), parear, recusa explicada, Voltar sem efeito, desconectar,
  ajuda e desligar. O cenário `rede` passou a conferir que B não muda o IP.
- **Não testado com hardware:** a VM não tem adaptador Bluetooth. A Shell real mostra corretamente o estado
  "Bluetooth indisponível".
- **Captura:** `b2-bluetooth` 1080p/720p. **Memória da Shell:** 162 MB com Ajustes → Bluetooth aberto e 170 MB no
  Início logo depois. A variação vem do coletor de lixo (±10 MB); a tela não pesa de forma mensurável.

### B3 — áudio (2026-11-12) ✅

- Ajustes → **Áudio** (nova categoria; perfis infantis também veem, porque volume é do dia a dia): volume geral com
  slider (←/→; mexer tira o mudo), mudo, **testar o som** (dois bipes na saída em uso), lista de **saídas** com
  tipo (TV/HDMI, analógica, Bluetooth, USB) e microfone com volume. A saída escolhida é **lembrada por perfil**
  (pelo `node.name`, que não muda entre boots) e reaplicada ao entrar no perfil.
- **Central rápida (A2):** o slider de volume agora é o volume **do sistema** (jogos e apps), e A liga/desliga o
  mudo. Era a lacuna "o volume só muda os sons da interface" do item 9. O volume dos sons da interface continua em
  Ajustes → Tela e aparência (antes "Tela e som").
- Core: `audio.rs` sobre `wpctl` (lê só o bloco Audio do `wpctl status`; tipo pelo `wpctl inspect`), `pw-play` com
  um WAV gerado no próprio Core. As mudanças são serializadas (volume e mudo seguidos não se cruzam) e emitem
  `audio.changed`.
- **Bugs corrigidos no caminho:**
  - Apertar A até 120 ms depois de mexer no volume ligava o mudo, e o volume atrasado o desligava em seguida. Agora
    vão juntos, e o Core aplica uma mudança por vez.
  - Clicar ou tocar no trilho de **qualquer** slider mudava o volume dos sons da interface. Agora cada slider tem
    seu dono.
- **Testes:** unidade (parser do `wpctl status`, tipo da saída, WAV válido, saída lembrada por perfil com o
  `wpctl` falso) e cenário de UI `audio`. O cenário percorre volume, saída HDMI, teste, microfone e a Central
  (volume do sistema e mudo, sem corrida). **Na VM real** (PipeWire de verdade): estado correto (40 %, mudo,
  saída analógica, microfone) e o teste de som tocou.
- **Captura:** `b3-audio` 1080p/720p. **Memória da Shell:** 186–191 MB com Ajustes → Áudio aberto e 188 MB no
  Início, na mesma sessão (a sessão está mais pesada que nas medições anteriores; a tela não acrescenta nada
  mensurável).

### Página de envio de ROMs (celular e PC) (2026-11-12)

- `roms_upload.html` (aberta pelo QR/link) agora é responsiva:
  - **Celular:** uma coluna (console → arquivos → dicas).
  - **PC (≥ 860 px ou mouse):** duas colunas (console e dicas fixos à esquerda, arquivos à direita), **arrastar e
    soltar** arquivos, textos "Arraste… ou clique", estados de hover e lista com rolagem.
  - **Os dois:** dá para escolher arquivos em várias levas, tirar um da lista antes de enviar, ver o total
    (quantidade e tamanho) e o progresso geral ("Enviando 2 de 5…"). Sem dependências externas.
- Conferido renderizando em 1440×900 e 390×844 (sem rolagem horizontal). O teste `roms_upload_from_phone` continua
  passando.

### B4 — tela e vídeo (2026-11-12) ✅

- Ajustes → **Tela e vídeo** (nova categoria; a antiga "Tela e som" virou **Aparência**):
  - tela atual (nome, resolução, Hz, escala);
  - **resolução** e **taxa de atualização** (só os modos que a tela informa, maior primeiro), **escala** (100–200 %)
    e **rotação**, que se escolhem com ←/→ e se aplicam juntas em "Aplicar mudanças";
  - **VRR/FreeSync** liga/desliga;
  - **HDR** quando o compositor e a tela informam. Senão aparece "Não disponível", com o motivo.
- **Confirmação de 15 s:** é o Core que aplica e volta sozinho, não a Shell, então funciona mesmo com tela preta.
  A Shell mostra "Manter esta configuração? Volta sozinho em N s". Sem resposta, ou com "Voltar à anterior",
  a configuração anterior volta. Só o que foi confirmado é salvo (por tela: marca/modelo/série) e reaplicado
  quando o Core inicia.
- **Margem (overscan):** slider de 0 a 10 % e **assistente** em tela cheia com setas nos quatro cantos (←/→ ao vivo,
  A salva, B desiste). A interface inteira encolhe por igual. O valor é do aparelho e vale desde a tela de
  perfis. **Limite:** só a interface do Nexus respeita a margem. Jogos e apps usam a tela inteira; para eles, o
  certo é desligar o overscan na TV (modo "Ajustar à tela", "Just Scan" ou "Pixel a pixel").
- Core: `display.rs` (sway `get_outputs`/`output …`, valores validados, token pequeno para não perder precisão no JSON
  da Shell).
- **Testes:** unidade (parser das saídas, validação, junção do salvo) e cenário de UI `tela` com um `swaymsg` falso
  que guarda estado e recusa modos desconhecidos. O cenário percorre: resolução → contagem → manter (não volta
  depois do prazo); escala sem resposta → volta sozinha; VRR; HDR indisponível; margem (encolhe 6 % ao vivo,
  salva no Core, B desiste). **Na sessão real (sway 1.11):** escala 125 % aplicada e, sem resposta, de volta a
  100 % em 15 s. A saída aninhada da VM não informa modos, e a tela mostra "Esta tela não informa os modos
  disponíveis".
- **Não testado:** TV de verdade com HDMI (modos, VRR) e HDR. O `swaymsg output … hdr` só aparece se o sway e a
  tela suportarem.
- **Captura:** `b4-tela`, `b4-confirmar`, `b4-margem` (1080p/720p). **Memória da Shell:** 185 MB com Tela e vídeo
  aberto (188 MB no Início na mesma sessão).

### Navegação por controle nos Ajustes e na Biblioteca (2026-11-12) ✅

Pedido do usuário: "quando chegar no final da página, deve travar; só voltar para o menu lateral com ←".
- **Zonas no motor de foco** (`packages/focus`, `data-zone` num contêiner):
  - ↑/↓ não saem da zona e **travam no fim** (com o som de "bump");
  - só ←/→ trocam de coluna;
  - voltando à lista lateral, o foco cai na **categoria atual** (não num item qualquer, que trocaria a tela);
  - voltando ao painel, cai no **último item usado** (ou no primeiro visível);
  - `data-zone-exit="u"` deixa ↑ sair no topo (Biblioteca → abas).
- Aplicado em Ajustes (lista + painel) e Biblioteca (filtros + grade).
- **Bugs corrigidos no caminho:**
  - um `class={…}` reativo (item que vira `.on`) apagava a marca `.focus`: o item continuava focado sem realce;
    agora o motor repõe a marca;
  - andar pela lista lateral a partir de "Voltar ao Início" pulava para a categoria ativa.
- **Teste:** cenário `navegacao` (fim e topo do painel, ← para a categoria certa, → para o mesmo item, pontas da
  lista lateral, ↓ item a item).

### B6 — controles (2026-11-12) ✅ (parcial: sem perfil por emulador)

- Ajustes → **Controles** ganhou:
  - **Testar botões e analógicos:** tela cheia que mostra ao vivo A/B/X/Y, LB/RB, LT/RT (com %), Select/Start/Guia, L3/R3,
    direcional e os dois analógicos (com a zona morta desenhada). Lê a Gamepad API do próprio navegador, então não passa pelo
    Core. Durante o teste o controle **não navega** nos menus: segure **B por 1 s** para sair; **Select** faz vibrar. A
    navegação só volta quando todos os botões forem soltos (senão o botão segurado abriria algo nos Ajustes).
  - **Vibração de teste** (`vibrationActuator`; avisa, sem erro, se o controle/conexão não vibra).
  - **Zona morta** dos analógicos (0–40 %, de 2 em 2, padrão 10 %): guardada neste aparelho (`localStorage`) e aplicada à
    navegação por controle da Shell (`input.ts`).
  - **Ajuda para controle sem suporte** (cinco situações comuns, em português simples).
- Já existia e foi mantido: lista, apelido, ordem dos jogadores, mapeamento e exportação para o RetroArch.
- **Teste:** cenário `controles` (controle falso injetado em `navigator.getGamepads`): zona morta, vibração, botões,
  gatilho, analógicos, zona morta, "sem navegar durante o teste", sair com B. Capturas `b6-controles-teste` e
  `b6-controles` (1080p/720p).
- **Não feito:** **perfil de controle por emulador** (exigiria um arquivo de configuração do RetroArch por console, ainda não
  existe na pasta de dados do Nexus) e **zona morta nos emuladores** (hoje o RetroArch usa a dele; a do Nexus vale só para a
  interface). `tests/e2e/lib.mjs` passou a aceitar `NEXUS_CORE_BIN` (binário do Core a testar).

### B7 — celular (revisão) (2026-11-12) ✅

- Ajustes → Controle no celular: nova seção **"O que o celular pode fazer"** (ser o controle, mouse/teclado, enviar jogos
  pela página de ROMs) e **"Validade do código"** (vence com 7 dias sem uso; usar renova; "Gerar novo código" desconecta
  todos). Nada mudou no Core (a validade já existia, item 8).
- **Testes:** cenários `celular` e `adulto_demo` passam (o primeiro precisa de `apps/remote/dist` construído).
