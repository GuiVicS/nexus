<!-- Pedido original do usuário (2026-11-12). O andamento de cada item fica em HANDOFF.md §4b e docs/STATUS.md. -->

# Prompt: Telas de sistema do Nexus (a UI que substitui o desktop)

## Contexto

Leia HANDOFF.md, docs/architecture/ARCHITECTURE.md e docs/STATUS.md.
O Nexus é a UI completa do sistema: depois de instalado, o usuário não verá
desktop, gerenciador de arquivos nem terminal. Tudo o que ele precisaria fazer
no sistema (rede, áudio, tela, energia, atualização) tem que ser possível
dentro do Nexus, só com o controle (ou o celular).

## Princípios

1. Primeiro, auditar: para cada item abaixo, verificar se já existe, está
   parcial ou falta. Registrar o resultado em docs/STATUS.md antes de
   implementar qualquer coisa.
2. Tudo navegável só com controle (D-pad, confirmar, voltar), sem depender de
   mouse. Qualquer campo de texto abre teclado na tela e também aceita o
   teclado do celular (Remote).
3. Legível a 3 metros de uma TV: texto grande, contraste alto, alvos de foco
   grandes, foco sempre visível.
4. As configurações ficam escondidas em uma área "Sistema" (ícone de engrenagem
   discreto ou botão guia), fora do caminho de quem só quer jogar.
5. Itens perigosos (formatar disco, reset, modo técnico, desativar autologin)
   exigem a senha dos responsáveis e confirmação explícita.
6. Leve: nada de animações pesadas, blur caro ou vídeo em loop. O objetivo do
   projeto é entregar o máximo de desempenho aos jogos. Telas de sistema só
   consomem recursos enquanto estão abertas.
7. Toda ação de sistema passa pelo Core (Rust), que fala com os serviços do
   Linux (NetworkManager, BlueZ, PipeWire/wpctl, systemd-logind, udisks2,
   UPower, apt). A Shell nunca roda comandos privilegiados. Permissões via
   polkit, no mínimo necessário.
8. Cada tela precisa ter estados de carregando, vazio e erro, com mensagem em
   português simples e uma ação possível ("Tentar de novo", "Voltar").

## A. Elementos sempre visíveis (fora das configurações)

A1. Barra de status discreta (some sozinha durante jogos e vídeos):
hora, Wi-Fi/cabo (com sinal), Bluetooth, volume/mudo, bateria dos controles
conectados, perfil ativo. Indicadores de alerta só quando houver problema
(sem internet, controle com bateria baixa, disco quase cheio).

A2. Central de controle rápido (abre com o botão guia/home, também por cima de
um jogo): volume, saída de áudio, Wi-Fi e Bluetooth liga/desliga, brilho
quando houver, suspender, reiniciar, desligar, "Voltar ao Nexus" e "Encerrar
jogo". Tem que abrir e fechar rápido, sem pesar no jogo.

A3. Overlay durante o jogo: volume, bateria do controle, aviso de notificação
importante, e saída de emergência para jogo travado (combinação de botões
que encerra o processo e volta à Shell, com confirmação).

A4. Notificações (toasts): atualização disponível, download concluído, controle
desconectado, disco cheio, erro de jogo. Fila curta, sem acumular, e com
opção de silenciar durante jogos.

A5. Tela de bloqueio/troca de perfil e tela de desligar (suspender, reiniciar,
desligar, trocar perfil).

A6. Boot e transições: tela de boot personalizada (Plymouth) com a marca, tela
de "iniciando" enquanto a Shell sobe, e tela de erro amigável se o Core ou a
Shell falharem, com opção de reiniciar a sessão ou abrir o modo de
recuperação (ver B15).

A7. Primeiro uso (assistente): idioma, fuso e teclado, rede, parear controle,
criar perfil e senha dos responsáveis, escolher pasta de ROMs, parear
celular, e no PC conectar a Steam. Deve ser possível concluir só com o controle.

## B. Área "Sistema" (configurações, em lista lateral com seções)

B1. Rede: lista e conexão a redes Wi-Fi (senha pelo teclado na tela ou
celular), cabo, IP e DNS atuais, rede oculta, esquecer rede, teste de
conexão, e modo avançado (IP fixo) atrás da senha dos responsáveis.

B2. Bluetooth: ligar/desligar, buscar, parear, conectar, esquecer; mostrar tipo
(controle, fone, teclado) e bateria. Ajudar com o caso comum: controle que
não pareia (instruções por modelo).

B3. Áudio: saída (HDMI, analógica, Bluetooth), volume geral, teste de som,
entrada de microfone quando houver, e saída padrão lembrada por perfil.

