# 0004 — gamescope: por jogo, como sessão, ou não usar (PROPOSTA, aguardando decisão)

Contexto: o gamescope (compositor do SteamOS) dá escala de resolução (FSR), limite de FPS, VRR/HDR e isola o jogo.
Hoje o Nexus usa o sway como sessão e mostra o jogo em tela cheia na área 2. Nesta VM não há GPU (só Vulkan por
software), então nada aqui foi medido com jogo real.

| Opção | Prós | Contras |
|---|---|---|
| A. Manter só o sway (hoje) | simples, funciona com Electron Wayland, Steam, Flatpak e navegadores; já testado | sem FSR/limite de FPS/VRR do sistema (cada jogo resolve o seu) |
| B. gamescope **por jogo**, opcional (`gamescope -f -W … -r 60 -- <jogo>`; jogos Steam via opção de lançamento) | ganha FSR, limite de FPS e VRR só onde o usuário liga; o resto segue igual | precisa de Vulkan 1.3 (fica de fora o Pi 4 e GPUs antigas); NVIDIA proprietária com histórico de problemas; mais uma camada (Xwayland própria) no caminho do foco e do overlay da Steam; jogos Steam exigem mexer nas opções de lançamento de cada jogo |
| C. gamescope **como sessão** no lugar do sway | é o modelo do SteamOS (um app por vez, troca de foco controlada pelo compositor) | a Shell (Electron Wayland) teria de rodar por Xwayland/`--expose-wayland`; o Nexus perde o controle de foco/áreas que já funciona (show_shell/show_game, scratchpad da Steam, menu por cima do jogo) e precisaria reimplementar via átomos X do gamescope; maior risco de tela preta; não roda sem Vulkan (VMs, Pi 4) |

Recomendação: **A agora, B como opção avançada no PC** ("Escala e limite de FPS" por jogo em Ajustes), só quando
`gamescope` e Vulkan 1.3 existirem, desligado por padrão. C só se B mostrar ganho claro e houver hardware para testar.
Para decidir com números: medir no PC do usuário (GPU real) FPS/latência de um jogo com e sem gamescope e o custo de
RAM do gamescope (~60–100 MB esperados).
