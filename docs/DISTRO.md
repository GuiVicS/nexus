# Distro Nexus e imagem pronta do Raspberry Pi: análise e decisões (rascunho)

Conversa de 2026-11-12. O usuário quer:
1. uma **imagem `.img` pronta do Raspberry Pi**, com o Nexus já instalado;
2. uma **distro própria, universal para PC**.

Nada disto está implementado. Ficou só a análise e a decisão de base. Ordem combinada:
**1) o usuário testa a 0.5.0 num PC (VM Debian 13) e no Pi (`docs/TESTE-REAL.md`) → 2) imagem do Pi → 3) ISO para PC.**

## 1. Imagem pronta do Raspberry Pi (`.img`)

**Não funciona: instalar numa VM QEMU (`-M virt`) e copiar o disco.** Essa VM é um "PC ARM genérico": liga por UEFI,
com kernel genérico do Debian e discos virtio. O Pi precisa do próprio firmware na partição de boot FAT (`start4.elf`,
`config.txt`), do kernel do Pi e do device tree. O disco feito na VM não liga no Pi.

**Caminho certo (proposto, falta o usuário confirmar):** script `tools/build-pi-image.sh`, rodado com `sudo` num Linux x86. Etapas:
1. Baixa o **Raspberry Pi OS Lite 64 bits** oficial.
2. Monta a imagem (loop) e entra nela com `chroot`, usando `qemu-user-static`/binfmt (emulação só de programas, rápida).
3. Instala as dependências, o greetd e o `.deb` do Nexus.
4. Cria um **serviço de primeiro boot**, porque o que depende do aparelho ligado não pode ser feito no chroot.
   Ele roda `nexus-installer install --autologin <usuário criado no Raspberry Pi Imager>` e detecta a GPU.
5. Fecha e comprime em `nexus-<versão>-pi.img.xz`, que pode entrar nas releases.

Alternativas: **sdm** (ferramenta pronta de personalizar imagens do Pi) ou **pi-gen** (gera o Pi OS do zero; bem mais trabalho).

Limites:
- A montagem precisa de root e de cerca de 4 a 5 GB livres.
- O QEMU `-M raspi4b` liga a imagem, mas sem rede e sem USB: serve só para ver se o sistema sobe.

## 2. Distro universal para PC: base **Debian 13 (Trixie)** (decisão do usuário)

Por que o Debian 13:
- O Nexus é `.deb` e o instalador já conhece a família Debian.
- O `docs/BASE.md` já tem o Debian 13 sem desktop como base oficial.
- O Raspberry Pi OS também é Debian, então PC e Pi ficam com a mesma base.
- Kernel 6.12 LTS, com kernel mais novo disponível no backports.
- A ISO oficial já traz o firmware não livre.
- Tem Secure Boot (shim assinado).

O Ubuntu 24.04 também foi considerado, porque detecta e instala o driver NVIDIA com mais facilidade e tem kernel HWE.
Ficou de fora por trazer snap e GNOME.

O que decide se "funciona em qualquer PC":
- **Kernel recente:** opção de kernel do backports no menu de boot, para hardware muito novo.
- **Firmware:** `firmware-linux-nonfree`, `firmware-misc-nonfree`, `firmware-amd-graphics`, `firmware-iwlwifi`,
  `firmware-realtek`, `firmware-atheros`, `firmware-brcm80211`, `firmware-sof-signed` (áudio de notebooks novos),
  `intel-microcode`, `amd64-microcode`.
- **NVIDIA:** é o ponto mais difícil. Com Secure Boot, o módulo DKMS precisa de **registro de chave MOK**
  (senha mostrada na TV, confirmada no próximo boot). Placas muito antigas ficam no nouveau, e o `docs/GPU.md` diz o que esperar.
- **Secure Boot:** shim e GRUB assinados do Debian.
- Só **x86_64**. O Pi continua com imagem própria (seção 1).

Esboço (a detalhar quando o usuário pedir o plano completo):
1. **ISO live híbrida** (BIOS + UEFI + Secure Boot) gerada por `live-build`, com o Nexus e as dependências dentro.
   Dá para ligar direto no Nexus e experimentar sem instalar.
2. **Instalador gráfico** (Calamares numa sessão sway): disco, usuário e Wi-Fi.
   Depois da instalação, um passo chama `nexus-installer install --autologin <usuário>`.
3. **Primeiro boot:** detecta a GPU (`installer/src/gpu.rs`), instala o driver NVIDIA se precisar (MOK com Secure Boot)
   e confere Wi-Fi, áudio e Bluetooth.
4. **Atualizações:** as de segurança do Debian já são automáticas e nunca rodam com jogo aberto (`nexus-slim`).
   As do Nexus exigem um **repositório apt próprio com chave GPG**, decisão pendente (`docs/APT.md`), que vira obrigatória aqui.
5. **Visual:** tema do Plymouth e do GRUB, splash do Nexus, boot rápido.
   A distro não pode se chamar "Debian" (marca registrada); pode ser "Nexus, baseado em Debian".
6. **Validação** em vários PCs: Intel, AMD, NVIDIA, desktop e notebook.
7. Depois, se fizer sentido: sistema imutável com atualização A/B e volta automática (como o SteamOS e o Bazzite).

Infraestrutura e limites:
- A ISO precisa de root e de uns 15 GB de disco. Gerar pelo **GitHub Actions** (contêiner `debian:trixie` privilegiado).
- Os runners Linux do GitHub têm KVM, então dá para testar o boot em QEMU automaticamente.
- **Cada arquivo de release do GitHub tem limite de 2 GiB**: manter a ISO abaixo disso, ou dividir.
- Steam e RetroArch (Flatpak, uns 600 MB ou mais com runtime e núcleos): dentro da ISO, que fica maior e funciona offline,
  ou baixados no primeiro boot. **Decisão pendente.**

Decisões pendentes para a distro:
- nome;
- Calamares ou instalador próprio na Shell;
- Steam e RetroArch dentro da ISO ou no primeiro boot;
- chave GPG do repositório apt;
- kernel padrão: stable ou backports.
