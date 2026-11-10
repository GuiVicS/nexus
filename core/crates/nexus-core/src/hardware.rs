//! Detecção de hardware → perfil de capacidades. A Shell lê `effects`, nunca o nome do aparelho.
use nexus_protocol::HardwareProfile;
use std::path::Path;

pub fn mem_mb(meminfo: &str) -> u64 {
    meminfo.lines().find(|l| l.starts_with("MemTotal:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).map(|kb| kb / 1024).unwrap_or(0)
}

pub fn gpu_vendor_name(id: &str) -> &'static str {
    match id.trim().to_lowercase().as_str() {
        "0x1002" => "amd",
        "0x8086" => "intel",
        "0x10de" => "nvidia",
        "0x15ad" | "0x80ee" | "0x1af4" | "0x1234" | "0x1b36" => "virtual", // VMware/VirtualBox/virtio/QEMU
        _ => "unknown",
    }
}

/// GPUs sem ID PCI (Raspberry Pi e outras placas ARM) são reconhecidas pelo driver do kernel.
pub fn gpu_driver_name(driver: &str) -> &'static str {
    match driver {
        "vc4" | "v3d" => "broadcom", // Raspberry Pi 4/5
        "panfrost" | "panthor" | "lima" => "mali",
        "msm" => "adreno",
        "amdgpu" | "radeon" => "amd",
        "i915" | "xe" => "intel",
        "nvidia" | "nouveau" | "nvidia-drm" => "nvidia",
        "vmwgfx" | "vboxvideo" | "virtio_gpu" | "virtio-pci" | "qxl" | "bochs-drm" | "cirrus" => "virtual",
        _ => "unknown",
    }
}

pub fn os_pretty(os_release: &str) -> String {
    os_release.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|v| v.trim_matches('"').to_string()).unwrap_or_else(|| "Linux".into())
}

pub fn tier(mem_mb: u64, gpu: &str) -> (&'static str, &'static str) {
    // GPU de placa ARM ou de VM: dá conta da interface, mas sem efeitos pesados (desfoque, sombras grandes)
    let weak = matches!(gpu, "broadcom" | "mali" | "adreno" | "virtual");
    if mem_mb < 3500 || gpu == "unknown" && mem_mb < 6000 {
        ("low", "minimal")
    } else if weak {
        ("mid", "reduced")
    } else if mem_mb < 7500 {
        ("mid", "reduced")
    } else {
        ("high", "full")
    }
}

fn disk(path: &Path) -> (u64, u64) {
    let Ok(c) = std::ffi::CString::new(path.to_string_lossy().as_bytes()) else { return (0, 0) };
    unsafe {
        let mut s: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut s) == 0 {
            let bs = s.f_frsize as u64;
            return (s.f_bavail as u64 * bs, s.f_blocks as u64 * bs);
        }
    }
    (0, 0)
}

pub fn detect(providers: Vec<String>, data_dir: &Path) -> HardwareProfile {
    let mem = mem_mb(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default());
    let cards: Vec<_> = std::fs::read_dir("/sys/class/drm")
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.starts_with("card") && !n.contains('-')
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    // no Pi há dois "cards" (vc4 = saída de vídeo, v3d = 3D): vale o primeiro reconhecido
    let gpu = cards
        .iter()
        .map(|c| {
            let by_id = std::fs::read_to_string(c.join("device/vendor")).map(|v| gpu_vendor_name(&v)).unwrap_or("unknown");
            if by_id != "unknown" {
                return by_id;
            }
            std::fs::read_link(c.join("device/driver")).ok().and_then(|l| l.file_name().map(|n| gpu_driver_name(&n.to_string_lossy()))).unwrap_or("unknown")
        })
        .find(|g| *g != "unknown")
        .unwrap_or("unknown");
    let (t, fx) = tier(mem, gpu);
    let probe = if data_dir.exists() { data_dir } else { Path::new("/") };
    let (free, total) = disk(probe);
    HardwareProfile {
        architecture: std::env::consts::ARCH.into(),
        performance_tier: t.into(),
        memory_mb: mem,
        gpu_vendor: gpu.into(),
        effects: fx.into(),
        providers_available: providers,
        os: os_pretty(&std::fs::read_to_string("/etc/os-release").unwrap_or_default()),
        disk_free_bytes: free,
        disk_total_bytes: total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses() {
        assert_eq!(mem_mb("MemTotal:       16384000 kB\nMemFree: 1 kB"), 16000);
        assert_eq!(gpu_vendor_name("0x10DE\n"), "nvidia");
        assert_eq!(os_pretty("NAME=x\nPRETTY_NAME=\"Ubuntu 24.04 LTS\"\n"), "Ubuntu 24.04 LTS");
        assert_eq!(tier(16000, "amd"), ("high", "full"));
        assert_eq!(tier(4096, "intel"), ("mid", "reduced"));
        assert_eq!(tier(2048, "intel"), ("low", "minimal"));
        assert_eq!(tier(8000, "broadcom"), ("mid", "reduced"), "Raspberry Pi 5 de 8 GB");
        assert_eq!(tier(2048, "broadcom"), ("low", "minimal"), "Pi de 2 GB");
        assert_eq!(gpu_driver_name("v3d"), "broadcom");
        assert_eq!(gpu_vendor_name("0x15ad"), "virtual");
    }
    #[test]
    fn detect_runs_here() {
        let h = detect(vec!["native".into()], Path::new("/tmp"));
        assert!(h.memory_mb > 0 && h.disk_total_bytes > 0);
    }
}
