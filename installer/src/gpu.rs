//! GPU: fabricante, driver em uso e o que isso significa para o Nexus (ver docs/GPU.md).
//! Lê só o /sys (sem lspci/glxinfo); `root` existe para os testes.
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Gpu {
    /// amd, intel, nvidia, broadcom, mali, virtual, unknown
    pub vendor: String,
    /// driver do kernel em uso (amdgpu, i915, xe, nouveau, nvidia, vc4…); vazio se nenhum
    pub driver: String,
    /// versão do driver proprietário da NVIDIA (/sys/module/nvidia/version)
    pub nvidia_version: Option<String>,
    /// nvidia-drm.modeset (obrigatório para Wayland/sway)
    pub nvidia_modeset: Option<bool>,
}

/// Nível de suporte, do melhor para o pior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// testado e recomendado
    Good,
    /// funciona, com limitações conhecidas
    Limited,
    /// a interface abre, mas jogos não vão rodar bem até corrigir o driver
    NeedsDriver,
}

pub struct Advice {
    pub level: Level,
    pub notes: Vec<String>,
}

fn vendor_of_pci(id: &str) -> Option<&'static str> {
    Some(match id.trim().to_lowercase().as_str() {
        "0x1002" => "amd",
        "0x8086" => "intel",
        "0x10de" => "nvidia",
        "0x15ad" | "0x80ee" | "0x1af4" | "0x1b36" | "0x1234" | "0x1013" => "virtual",
        _ => return None,
    })
}

fn vendor_of_driver(d: &str) -> &'static str {
    match d {
        "amdgpu" | "radeon" => "amd",
        "i915" | "xe" => "intel",
        "nouveau" | "nvidia" => "nvidia",
        "vc4" | "v3d" => "broadcom",
        "panfrost" | "panthor" | "lima" => "mali",
        "vmwgfx" | "vboxvideo" | "virtio_gpu" | "virtio-pci" | "qxl" | "bochs-drm" | "bochs" | "cirrus" | "cirrus-qemu" => "virtual",
        _ => "unknown",
    }
}

pub fn probe() -> Gpu {
    probe_at(Path::new("/"))
}

pub fn probe_at(root: &Path) -> Gpu {
    let drm = root.join("sys/class/drm");
    let mut cards: Vec<_> = std::fs::read_dir(&drm)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with("card") && !n.contains('-')).collect())
        .unwrap_or_default();
    cards.sort();
    // com duas GPUs (notebook híbrido), a dedicada é a que importa para jogos
    let mut found: Vec<(String, String)> = cards
        .iter()
        .map(|c| {
            let dev = drm.join(c).join("device");
            let driver = std::fs::read_link(dev.join("driver")).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).unwrap_or_default();
            let vendor = std::fs::read_to_string(dev.join("vendor")).ok().and_then(|v| vendor_of_pci(&v)).unwrap_or_else(|| vendor_of_driver(&driver));
            (vendor.to_string(), driver)
        })
        .collect();
    let rank = |v: &str| match v {
        "nvidia" => 0,
        "amd" => 1,
        "intel" => 2,
        "unknown" => 9,
        _ => 5,
    };
    found.sort_by_key(|(v, _)| rank(v));
    let (vendor, driver) = found.into_iter().next().unwrap_or(("unknown".into(), String::new()));
    let module = root.join("sys/module");
    let nvidia_version = std::fs::read_to_string(module.join("nvidia/version")).ok().map(|v| v.trim().to_string());
    let nvidia_modeset = std::fs::read_to_string(module.join("nvidia_drm/parameters/modeset")).ok().map(|v| matches!(v.trim(), "Y" | "1"));
    Gpu { vendor, driver, nvidia_version, nvidia_modeset }
}

fn major(v: &str) -> u32 {
    v.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0)
}

/// Comando para instalar o driver da NVIDIA, por família.
fn nvidia_install_hint(debian_like: bool, is_ubuntu: bool) -> &'static str {
    if is_ubuntu {
        "sudo ubuntu-drivers install"
    } else if debian_like {
        "sudo apt install nvidia-driver firmware-misc-nonfree   (precisa de contrib/non-free)"
    } else {
        "instale o driver proprietário da NVIDIA pelo gerenciador de pacotes da distribuição"
    }
}

