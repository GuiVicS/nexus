use crate::autologin::{self, Dm};
use crate::detect::{Family, System};
use std::process::Command;

pub struct Options {
    pub steam: bool,
    pub emulation: bool,
    pub autologin_user: Option<String>,
}

pub enum Action {
    Cmd(Vec<String>),
    Autologin { dm: Dm, user: String },
}

pub struct Step {
    pub title: String,
    pub action: Action,
}

impl Step {
    pub fn display(&self) -> String {
        match &self.action {
            Action::Cmd(c) => c.join(" "),
            Action::Autologin { dm, user } => format!("(escreve a configuração de autologin de {dm:?} para {user}, com backup)"),
        }
    }

    pub fn run(&self) -> Result<(), String> {
        match &self.action {
            Action::Cmd(c) => {
                let st = Command::new(&c[0]).args(&c[1..]).status().map_err(|e| format!("{}: {e}", c[0]))?;
                if st.success() { Ok(()) } else { Err(format!("{} saiu com {st}", c[0])) }
            }
            Action::Autologin { dm, user } => autologin::configure(std::path::Path::new("/"), *dm, user).map_err(|e| e.to_string()),
        }
    }
}

fn cmd(title: &str, c: &[&str]) -> Step {
    Step { title: title.into(), action: Action::Cmd(c.iter().map(|s| s.to_string()).collect()) }
}

/// Pacotes do Nexus por família (nomes dos pacotes publicados em nosso repositório).
pub fn install_cmd(f: Family, pkgs: &[&str]) -> Vec<String> {
    let mut v: Vec<&str> = match f {
        Family::Debian => vec!["apt-get", "install", "-y"],
        Family::Fedora => vec!["dnf", "install", "-y"],
        Family::Arch => vec!["pacman", "-S", "--needed", "--noconfirm"],
        Family::Unknown => vec!["false"],
    };
    v.extend(pkgs);
    v.into_iter().map(String::from).collect()
}

fn sh(title: &str, script: &str) -> Step {
    Step { title: title.into(), action: Action::Cmd(vec!["sh".into(), "-c".into(), script.into()]) }
}

pub fn build(sys: &System, o: &Options) -> Vec<Step> {
    let mut s = vec![];
    let step = |title: &str, f: Family, pkgs: &[&str]| Step { title: title.into(), action: Action::Cmd(install_cmd(f, pkgs)) };
    let installed = std::path::Path::new("/usr/bin/nexus-core").exists();
    if sys.family == Family::Debian {
        s.push(cmd("Atualizar a lista de pacotes", &["apt-get", "update"]));
    }
    if !installed {
        s.push(step("Instalar o Nexus (Shell, Core e sessão)", sys.family, &["nexus"]));
    }
    if o.steam && !crate::detect::steam_possible(&sys.arch) {
        // Raspberry Pi e outros ARM: a Steam para Linux não existe para essa arquitetura
        s.push(sh("Steam: não disponível nesta arquitetura (ARM) — o Nexus segue sem ela", "true"));
    } else if o.steam {
        match sys.family {
            Family::Debian => {
                s.push(cmd("Habilitar a arquitetura de 32 bits (necessária para a Steam)", &["dpkg", "--add-architecture", "i386"]));
                if sys.id == "ubuntu" {
                    s.push(step("Instalar o gerenciador de repositórios", sys.family, &["software-properties-common"]));
                    s.push(cmd("Habilitar o repositório multiverse (onde fica a Steam)", &["add-apt-repository", "-y", "multiverse"]));
                } else {
                    s.push(sh("Habilitar contrib e non-free (onde fica a Steam)", "sed -i -E 's/^(deb .* main)( contrib)?( non-free)?( non-free-firmware)?$/\\1 contrib non-free non-free-firmware/' /etc/apt/sources.list 2>/dev/null || true"));
                }
                s.push(cmd("Atualizar a lista de pacotes", &["apt-get", "update"]));
                s.push(step("Instalar a Steam", sys.family, &["steam-installer"]));
            }
            Family::Fedora => {
                s.push(sh("Habilitar o RPM Fusion (livre e não livre)", "dnf install -y \"https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm\" \"https://mirrors.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-$(rpm -E %fedora).noarch.rpm\""));
                s.push(step("Instalar a Steam", sys.family, &["steam"]));
            }
            Family::Arch => {
                s.push(sh("Habilitar o repositório multilib (32 bits)", "grep -q '^\\[multilib\\]' /etc/pacman.conf || printf '\\n[multilib]\\nInclude = /etc/pacman.d/mirrorlist\\n' >> /etc/pacman.conf"));
                s.push(cmd("Atualizar a lista de pacotes", &["pacman", "-Sy"]));
                s.push(step("Instalar a Steam", sys.family, &["steam"]));
            }
            Family::Unknown => {}
        }
    }
    if o.emulation {
        if !sys.has_flatpak {
            s.push(step("Instalar o Flatpak", sys.family, &["flatpak"]));
        }
        s.push(cmd("Adicionar o repositório Flathub", &["flatpak", "remote-add", "--if-not-exists", "flathub", "https://dl.flathub.org/repo/flathub.flatpakrepo"]));
        s.push(cmd("Instalar o RetroArch (Flatpak)", &["flatpak", "install", "--system", "-y", "flathub", "org.libretro.RetroArch"]));
    }
    if let (Some(user), Some(dm)) = (&o.autologin_user, sys.display_manager.as_deref().and_then(autologin::dm_from_name)) {
        s.push(Step { title: format!("Entrar direto no Nexus ao ligar ({user})"), action: Action::Autologin { dm, user: user.clone() } });
    }
    s
}

