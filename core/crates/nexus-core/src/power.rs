//! Energia (B10): apagar a tela e suspender por inatividade (`swayidle`, só quando não há jogo aberto — controle
//! analógico não conta como atividade para o sway, então com jogo o temporizador fica desligado), o que o botão
//! físico de energia faz (o Core segura um "inhibitor" do logind e lê o botão pelo evdev) e o estado do autologin.
//! `NEXUS_SWAYIDLE`, `NEXUS_INHIBIT`, `NEXUS_ETC_DIR` e `NEXUS_SWAYMSG` trocam programas/pastas (testes).
use crate::core::Core;
use crate::db::Db;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

pub const SCREEN_OPTS: [u32; 5] = [0, 5, 10, 15, 30];
pub const SUSPEND_OPTS: [u32; 5] = [0, 15, 30, 60, 120];
pub const KEYS: [&str; 4] = ["ask", "suspend", "off", "nothing"];

#[derive(Debug, Clone, PartialEq)]
pub struct Prefs {
    pub screen_min: u32,
    pub suspend_min: u32,
    pub key: String,
}

pub fn prefs(db: &Db) -> Prefs {
    let num = |k: &str, def: u32, opts: &[u32]| db.kv_get(k).and_then(|v| v.parse().ok()).filter(|v| opts.contains(v)).unwrap_or(def);
    Prefs {
        screen_min: num("power_screen_min", 15, &SCREEN_OPTS),
        suspend_min: num("power_suspend_min", 0, &SUSPEND_OPTS),
        key: db.kv_get("power_key").filter(|k| KEYS.contains(&k.as_str())).unwrap_or_else(|| "ask".into()),
    }
}

/// Argumentos do swayidle (vazio = nada a fazer).
pub fn idle_args(p: &Prefs, swaymsg: &str) -> Vec<String> {
    let mut a: Vec<String> = vec![];
    if p.screen_min > 0 {
        a.extend(["timeout".into(), (p.screen_min * 60).to_string(), format!("{swaymsg} 'output * power off'"), "resume".into(), format!("{swaymsg} 'output * power on'")]);
    }
    if p.suspend_min > 0 {
        a.extend(["timeout".into(), (p.suspend_min * 60).to_string(), "systemctl suspend".into()]);
    }
    if !a.is_empty() {
        a.insert(0, "-w".into());
    }
    a
}

fn etc() -> PathBuf {
    PathBuf::from(std::env::var("NEXUS_ETC_DIR").unwrap_or_else(|_| "/etc".into()))
}

/// O aparelho abre direto no Nexus (autologin configurado pelo instalador)? Só leitura: desligar mexe em /etc (B15).
pub fn autologin(root: &Path) -> Value {
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap_or_default();
    let on = |t: &str, key: &str| t.lines().any(|l| { let l = l.trim(); !l.starts_with('#') && !l.starts_with(';') && l.starts_with(key) && l.split_once('=').map_or(false, |(_, v)| !v.trim().is_empty() && v.trim() != "false") });
    let gdm = ["gdm3/custom.conf", "gdm/custom.conf"].iter().map(|f| read(f)).any(|t| on(&t, "AutomaticLogin"));
    let sddm = std::fs::read_dir(root.join("sddm.conf.d")).map(|d| d.flatten().any(|e| on(&std::fs::read_to_string(e.path()).unwrap_or_default(), "User"))).unwrap_or(false) || on(&read("sddm.conf"), "User");
    let lightdm = on(&read("lightdm/lightdm.conf"), "autologin-user") || std::fs::read_dir(root.join("lightdm/lightdm.conf.d")).map(|d| d.flatten().any(|e| on(&std::fs::read_to_string(e.path()).unwrap_or_default(), "autologin-user"))).unwrap_or(false);
    let greetd = read("greetd/config.toml").contains("initial_session");
    let (enabled, dm) = if gdm { (true, "gdm") } else if sddm { (true, "sddm") } else if lightdm { (true, "lightdm") } else if greetd { (true, "greetd") } else { (false, "") };
    json!({ "enabled": enabled, "dm": dm })
}

pub fn status(db: &Db) -> Value {
    let p = prefs(db);
    let idle_ok = std::env::var("NEXUS_SWAYIDLE").is_ok() || which("swayidle");
    json!({ "screenMin": p.screen_min, "suspendMin": p.suspend_min, "key": p.key, "idleAvailable": idle_ok, "autologin": autologin(&etc()),
        "screenOpts": SCREEN_OPTS, "suspendOpts": SUSPEND_OPTS })
}

fn which(bin: &str) -> bool {
    std::env::var("PATH").unwrap_or_default().split(':').any(|d| Path::new(d).join(bin).is_file())
}