pub fn advice(g: &Gpu, debian_like: bool, is_ubuntu: bool) -> Advice {
    let mut notes = Vec::new();
    let level = match (g.vendor.as_str(), g.driver.as_str()) {
        ("amd", "amdgpu") | ("intel", "i915") | ("intel", "xe") => Level::Good,
        ("amd", "radeon") => {
            notes.push("GPU AMD antiga (driver radeon, sem Vulkan): a interface e os emuladores funcionam, mas a maioria dos jogos da Steam (Proton) não roda.".into());
            Level::Limited
        }
        ("nvidia", "nvidia") => {
            let v = g.nvidia_version.as_deref().unwrap_or("?");
            let mut lvl = Level::Limited;
            if g.nvidia_modeset != Some(true) {
                notes.push("NVIDIA: falta ligar o modeset do driver (obrigatório para a sessão Wayland do Nexus). Rode:\n    echo 'options nvidia-drm modeset=1 fbdev=1' | sudo tee /etc/modprobe.d/nexus-nvidia.conf && sudo update-initramfs -u\n  e reinicie.".into());
                lvl = Level::NeedsDriver;
            }
            if major(v) < 535 {
                notes.push(format!("NVIDIA: driver {v} é antigo para Wayland; use o 535 ou mais novo ({}).", nvidia_install_hint(debian_like, is_ubuntu)));
                lvl = Level::NeedsDriver;
            }
            notes.push("NVIDIA com driver proprietário: suporte experimental (o sway roda com --unsupported-gpu). Se a tela ficar preta: Ctrl+Alt+F3, entre e rode `sudo nexus-installer revert-autologin`.".into());
            lvl
        }
        ("nvidia", _) => {
            notes.push(format!(
                "NVIDIA com driver aberto (nouveau): a interface abre, mas os jogos ficam muito lentos e sem Vulkan. Instale o driver proprietário: {}",
                nvidia_install_hint(debian_like, is_ubuntu)
            ));
            Level::NeedsDriver
        }
        ("broadcom", _) | ("mali", _) => {
            notes.push("GPU de placa ARM: interface e emuladores leves (até PS1/N64 conforme a placa); sem Steam.".into());
            Level::Limited
        }
        ("virtual", _) => {
            notes.push("Máquina virtual: vídeo por software. Serve para testar a interface; jogos 3D não rodam bem.".into());
            Level::Limited
        }
        (_, "") => {
            notes.push("Nenhum driver de vídeo ativo: o Nexus vai usar renderização por software (lento).".into());
            Level::NeedsDriver
        }
        _ => {
            notes.push(format!("GPU não reconhecida (driver {}): pode funcionar, mas não foi testada.", g.driver));
            Level::Limited
        }
    };
    Advice { level, notes }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(root: &Path, n: &str, vendor: Option<&str>, driver: &str) {
        let dev = root.join(format!("sys/class/drm/{n}/device"));
        std::fs::create_dir_all(&dev).unwrap();
        if let Some(v) = vendor {
            std::fs::write(dev.join("vendor"), format!("{v}\n")).unwrap();
        }
        let drv = root.join("sys/bus/pci/drivers").join(driver);
        std::fs::create_dir_all(&drv).unwrap();
        std::os::unix::fs::symlink(&drv, dev.join("driver")).unwrap();
        std::fs::create_dir_all(root.join(format!("sys/class/drm/{n}-HDMI-A-1"))).unwrap();
    }

    #[test]
    fn hybrid_laptop_prefers_the_dedicated_gpu() {
        let d = tempfile::tempdir().unwrap();
        card(d.path(), "card0", Some("0x8086"), "i915");
        card(d.path(), "card1", Some("0x10de"), "nvidia");
        std::fs::create_dir_all(d.path().join("sys/module/nvidia_drm/parameters")).unwrap();
        std::fs::create_dir_all(d.path().join("sys/module/nvidia")).unwrap();
        std::fs::write(d.path().join("sys/module/nvidia/version"), "550.120\n").unwrap();
        std::fs::write(d.path().join("sys/module/nvidia_drm/parameters/modeset"), "N\n").unwrap();
        let g = probe_at(d.path());
        assert_eq!((g.vendor.as_str(), g.driver.as_str()), ("nvidia", "nvidia"));
        assert_eq!(g.nvidia_version.as_deref(), Some("550.120"));
        assert_eq!(g.nvidia_modeset, Some(false));
        let a = advice(&g, true, false);
        assert_eq!(a.level, Level::NeedsDriver);
        assert!(a.notes[0].contains("modeset=1"));
    }

    #[test]
    fn matrix() {
        let g = |v: &str, d: &str| Gpu { vendor: v.into(), driver: d.into(), nvidia_version: None, nvidia_modeset: None };
        assert_eq!(advice(&g("amd", "amdgpu"), true, false).level, Level::Good);
        assert_eq!(advice(&g("intel", "xe"), true, false).level, Level::Good);
        assert_eq!(advice(&g("amd", "radeon"), true, false).level, Level::Limited);
        let n = advice(&g("nvidia", "nouveau"), true, true);
        assert_eq!(n.level, Level::NeedsDriver);
        assert!(n.notes[0].contains("ubuntu-drivers"));
        assert!(advice(&g("nvidia", "nouveau"), true, false).notes[0].contains("nvidia-driver"));
        let ok = Gpu { vendor: "nvidia".into(), driver: "nvidia".into(), nvidia_version: Some("560.35.03".into()), nvidia_modeset: Some(true) };
        assert_eq!(advice(&ok, true, false).level, Level::Limited);
        let old = Gpu { nvidia_version: Some("470.256".into()), ..ok };
        assert_eq!(advice(&old, true, false).level, Level::NeedsDriver);
        assert_eq!(advice(&g("virtual", "vmwgfx"), true, false).level, Level::Limited);
        assert_eq!(advice(&g("unknown", ""), true, false).level, Level::NeedsDriver);
    }

    #[test]
    fn arm_board_by_driver() {
        let d = tempfile::tempdir().unwrap();
        card(d.path(), "card1", None, "vc4");
        assert_eq!(probe_at(d.path()).vendor, "broadcom");
    }
}
