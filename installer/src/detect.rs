use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct System {
    pub id: String,
    pub id_like: Vec<String>,
    pub version_id: String,
    pub pretty: String,
    pub arch: String,
    pub family: Family,
    pub supported: Support,
    pub gpu_vendor: String,
    pub memory_mb: u64,
    pub disk_free_mb: u64,
    pub has_steam: bool,
    pub has_flatpak: bool,
    /// navegador para o streaming (Netflix, YouTube TV…)
    #[serde(default)]
    pub has_browser: bool,
    pub display_manager: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Debian,
    Fedora,
    Arch,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// distro e versão do MVP
    Supported,
    /// família conhecida, ainda não validada
    Experimental,
    No,
}

pub fn parse_os_release(text: &str) -> (String, Vec<String>, String, String) {
    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(|v| v.trim_matches('"').to_string()).unwrap_or_default();
    (get("ID"), get("ID_LIKE").split_whitespace().map(String::from).collect(), get("VERSION_ID"), get("PRETTY_NAME"))
}

pub fn family_of(id: &str, like: &[String]) -> Family {
    let all: Vec<&str> = std::iter::once(id).chain(like.iter().map(String::as_str)).collect();
    if all.iter().any(|x| ["debian", "ubuntu"].contains(x)) {
        Family::Debian
    } else if all.iter().any(|x| ["fedora", "rhel"].contains(x)) {
        Family::Fedora
    } else if all.iter().any(|x| ["arch"].contains(x)) {
        Family::Arch
    } else {
        Family::Unknown
    }
}

/// Arquiteturas com pacote do Nexus: PC (x86_64) e ARM 64 bits (Raspberry Pi 4/5 com sistema de 64 bits).
pub fn arch_ok(arch: &str) -> bool {
    matches!(arch, "x86_64" | "aarch64")
}

/// A Steam para Linux só existe para PC (x86_64); no ARM o Nexus funciona sem ela (emuladores, streaming, apps).
pub fn steam_possible(arch: &str) -> bool {
    arch == "x86_64"
}

pub fn support_of(id: &str, version: &str, arch: &str) -> Support {
    if !arch_ok(arch) {
        return Support::No;
    }
    if arch == "aarch64" {
        return if family_of(id, &[]) == Family::Debian || id == "raspbian" { Support::Experimental } else { Support::No };
    }
    if id == "ubuntu" && version == "24.04" {
        return Support::Supported;
    }
    match family_of(id, &[]) {
        Family::Unknown => Support::No,
        _ => Support::Experimental,
    }
}