fn spawn_child(mut cmd: Command) -> Option<Child> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // morre junto com o Core
    unsafe {
        std::os::unix::process::CommandExt::pre_exec(&mut cmd, || {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    cmd.spawn().ok()
}

/// Mantém o swayidle de acordo com as preferências e com o jogo (sem jogo aberto).
pub fn spawn_idle(core: Arc<Core>) {
    let explicit = std::env::var("NEXUS_SWAYIDLE").ok();
    if core.cfg.compositor != "sway" && explicit.is_none() {
        return;
    }
    let bin = explicit.unwrap_or_else(|| "swayidle".into());
    let swaymsg = std::env::var("NEXUS_SWAYMSG").unwrap_or_else(|_| "swaymsg".into());
    std::thread::spawn(move || {
        let mut cur: Option<(Vec<String>, Child)> = None;
        loop {
            let want = if core.game_info().is_some() { vec![] } else { idle_args(&prefs(&core.db), &swaymsg) };
            if cur.as_ref().map(|c| &c.0) != Some(&want) && !(cur.is_none() && want.is_empty()) {
                if let Some((_, mut c)) = cur.take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                if !want.is_empty() {
                    let mut cmd = Command::new(&bin);
                    cmd.args(&want);
                    cur = spawn_child(cmd).map(|c| (want, c));
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

/// O botão de energia foi apertado: faz o que o usuário escolheu.
pub fn button_pressed(core: &Core) {
    match prefs(&core.db).key.as_str() {
        "suspend" => do_power(core, "suspend"),
        "off" => do_power(core, "off"),
        "nothing" => {}
        _ => core.emit("power.button", json!({ "game": core.game_info().map(|g| g.title) })),
    }
}

pub fn do_power(core: &Core, a: &str) {
    let cmd = match a {
        "off" => "poweroff",
        "restart" => "reboot",
        _ => "suspend",
    };
    if core.cfg.dev {
        core.emit("system.power", json!({ "action": a, "simulated": true }));
        return;
    }
    let _ = Command::new("systemctl").arg(cmd).spawn();
}

/// Lê o botão físico (evdev, KEY_POWER) e segura o inhibitor do logind para ele não desligar na hora.
pub fn watch_key(core: Arc<Core>) {
    std::thread::spawn(move || {
        let mut inhibitor: Option<Child> = None;
        let mut watched: std::collections::HashSet<PathBuf> = Default::default();
        loop {
            if let Ok(rd) = std::fs::read_dir("/dev/input") {
                for e in rd.flatten() {
                    let p = e.path();
                    if !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("event")) || watched.contains(&p) {
                        continue;
                    }
                    let Ok(dev) = evdev::Device::open(&p) else { continue };
                    let is_power = dev.name().is_some_and(|n| n.contains("Power Button")) && dev.supported_keys().is_some_and(|k| k.contains(evdev::KeyCode::KEY_POWER));
                    if !is_power {
                        continue;
                    }
                    watched.insert(p.clone());
                    if inhibitor.is_none() {
                        let mut c = Command::new(std::env::var("NEXUS_INHIBIT").unwrap_or_else(|_| "systemd-inhibit".into()));
                        c.args(["--what=handle-power-key", "--who=Nexus", "--why=O Nexus decide o que o botão de energia faz", "--mode=block", "sleep", "infinity"]);
                        inhibitor = spawn_child(c);
                    }
                    let core = core.clone();
                    std::thread::spawn(move || {
                        let mut dev = dev;
                        while let Ok(evs) = dev.fetch_events() {
                            for ev in evs {
                                if ev.event_type() == evdev::EventType::KEY && ev.code() == evdev::KeyCode::KEY_POWER.code() && ev.value() == 1 {
                                    button_pressed(&core);
                                }
                            }
                        }
                    });
                }
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argumentos_do_swayidle() {
        let p = |s, u| Prefs { screen_min: s, suspend_min: u, key: "ask".into() };
        assert!(idle_args(&p(0, 0), "swaymsg").is_empty());
        let a = idle_args(&p(10, 0), "swaymsg");
        assert_eq!(a[..3], ["-w", "timeout", "600"]);
        assert!(a.contains(&"resume".to_string()) && a.iter().any(|x| x.contains("output * power on")));
        let b = idle_args(&p(0, 30), "swaymsg");
        assert_eq!(b, ["-w", "timeout", "1800", "systemctl suspend"]);
        assert_eq!(idle_args(&p(5, 15), "swaymsg").iter().filter(|x| *x == "timeout").count(), 2);
    }

    #[test]
    fn autologin_lido_dos_arquivos() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(autologin(d.path())["enabled"], false);
        std::fs::create_dir_all(d.path().join("gdm3")).unwrap();
        std::fs::write(d.path().join("gdm3/custom.conf"), "[daemon]\n#AutomaticLogin=x\nAutomaticLoginEnable=true\nAutomaticLogin=lia\n").unwrap();
        let a = autologin(d.path());
        assert_eq!((a["enabled"].as_bool(), a["dm"].as_str()), (Some(true), Some("gdm")));
        let d2 = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d2.path().join("sddm.conf.d")).unwrap();
        std::fs::write(d2.path().join("sddm.conf.d/10-nexus-autologin.conf"), "[Autologin]\nUser=lia\n").unwrap();
        assert_eq!(autologin(d2.path())["dm"], "sddm");
    }
}