pub fn doctor(sys: &System) -> bool {
    let mut ok = true;
    let mut line = |good: bool, what: &str, hint: &str| {
        println!("{} {what}{}", if good { "✓" } else { "✗" }, if good { String::new() } else { format!(" — {hint}") });
        ok &= good;
    };
    let exists = |p: &str| std::path::Path::new(p).exists();
    line(exists("/usr/share/wayland-sessions/nexus.desktop"), "sessão Nexus registrada", "reinstale o pacote nexus-session");
    line(exists("/usr/bin/nexus-core") || exists("/opt/nexus/bin/nexus-core"), "Core instalado", "reinstale o pacote nexus");
    line(exists("/dev/uinput"), "/dev/uinput presente", "carregue o módulo: modprobe uinput");
    line(std::fs::OpenOptions::new().write(true).open("/dev/uinput").is_ok(), "permissão de escrita em /dev/uinput (celular como controle nos jogos)", "adicione o usuário ao grupo 'input' ou instale a regra udev do Nexus");
    line(sys.has_steam, "Steam instalada", "rode: nexus-installer install --steam --yes");
    line(exists("/usr/bin/sway"), "compositor sway instalado", "instale o pacote sway");
    line(sys.requirement_problems().is_empty(), "requisitos de hardware/sistema", "veja: nexus-installer check");
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Support;

    fn sys(f: Family, dm: Option<&str>) -> System {
        System { id: "x".into(), id_like: vec![], version_id: "1".into(), pretty: "X".into(), arch: "x86_64".into(), family: f, supported: Support::Supported, gpu_vendor: "amd".into(), memory_mb: 8000, disk_free_mb: 50000, has_steam: false, has_flatpak: true, display_manager: dm.map(String::from) }
    }

    #[test]
    fn ubuntu_plan_with_steam_and_autologin() {
        let mut y = sys(Family::Debian, Some("sddm"));
        y.id = "ubuntu".into();
        let p = build(&y, &Options { steam: true, emulation: false, autologin_user: Some("lia".into()) });
        let t: Vec<String> = p.iter().map(|s| s.display()).collect();
        let pos = |needle: &str| t.iter().position(|x| x.contains(needle)).unwrap_or_else(|| panic!("falta: {needle} em {t:?}"));
        assert!(pos("dpkg --add-architecture i386") < pos("apt-get install -y steam-installer"));
        assert!(pos("add-apt-repository -y multiverse") < pos("apt-get install -y steam-installer"));
        assert!(t.last().unwrap().contains("autologin"));
        assert!(!t.iter().any(|x| x.contains("RetroArch")), "só entra o que foi pedido");
    }

    #[test]
    fn raspberry_pi_skips_steam() {
        let mut pi = sys(Family::Debian, Some("lightdm"));
        pi.id = "debian".into();
        pi.arch = "aarch64".into();
        let p = build(&pi, &Options { steam: true, emulation: true, autologin_user: Some("pi".into()) });
        let t: Vec<String> = p.iter().map(|s| s.display()).collect();
        assert!(!t.iter().any(|x| x.contains("i386") || x.contains("steam-installer")), "sem Steam nem 32 bits no ARM: {t:?}");
        assert!(t.iter().any(|x| x.contains("RetroArch")));
        assert!(t.last().unwrap().contains("autologin"), "LightDM do Raspberry Pi OS");
    }

    #[test]
    fn arch_fedora_and_emulation_plans() {
        let p = build(&sys(Family::Arch, None), &Options { steam: true, emulation: false, autologin_user: Some("lia".into()) });
        let t: Vec<String> = p.iter().map(|s| s.display()).collect();
        assert!(t.iter().any(|x| x.contains("multilib")));
        assert!(t.iter().any(|x| x == "pacman -S --needed --noconfirm steam"));
        assert!(!t.iter().any(|x| x.contains("autologin")), "sem gerenciador de login conhecido não mexe no login");
        let mut nf = sys(Family::Fedora, None);
        nf.has_flatpak = false;
        let f = build(&nf, &Options { steam: true, emulation: true, autologin_user: None });
        let t: Vec<String> = f.iter().map(|s| s.display()).collect();
        assert!(t.iter().any(|x| x.contains("rpmfusion-nonfree-release")));
        let fl = t.iter().position(|x| x == "dnf install -y flatpak").unwrap();
        let hub = t.iter().position(|x| x.contains("remote-add")).unwrap();
        let ra = t.iter().position(|x| x.contains("org.libretro.RetroArch")).unwrap();
        assert!(fl < hub && hub < ra, "Flatpak, depois Flathub, depois RetroArch");
    }
}
