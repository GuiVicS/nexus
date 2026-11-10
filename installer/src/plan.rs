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

pub fn build(sys: &System, o: &Options) -> Vec<Step> {
    let mut s = vec![];
    let step = |title: &str, f: Family, pkgs: &[&str]| Step { title: title.into(), action: Action::Cmd(install_cmd(f, pkgs)) };
    if sys.family == Family::Debian {
        s.push(cmd("Atualizar a lista de pacotes", &["apt-get", "update"]));
    }
    s.push(step("Instalar o Nexus (Shell, Core e sessão)", sys.family, &["nexus"]));
    if o.steam {
        match sys.family {
            Family::Debian => {
                s.push(cmd("Habilitar arquitetura de 32 bits (necessária para a Steam)", &["dpkg", "--add-architecture", "i386"]));
                s.push(cmd("Atualizar a lista de pacotes", &["apt-get", "update"]));
            }
            Family::Fedora => s.push(cmd("Habilitar o RPM Fusion não livre (Steam)", &["dnf", "install", "-y", "rpmfusion-nonfree-release"])),
            Family::Arch => {}
            Family::Unknown => {}
        }
        s.push(step("Instalar a Steam", sys.family, &["steam"]));
    }
    if o.emulation {
        s.push(cmd("Instalar o RetroArch (Flatpak)", &["flatpak", "install", "-y", "flathub", "org.libretro.RetroArch"]));
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
    fn debian_plan_with_steam_and_autologin() {
        let p = build(&sys(Family::Debian, Some("sddm")), &Options { steam: true, emulation: false, autologin_user: Some("lia".into()) });
        let t: Vec<String> = p.iter().map(|s| s.display()).collect();
        assert!(t[0].starts_with("apt-get update"));
        assert!(t.iter().any(|x| x == "apt-get install -y nexus"));
        assert!(t.iter().any(|x| x == "dpkg --add-architecture i386"));
        assert!(t.iter().any(|x| x == "apt-get install -y steam"));
        assert!(t.last().unwrap().contains("autologin"));
        assert!(!t.iter().any(|x| x.contains("RetroArch")), "só entra o que foi pedido");
    }

    #[test]
    fn minimal_plan_and_other_families() {
        let p = build(&sys(Family::Arch, None), &Options { steam: false, emulation: false, autologin_user: Some("lia".into()) });
        let t: Vec<String> = p.iter().map(|s| s.display()).collect();
        assert!(t.contains(&"pacman -S --needed --noconfirm nexus".to_string()));
        assert!(!t.iter().any(|x| x.contains("autologin")), "sem gerenciador de login conhecido não mexe no login");
        assert!(!t.iter().any(|x| x.contains("steam")));
        let f = build(&sys(Family::Fedora, None), &Options { steam: true, emulation: true, autologin_user: None });
        assert!(f.iter().any(|s| s.display().contains("rpmfusion")));
        assert!(f.iter().any(|s| s.display().contains("org.libretro.RetroArch")));
    }
}
