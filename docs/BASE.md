# Base recomendada para um aparelho Nexus

**Oficial: Debian estável (13 "trixie" ou 12 "bookworm") instalado pelo netinst, sem ambiente de desktop.**
Ubuntu LTS (24.04/26.04) funciona e é testado no desenvolvimento, mas traz GNOME, snapd e serviços de desktop que
consomem RAM e CPU sem uso num console: suportado "sem garantia de desempenho".

## Instalar a base (PC)
1. Baixe o `debian-<versão>-amd64-netinst.iso` em debian.org e grave num pendrive.
2. Na instalação, em **Seleção de software**, desmarque *Ambiente de área de trabalho* e *GNOME*; deixe marcados
   só **servidor SSH** (opcional, para suporte) e **utilitários de sistema padrão**.
3. Crie o usuário que vai jogar (ex.: `sala`). Depois de reiniciar, entre no terminal com ele.
4. Instale o Nexus (com sudo configurado, ou como root):
   ```sh
   apt install -y curl sudo && curl -fsSL https://raw.githubusercontent.com/GuiVicS/nexus/main/tools/update.sh | sh -s -- --autologin sala
   ```
   Sem gerenciador de login, o instalador põe o **greetd** (login direto na sessão Nexus, sem tela própria).

## Raspberry Pi 4/5
Raspberry Pi OS **Lite** 64 bits (Debian sem desktop) gravado pelo Raspberry Pi Imager; mesmo comando acima.

## O que o instalador ajusta (`--sem-otimizar` deixa como está)
- Desliga serviços que um console não usa: impressão (cups, só se não houver impressora configurada), modem 3G/4G
  (ModemManager, só sem modem), localização (geoclue), perfil de cor (colord), descoberta local (avahi), relatórios
  de falha (whoopsie, kerneloops, apport) e os indexadores de arquivos da sessão (localsearch/tracker). **Nada é
  removido**: só `systemctl disable`/`mask`, anotado em `/var/lib/nexus/slim/`.
- Liga **zram** (swap comprimida na RAM, metade da memória até 4 GB, zstd) com `vm.swappiness=150`: a memória
  da Shell congelada durante o jogo vai comprimida, liberando RAM para o jogo.
- Atualizações automáticas do apt às 4h/5h e **nunca com um jogo aberto** (`ExecCondition` → `nexus-slim idle`).
- Desfazer tudo: `sudo /usr/lib/nexus/nexus-slim revert`. Ver o que foi feito: `nexus-slim status`.

Saídas de emergência continuam iguais: Ctrl+Alt+F3, `sudo touch /etc/nexus/force-desktop` (numa base sem desktop abre o
terminal foot, se instalado; senão use o Ctrl+Alt+F3), `sudo nexus-installer revert-autologin`.
