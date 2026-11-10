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