B4. Tela e vídeo: resolução, taxa de atualização, escala da interface,
overscan/margem para TVs que cortam bordas, rotação, HDR e VRR quando o
hardware e o compositor suportarem, e um assistente de ajuste de margem.
Toda mudança de resolução tem contagem regressiva de 15 s para confirmar,
e volta sozinha se o usuário não confirmar (evita tela preta).

B5. HDMI-CEC (quando suportado): ligar a TV junto com o Nexus, desligar a TV ao
desligar, e usar o controle remoto da TV para navegar.

B6. Controles: lista de controles conectados, teste de botões e analógicos,
mapeamento, zona morta, vibração, ordem dos jogadores, e perfil de controle
por emulador. Ajuda com controles sem suporte.

B7. Celular como controle (Remote): parear, ver aparelhos pareados, revogar
acesso, trocar o código, e ver o endereço atual. Explicar com texto curto o
que o celular pode fazer (controle, mouse, teclado).

B8. Idioma, teclado, data e hora: idioma da interface, layout do teclado
(br/us/outros), fuso e formato de hora, sincronização automática.

B9. Armazenamento: espaço usado por categoria (jogos, ROMs, capas, sistema),
discos externos USB (listar, montar, ejetar com segurança, usar como pasta de
ROMs ou biblioteca), limpar cache e temporários, e formatar disco (só com
senha e confirmação dupla). Alerta de pouco espaço.

B10. Energia: suspender, reiniciar, desligar; tempo de inatividade para
suspender e para desligar a tela; o que o botão físico de energia faz;
comportamento ao ligar (abrir direto no Nexus).

B11. Atualizações: versão atual do Nexus e do sistema, verificar atualização,
instalar (Nexus e pacotes do sistema), histórico, e reverter para a versão
anterior se a nova der problema. Atualizar só quando o usuário aceitar e
nunca durante um jogo. Indicar claramente se vai reiniciar.

B12. Perfis e controle parental: já existente. Revisar se cobre: limite de
tempo, classificação por idade, apps e jogos bloqueados, senha dos
responsáveis (trocar, recuperar), e relatório de uso.

B13. Aparência: tema claro/escuro/contraste alto, tamanho do texto, fundo,
sons da interface e volume deles, e redução de animações.

B14. Acessibilidade: texto maior, contraste alto, leitor de tela (se viável),
legendas, alternativas para quem não usa um dos controles, e navegação
simplificada.

B15. Recuperação e modo técnico (atrás da senha dos responsáveis):

- Reiniciar só a sessão do Nexus.
- Modo de recuperação: tela mínima que oferece reparar a Shell, reverter
  atualização, voltar ao desktop (force-desktop) e abrir diagnóstico.
- Ligar/desligar SSH, e mostrar o IP e como conectar.
- Abrir terminal (opcional, avisando que é para usuários avançados).
- Restaurar padrões das configurações, e apagar os dados do perfil.
- Reset de fábrica do Nexus, com confirmação dupla e explicação do que some.

B16. Sobre e diagnóstico: modelo do aparelho, versão do sistema e do Nexus,
CPU, memória, GPU, temperatura, uso atual de CPU/RAM/disco, tempo ligado,
e se há throttling (importante no Raspberry Pi). Botão "Gerar relatório de
suporte" que junta logs e informações em um arquivo e mostra onde está (ou
entrega pelo celular). Mostrar também o que está consumindo recursos.

B17. Gerenciador de apps e jogos instalados: lista com tamanho, desinstalar,
ocultar, mudar categoria, e adicionar app ou atalho manualmente (um
.desktop, um site de streaming, um emulador).

B18. ROMs e biblioteca: indicar as pastas de ROMs, reescanear biblioteca,
baixar capas e metadados, e escolher o emulador padrão por console.

B19. Privacidade e rede: o que o Nexus envia pela rede (se algo), ativar ou
desativar telemetria (padrão: desligada), limpar histórico e dados de busca.

## Entregáveis e ordem

Ordem de implementação sugerida (do que mais trava o uso para o que menos):

1. A6, A7, B1, B2, B3, B4 (sem isso o aparelho nem fica usável sem teclado)
2. A1, A2, A3, A4, B10, B9
3. B6, B7, B11, B15, B16
4. Os demais.

Para cada tela:

- Código e testes (e2e com navegação só por controle).
- Atualização de docs/STATUS.md: o que existia, o que foi feito, o que falta.
- Captura de tela da tela final em 1080p e em 720p, para conferir legibilidade.
- Medição de memória da Shell com a tela aberta, para garantir que não pesou.

Não implemente tudo de uma vez: faça uma seção, rode os testes, registre no
STATUS.md e siga para a próxima.