fn meminfo_mb(t: &str) -> u64 {
    t.lines().find(|l| l.starts_with("MemTotal:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).map(|k| k / 1024).unwrap_or(0)
}

fn gpu() -> String {
    std::fs::read_dir("/sys/class/drm")
        .ok()
        .and_then(|rd| rd.flatten().filter(|e| { let n = e.file_name().to_string_lossy().to_string(); n.starts_with("card") && !n.contains('-') }).find_map(|e| std::fs::read_to_string(e.path().join("device/vendor")).ok()))
        .map(|v| match v.trim().to_lowercase().as_str() { "0x1002" => "amd", "0x8086" => "intel", "0x10de" => "nvidia", _ => "unknown" })
        .filter(|g| *g != "unknown")
        .or_else(|| {
            // placas ARM não têm ID PCI: reconhece pelo driver (vc4/v3d = Raspberry Pi)
            std::fs::read_dir("/sys/class/drm").ok()?.flatten().find_map(|e| {
                let d = std::fs::read_link(e.path().join("device/driver")).ok()?;
                match d.file_name()?.to_str()? {
                    "vc4" | "v3d" => Some("broadcom"),
                    "panfrost" | "panthor" | "lima" => Some("mali"),
                    _ => None,
                }
            })
        })
        .unwrap_or("unknown")
        .to_string()
}

fn free_mb(path: &str) -> u64 {
    let Ok(c) = std::ffi::CString::new(path) else { return 0 };
    #[repr(C)]
    struct StatVfs { f_bsize: u64, f_frsize: u64, f_blocks: u64, f_bfree: u64, f_bavail: u64, rest: [u64; 12] }
    extern "C" { fn statvfs(p: *const std::ffi::c_char, b: *mut StatVfs) -> i32; }
    unsafe {
        let mut s: StatVfs = std::mem::zeroed();
        if statvfs(c.as_ptr(), &mut s) == 0 { s.f_bavail * s.f_frsize / 1_048_576 } else { 0 }
    }
}

fn which(bin: &str) -> bool {
    std::env::var("PATH").unwrap_or_default().split(':').any(|d| Path::new(d).join(bin).is_file())
}

fn display_manager() -> Option<String> {
    let l = std::fs::read_link("/etc/systemd/system/display-manager.service").ok()?;
    let n = l.file_stem()?.to_string_lossy().to_string();
    Some(n)
}

impl System {
    pub fn probe() -> System {
        let (id, id_like, version_id, pretty) = parse_os_release(&std::fs::read_to_string("/etc/os-release").unwrap_or_default());
        let arch = std::env::consts::ARCH.to_string();
        System {
            family: family_of(&id, &id_like),
            supported: support_of(&id, &version_id, &arch),
            gpu_vendor: gpu(),
            memory_mb: meminfo_mb(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default()),
            disk_free_mb: free_mb("/opt").max(free_mb("/")),
            has_steam: which("steam") || Path::new("/var/lib/flatpak/app/com.valvesoftware.Steam").exists(),
            has_flatpak: which("flatpak"),
            has_browser: ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "microsoft-edge", "brave-browser", "firefox"]
                .iter()
                .any(|b| which(b) || Path::new("/snap/bin").join(b).exists() || Path::new("/usr/bin").join(b).exists()),
            display_manager: display_manager(),
            id, id_like, version_id, pretty, arch,
        }
    }

    pub fn requirement_problems(&self) -> Vec<String> {
        let mut p = vec![];
        if !arch_ok(&self.arch) {
            p.push(format!("Arquitetura {} não suportada (precisa de x86_64, ou aarch64 = Raspberry Pi com sistema de 64 bits).", self.arch));
        }
        if self.supported == Support::No && arch_ok(&self.arch) {
            let fams = if self.arch == "aarch64" { "no ARM: Raspberry Pi OS / Debian / Ubuntu" } else { "famílias: Debian/Ubuntu, Fedora, Arch" };
            p.push(format!("{} ainda não é suportado ({fams}).", self.pretty));
        }
        // no ARM não há Steam: 2 GB bastam para interface + emuladores leves (Pi 4/5 de 2 GB)
        let (min_mem, min_disk) = if self.arch == "aarch64" { (1800, 3000) } else { (3500, 6000) };
        if self.memory_mb < min_mem {
            p.push(format!("Memória insuficiente: {} MB (mínimo {} GB).", self.memory_mb, min_mem.div_ceil(1000)));
        }
        if self.disk_free_mb < min_disk {
            p.push(format!("Pouco espaço livre: {} MB (mínimo {} GB).", self.disk_free_mb, min_disk / 1000));
        }
        if self.gpu_vendor == "nvidia" {
            p.push("GPU NVIDIA ainda não validada com a sessão do Nexus (planejado para a V2).".into());
        }
        p
    }

    pub fn describe(&self) -> String {
        let sup = match self.supported { Support::Supported => "suportado", Support::Experimental => "experimental", Support::No => "não suportado" };
        format!(
            "Sistema:   {} ({}), {} — {}\nGPU:       {}\nMemória:   {} MB\nEspaço:    {} MB livres\nSteam:     {}\nFlatpak:   {}\nLogin:     {}",
            self.pretty, self.arch, self.version_id, sup, self.gpu_vendor, self.memory_mb, self.disk_free_mb,
            if self.has_steam { "instalada" } else { "não encontrada" },
            if self.has_flatpak { "sim" } else { "não" },
            self.display_manager.as_deref().unwrap_or("não detectado")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release_and_families() {
        let t = "NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\nVERSION_ID=\"24.04\"\nPRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\n";
        let (id, like, v, pretty) = parse_os_release(t);
        assert_eq!((id.as_str(), v.as_str(), pretty.as_str()), ("ubuntu", "24.04", "Ubuntu 24.04.1 LTS"));
        assert_eq!(family_of(&id, &like), Family::Debian);
        assert_eq!(family_of("cachyos", &["arch".to_string()]), Family::Arch);
        assert_eq!(family_of("nobara", &["fedora".to_string()]), Family::Fedora);
        assert_eq!(family_of("alpine", &[]), Family::Unknown);
    }

    #[test]
    fn support_matrix() {
        assert_eq!(support_of("ubuntu", "24.04", "x86_64"), Support::Supported);
        assert_eq!(support_of("ubuntu", "22.04", "x86_64"), Support::Experimental);
        assert_eq!(support_of("fedora", "42", "x86_64"), Support::Experimental);
        assert_eq!(support_of("alpine", "3", "x86_64"), Support::No);
        assert_eq!(support_of("debian", "12", "aarch64"), Support::Experimental, "Raspberry Pi OS 64 bits");
        assert_eq!(support_of("ubuntu", "24.04", "aarch64"), Support::Experimental);
        assert_eq!(support_of("fedora", "42", "aarch64"), Support::No);
        assert_eq!(support_of("raspbian", "12", "armv7l"), Support::No, "Pi OS de 32 bits");
        assert!(steam_possible("x86_64") && !steam_possible("aarch64"));
    }

    fn sys(mem: u64, disk: u64, gpu: &str) -> System {
        System { id: "ubuntu".into(), id_like: vec![], version_id: "24.04".into(), pretty: "Ubuntu".into(), arch: "x86_64".into(), family: Family::Debian, supported: Support::Supported, gpu_vendor: gpu.into(), memory_mb: mem, disk_free_mb: disk, has_steam: false, has_flatpak: false, has_browser: true, display_manager: None }
    }

    #[test]
    fn requirements() {
        assert!(sys(8000, 50000, "amd").requirement_problems().is_empty());
        assert_eq!(sys(2000, 50000, "amd").requirement_problems().len(), 1);
        assert_eq!(sys(8000, 1000, "amd").requirement_problems().len(), 1);
        assert!(sys(8000, 50000, "nvidia").requirement_problems()[0].contains("NVIDIA"));
    }
}
