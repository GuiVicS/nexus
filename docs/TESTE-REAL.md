# Teste no aparelho real

Até a 0.5.0, tudo foi testado só numa máquina virtual, com programas falsos no lugar do Google Drive, do CEC, do pendrive e
de outras partes do sistema. Este roteiro é para o **primeiro teste num aparelho de verdade**. Anote o que falhar (foto da
tela + o item abaixo) e rode o [diagnóstico](#se-algo-der-errado) no fim.

## Instalar no Raspberry Pi (Raspberry Pi OS Lite, sem interface)

Precisa de **Raspberry Pi 4 ou 5** (de preferência 4 GB de RAM ou mais) e sistema **64 bits**.
O pacote do Pi **nunca rodou num Raspberry de verdade** (só no QEMU): é o ponto mais experimental.

1. **Gravar o cartão** com o Raspberry Pi Imager: *Raspberry Pi OS (other) → Raspberry Pi OS Lite (64-bit)*.
   Na engrenagem de personalização: usuário e senha, Wi-Fi, fuso horário e **ativar o SSH**.
2. **Tela de login leve.** O Lite não tem tela de login, e o Nexus precisa de uma para entrar sozinho ao ligar.
   Instale o greetd antes do Nexus:
   ```sh
   sudo apt update && sudo apt full-upgrade -y
   sudo apt install -y greetd
   sudo systemctl enable greetd
   ```
3. **Instalar o Nexus** (emuladores inclusos; a Steam é pulada, porque não existe para ARM):
   ```sh
   curl -fsSL https://raw.githubusercontent.com/GuiVicS/nexus/main/tools/update.sh | sh -s -- --autologin SEU_USUARIO
   ```
4. `sudo reboot`. O Pi deve ligar direto no assistente de primeiro uso do Nexus.

No **PC** (Debian/Ubuntu com interface), use só o passo 3. O instalador configura o login que já existe (GDM, LightDM ou SDDM).

## Roteiro de testes

Marque ✅ ou ❌ e anote o que viu.

### 1. Instalação e inicialização
- [ ] A instalação termina sem erro e instala sozinha `rclone`, `fuse3`, `mpv`, `swayidle` e `udisks2`.
- [ ] Ao reiniciar, entra direto no Nexus, sem ver o terminal ou o desktop.
- [ ] O assistente de primeiro uso aparece: idioma, rede, controle, perfil, PIN, celular, Google Drive.
- [ ] `Ctrl+Alt+F3` abre um terminal de emergência.
- [ ] Imagem na resolução da TV, sem bordas cortadas (senão: Ajustes → Tela e vídeo → margem).
- [ ] **Pi:** a interface responde sem travar. Anote a memória em Ajustes → Sobre e diagnóstico.

### 2. Controle e navegação
- [ ] Controle USB ou Bluetooth navega por tudo (sem mouse).
- [ ] Menu rápido: botão Guia (PS/Xbox), ou **Select+Start**, ou **F1** no teclado. Abre e fecha com a mesma tecla.
- [ ] Ajustes → Controles → testar botões: cada botão acende certo.
- [ ] Celular como controle (Ajustes → Controle no celular → QR) e "instalar como app" no celular.

### 3. Google Drive e Mídia
- [ ] Ajustes → Armazenamento → Google Drive → Entrar: login com a conta Google real termina e a janela fecha sozinha.
- [ ] A pasta **Nexus** aparece no seu Drive (pelo site), com as subpastas ROMs, Filmes, Séries, Músicas e Saves.
- [ ] Uma ROM em `Nexus/ROMs/<console>` aparece nos Emuladores com a marca da nuvem e abre (barra "Baixando do Google Drive").
- [ ] A mesma ROM na TV e no Drive aparece **uma vez só**.
- [ ] Aba Mídia: um filme em `Nexus/Filmes` toca em tela cheia; voltar ao Nexus funciona.
- [ ] Depois de reiniciar o aparelho, o Drive conecta sozinho.

### 4. Jogos
- [ ] Emuladores: escolher console → jogo abre sem a tela "RETROARCH", e sair volta ao Nexus.
- [ ] Saída de emergência dentro do jogo: segurar o botão por 3 s.
- [ ] **PC:** jogo da Steam abre e fecha sem a janela da Steam aparecer.
- [ ] Apps: adicionar um `.jar` (ex.: TLauncher) por arquivo. Com Java instalado, abre pela Biblioteca. Sem Java, a tela pede `sudo apt install default-jre`.

### 5. Sistema
- [ ] Barra de status: Wi-Fi, Bluetooth, bateria do controle e volume batem com o real.
- [ ] Central rápida: volume, saída de som, brilho (só em tela com brilho ajustável), Wi-Fi e Bluetooth liga/desliga.
- [ ] Energia: a tela apaga após o tempo escolhido. Suspender e acordar volta ao Nexus. O botão de ligar faz o que foi escolhido.
- [ ] Pendrive: aparece em Armazenamento; copiar ROMs; ejetar; formatar (use um pendrive sem nada importante).
- [ ] **TV (HDMI-CEC)**, se a TV tiver: Ajustes → TV mostra o adaptador; ligar/desligar a TV junto com o aparelho.
  O Pi tem CEC na HDMI; no PC precisa de adaptador. Antes: `sudo apt install cec-utils`.
- [ ] Atualizações: Ajustes → Atualizações → procurar (deve dizer que a 0.5.0 é a mais recente).
- [ ] **SSH:** Ajustes → Acesso por SSH → ligar e seguir os passos na tela, a partir de outro computador.
- [ ] Perfil infantil: com PIN dos responsáveis, a criança não vê Ajustes de adulto. "Esqueci o PIN" com o código de recuperação.

## Se algo der errado

Mande a foto da tela e a saída destes comandos (pelo SSH ou pelo terminal de emergência `Ctrl+Alt+F3`):
```sh
dpkg -l nexus | tail -1
nexus-installer doctor
cat ~/.local/state/nexus/session.log
journalctl --user -b -u nexus-shell -u nexus-core --no-pager | tail -60
journalctl -b -u greetd --no-pager | tail -40     # Pi com greetd
```
- **Voltar ao terminal normal ao ligar:** `sudo nexus-installer revert-autologin`.
- **Voltar ao desktop normal (PC):** `sudo touch /etc/nexus/force-desktop`.
- A sessão desfaz o autologin sozinha depois de 3 falhas em 5 minutos.
