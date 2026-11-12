use crate::autologin::{self, Dm};
use crate::detect::{Family, System};
use std::process::Command;

pub struct Options {
    pub steam: bool,
    pub emulation: bool,
    pub autologin_user: Option<String>,
    /// sistema leve para jogos (nexus-slim): serviços dispensáveis desligados, zram, apt fora do horário de jogo
    pub slim: bool,
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

/// Debian: a Steam fica em "contrib" (e depende de "non-free"). Acrescenta os componentes que faltam nos dois
/// formatos do apt: o antigo (`/etc/apt/sources.list`, Debian 12) e o deb822 (`debian.sources`, Debian 13+).
/// `$ROOT` só existe nos testes.
pub const DEBIAN_COMPONENTS: &str = r#"set -e
for f in "$ROOT/etc/apt/sources.list" "$ROOT"/etc/apt/sources.list.d/*.list; do
  [ -f "$f" ] && sed -i -E '/^deb(-src)? [^#]*debian[^#]* main/{/ contrib( |$)/!s/$/ contrib/;/ non-free( |$)/!s/$/ non-free/}' "$f"
done
for f in "$ROOT"/etc/apt/sources.list.d/*.sources; do
  [ -f "$f" ] && grep -q '^URIs:.*debian' "$f" && sed -i -E '/^Components:.*main/{/ contrib( |$)/!s/$/ contrib/;/ non-free( |$)/!s/$/ non-free/}' "$f"
done
true"#;

/// Depois do `apt-get update`: sem candidato para o steam-installer, para com uma explicação em vez de um erro do apt.
pub const STEAM_AVAILABLE: &str = r#"if LC_ALL=C apt-cache policy steam-installer 2>/dev/null | grep -q 'Candidate: [0-9]'; then exit 0; fi
echo "A Steam não apareceu nos repositórios do sistema." >&2
echo "  Debian: o repositório precisa dos componentes contrib e non-free (veja /etc/apt/sources.list.d/)." >&2
echo "  Ubuntu: precisa do multiverse (sudo add-apt-repository multiverse)." >&2
echo "  A arquitetura de 32 bits também é necessária: dpkg --print-foreign-architectures deve mostrar i386." >&2
echo "Corrija e rode o instalador de novo, ou instale sem a Steam: sudo ./install.sh --sem-steam" >&2
exit 1"#;

/// Debian usa `plymouth-set-default-theme`; Ubuntu, a alternativa `default.plymouth`. Sem Plymouth: nada a fazer.
/// Desfazer: `plymouth-set-default-theme -R <tema anterior>` (Debian) ou `update-alternatives --config default.plymouth`.
pub const PLYMOUTH_THEME: &str = r#"T=/usr/share/plymouth/themes/nexus/nexus.plymouth
[ -f "$T" ] && command -v plymouth >/dev/null 2>&1 || { echo "sem Plymouth: pulando"; exit 0; }
if update-alternatives --query default.plymouth >/dev/null 2>&1; then
  update-alternatives --install /usr/share/plymouth/themes/default.plymouth default.plymouth "$T" 150
  update-alternatives --set default.plymouth "$T"
elif command -v plymouth-set-default-theme >/dev/null 2>&1; then
  plymouth-set-default-theme nexus
fi
command -v update-initramfs >/dev/null 2>&1 && update-initramfs -u || true"#;

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
                    s.push(sh("Habilitar contrib e non-free (onde fica a Steam)", DEBIAN_COMPONENTS));
                }
                s.push(cmd("Atualizar a lista de pacotes", &["apt-get", "update"]));
                s.push(sh("Conferir se a Steam está disponível", STEAM_AVAILABLE));
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
    if !sys.has_browser {
        // streaming (Netflix, YouTube TV, Spotify…) abre num navegador em tela cheia
        let pkg = match (sys.family, sys.id.as_str()) {
            (Family::Debian, "ubuntu") => "chromium-browser",
            _ => "chromium",
        };
        if sys.family != Family::Unknown {
            s.push(step("Instalar o navegador para os serviços de streaming", sys.family, &[pkg]));
        }
    }
    if o.emulation {
        if !sys.has_flatpak {
            s.push(step("Instalar o Flatpak", sys.family, &["flatpak"]));
        }
        s.push(cmd("Adicionar o repositório Flathub", &["flatpak", "remote-add", "--if-not-exists", "flathub", "https://dl.flathub.org/repo/flathub.flatpakrepo"]));
        s.push(cmd("Instalar o RetroArch (Flatpak)", &["flatpak", "install", "--system", "-y", "flathub", "org.libretro.RetroArch"]));
        // fica na versão da máquina de desenvolvimento (deps.lock) e mascarado: só muda com uma release do Nexus
        s.push(cmd("Travar o RetroArch na versão do Nexus", &["/usr/lib/nexus/nexus-deps-apply"]));
    }
    if o.slim {
        if sys.family == Family::Debian {
            s.push(step("Instalar a swap comprimida na RAM (zram)", sys.family, &["systemd-zram-generator"]));
        }
        s.push(cmd("Deixar o sistema leve para jogos (desfazer: nexus-slim revert)", &["/usr/lib/nexus/nexus-slim", "apply"]));
    }
    // Base sem ambiente de desktop (Debian netinst): sem gerenciador de login, instala o greetd (leve, sem
    // interface própria) só para entrar direto na sessão Nexus.
    let mut dm = sys.display_manager.clone();
    if o.autologin_user.is_some() && dm.is_none() && sys.family == Family::Debian {
        s.push(step("Instalar o greetd (login direto no Nexus, sem desktop)", sys.family, &["greetd"]));
        s.push(cmd("Usar o greetd na inicialização", &["systemctl", "enable", "greetd.service"]));
        dm = Some("greetd".into());
    }
    // Modo console (entra direto no Nexus): a tela de boot também é a do Nexus, se o sistema usa Plymouth
    if o.autologin_user.is_some() && sys.family == Family::Debian {
        s.push(sh("Tela de boot do Nexus (Plymouth, se houver)", PLYMOUTH_THEME));
    }
    if let (Some(user), Some(dm)) = (&o.autologin_user, dm.as_deref().and_then(autologin::dm_from_name)) {
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
        System { id: "x".into(), id_like: vec![], version_id: "1".into(), pretty: "X".into(), arch: "x86_64".into(), family: f, supported: Support::Supported, gpu_vendor: "amd".into(), gpu: Default::default(), memory_mb: 8000, disk_free_mb: 50000, has_steam: false, has_flatpak: true, has_browser: true, display_manager: dm.map(String::from) }
    }

    #[test]
    fn debian_components_in_both_apt_formats() {
        let d = tempfile::tempdir().unwrap();
        let apt = d.path().join("etc/apt");
        std::fs::create_dir_all(apt.join("sources.list.d")).unwrap();
        // Debian 12: formato antigo, com e sem non-free-firmware; linha de outro repositório não muda
        std::fs::write(
            apt.join("sources.list"),
            "deb http://deb.debian.org/debian bookworm main non-free-firmware\ndeb-src http://deb.debian.org/debian bookworm main\ndeb http://security.debian.org/debian-security bookworm-security main contrib non-free\n# deb http://deb.debian.org/debian bookworm main\ndeb https://dl.google.com/linux/chrome/deb/ stable main\n",
        )
        .unwrap();
        // Debian 13: deb822
        std::fs::write(apt.join("sources.list.d/debian.sources"), "Types: deb\nURIs: http://deb.debian.org/debian\nSuites: trixie trixie-updates\nComponents: main non-free-firmware\nSigned-By: /usr/share/keyrings/debian-archive-keyring.gpg\n").unwrap();
        std::fs::write(apt.join("sources.list.d/outro.sources"), "Types: deb\nURIs: https://repo.example.com\nComponents: main\n").unwrap();
        for _ in 0..2 {
            // duas vezes: não duplica
            let st = Command::new("sh").arg("-c").arg(DEBIAN_COMPONENTS).env("ROOT", d.path()).status().unwrap();
            assert!(st.success());
        }
        let l = std::fs::read_to_string(apt.join("sources.list")).unwrap();
        let lines: Vec<&str> = l.lines().collect();
        assert_eq!(lines[0], "deb http://deb.debian.org/debian bookworm main non-free-firmware contrib non-free");
        assert_eq!(lines[1], "deb-src http://deb.debian.org/debian bookworm main contrib non-free");
        assert_eq!(lines[2], "deb http://security.debian.org/debian-security bookworm-security main contrib non-free", "já tinha tudo");
        assert!(lines[3].starts_with('#'), "comentário intacto");
        assert_eq!(lines[4], "deb https://dl.google.com/linux/chrome/deb/ stable main", "repositório de terceiros intacto");
        let s = std::fs::read_to_string(apt.join("sources.list.d/debian.sources")).unwrap();
        assert!(s.contains("Components: main non-free-firmware contrib non-free\n"), "{s}");
        assert_eq!(std::fs::read_to_string(apt.join("sources.list.d/outro.sources")).unwrap(), "Types: deb\nURIs: https://repo.example.com\nComponents: main\n");
    }

    #[test]
    fn steam_steps_order_on_debian() {
        let mut d = sys(Family::Debian, None);
        d.id = "debian".into();
        let t: Vec<String> = build(&d, &Options { steam: true, emulation: false, autologin_user: None, slim: false }).iter().map(|s| s.title.clone()).collect();
        let pos = |needle: &str| t.iter().position(|x| x.contains(needle)).unwrap_or_else(|| panic!("falta: {needle} em {t:?}"));
        assert!(pos("32 bits") < pos("contrib") && pos("contrib") < pos("Conferir se a Steam") && pos("Conferir se a Steam") < pos("Instalar a Steam"));
    }

    #[test]
    fn slim_and_greetd_on_a_bare_debian() {
        let mut d = sys(Family::Debian, None);
        d.id = "debian".into();
        let t: Vec<String> = build(&d, &Options { steam: false, emulation: false, autologin_user: Some("lia".into()), slim: true }).iter().map(|s| s.display()).collect();
        let pos = |needle: &str| t.iter().position(|x| x.contains(needle)).unwrap_or_else(|| panic!("falta: {needle} em {t:?}"));
        assert!(pos("systemd-zram-generator") < pos("nexus-slim apply"));
        assert!(pos("greetd") < pos("autologin"), "sem gerenciador de login: greetd e depois o autologin dele");
        assert!(pos("Plymouth") < pos("autologin"), "modo console troca a tela de boot");
        assert!(t.last().unwrap().contains("Greetd"), "{t:?}");
        // com desktop (gdm): não instala greetd
        let g = build(&sys(Family::Debian, Some("gdm3")), &Options { steam: false, emulation: false, autologin_user: Some("lia".into()), slim: false });
        assert!(!g.iter().any(|s| s.display().contains("greetd") || s.display().contains("nexus-slim")));
    }

    #[test]
    fn ubuntu_plan_with_steam_and_autologin() {
        let mut y = sys(Family::Debian, Some("sddm"));
        y.id = "ubuntu".into();
        let p = build(&y, &Options { steam: true, emulation: false, autologin_user: Some("lia".into()), slim: false });
        let t: Vec<String> = p.iter().map(|s| s.display()).collect();
        let pos = |needle: &str| t.iter().position(|x| x.contains(needle)).unwrap_or_else(|| panic!("falta: {needle} em {t:?}"));
        assert!(pos("dpkg --add-architecture i386") < pos("apt-get install -y steam-installer"));
        assert!(pos("add-apt-repository -y multiverse") < pos("apt-get install -y steam-installer"));
        assert!(t.last().unwrap().contains("autologin"));
        assert!(!t.iter().any(|x| x.contains("RetroArch")), "só entra o que foi pedido");
    }

    #[test]
    fn installs_browser_only_when_missing() {
        let mut y = sys(Family::Debian, None);
        y.id = "debian".into();
        let has = |b: &System| build(b, &Options { steam: false, emulation: false, autologin_user: None, slim: false }).iter().any(|s| s.display().contains("chromium"));
        assert!(!has(&y), "já tem navegador");
        y.has_browser = false;
        assert!(has(&y), "Raspberry Pi OS / Debian: chromium");
    }

    #[test]
    fn raspberry_pi_skips_steam() {
        let mut pi = sys(Family::Debian, Some("lightdm"));
        pi.id = "debian".into();
        pi.arch = "aarch64".into();
        let p = build(&pi, &Options { steam: true, emulation: true, autologin_user: Some("pi".into()), slim: false });
        let t: Vec<String> = p.iter().map(|s| s.display()).collect();
        assert!(!t.iter().any(|x| x.contains("i386") || x.contains("steam-installer")), "sem Steam nem 32 bits no ARM: {t:?}");
        assert!(t.iter().any(|x| x.contains("RetroArch")));
        assert!(t.last().unwrap().contains("autologin"), "LightDM do Raspberry Pi OS");
    }

    #[test]
    fn arch_fedora_and_emulation_plans() {
        let p = build(&sys(Family::Arch, None), &Options { steam: true, emulation: false, autologin_user: Some("lia".into()), slim: false });
        let t: Vec<String> = p.iter().map(|s| s.display()).collect();
        assert!(t.iter().any(|x| x.contains("multilib")));
        assert!(t.iter().any(|x| x == "pacman -S --needed --noconfirm steam"));
        assert!(!t.iter().any(|x| x.contains("autologin")), "sem gerenciador de login conhecido não mexe no login");
        let mut nf = sys(Family::Fedora, None);
        nf.has_flatpak = false;
        let f = build(&nf, &Options { steam: true, emulation: true, autologin_user: None, slim: false });
        let t: Vec<String> = f.iter().map(|s| s.display()).collect();
        assert!(t.iter().any(|x| x.contains("rpmfusion-nonfree-release")));
        let fl = t.iter().position(|x| x == "dnf install -y flatpak").unwrap();
        let hub = t.iter().position(|x| x.contains("remote-add")).unwrap();
        let ra = t.iter().position(|x| x.contains("org.libretro.RetroArch")).unwrap();
        assert!(fl < hub && hub < ra, "Flatpak, depois Flathub, depois RetroArch");
    }
}
