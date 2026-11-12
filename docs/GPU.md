# GPUs: o que funciona

O instalador detecta a GPU e o driver em uso (`nexus-installer detect`, código em `installer/src/gpu.rs`) e avisa o
que corrigir. GPU nunca bloqueia a instalação: os avisos dizem se a interface roda e se os jogos vão rodar bem.

| GPU / driver | Nível | Interface | Steam (Proton/Vulkan) | Emuladores | Observações |
|---|---|---|---|---|---|
| AMD (amdgpu, GCN 1.2+/RDNA) | **Recomendada** | sim | sim (RADV) | sim | Melhor caminho no Linux; Mesa do sistema basta. |
| Intel (i915/xe, Gen9+/Arc) | **Recomendada** | sim | sim, jogos leves (ANV) | sim | Integradas: limite é o desempenho, não o driver. |
| AMD antiga (radeon, pré-GCN) | Limitada | sim | não (sem Vulkan) | sim | Só jogos nativos OpenGL e emulação. |
| NVIDIA, driver proprietário ≥ 535 com `nvidia-drm.modeset=1` | Experimental | sim | sim | sim | sway roda com `--unsupported-gpu`. Não validada em hardware real ainda. |
| NVIDIA, driver proprietário < 535 ou sem modeset | Precisa de ajuste | instável | — | — | Ver abaixo. |
| NVIDIA, nouveau | Precisa de driver | sim (lento) | não na prática | leves | Instale o driver proprietário. |
| Raspberry Pi 4/5 (vc4/v3d) | Limitada | sim | não (ARM) | até PS1/N64 | Ver `docs/BASE.md`. |
| Mali (panfrost/panthor) | Limitada | sim | não (ARM) | leves | Não testado. |
| Máquina virtual (vmwgfx, virtio, qxl…) | Só teste | sim | não (lavapipe) | 2D | É a VM de desenvolvimento. |
| Sem driver | Precisa de driver | software (pixman) | não | não | A sessão cai para o renderizador por software. |

**Testado de verdade:** só a VM (vmwgfx + lavapipe). As outras linhas vêm da documentação dos drivers e do
comportamento conhecido do sway/Mesa; ficam como "a validar em hardware" no `docs/STATUS.md`.

## NVIDIA

1. Driver proprietário:
   - Ubuntu: `sudo ubuntu-drivers install`
   - Debian: ligar `contrib non-free` e `sudo apt install nvidia-driver firmware-misc-nonfree`
2. Modeset (obrigatório para Wayland):
   ```sh
   echo 'options nvidia-drm modeset=1 fbdev=1' | sudo tee /etc/modprobe.d/nexus-nvidia.conf
   sudo update-initramfs -u && sudo reboot
   ```
   Conferir: `cat /sys/module/nvidia_drm/parameters/modeset` → `Y`.
3. Se a tela ficar preta ao entrar no Nexus: `Ctrl+Alt+F3`, entre no terminal e rode
   `sudo nexus-installer revert-autologin` (ou `sudo touch /etc/nexus/force-desktop`) para voltar ao desktop.

## Notebook híbrido (Intel + NVIDIA/AMD)

O instalador considera a GPU dedicada. A interface roda na integrada; jogos da Steam escolhem a dedicada sozinhos na
maioria dos casos (Vulkan lista as duas). Para forçar num jogo NVIDIA: opção de lançamento
`__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia %command%`.
