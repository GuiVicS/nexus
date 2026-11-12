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
