//! Botão Guia dos controles físicos: leitura passiva (sem grab) via evdev. Com um jogo aberto,
//! a Shell não tem o foco do Gamepad API, então o Core traz o Nexus de volta à frente.
use crate::core::Core;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BTN_MODE: u16 = 0x13c;

pub fn spawn(core: Arc<Core>) {
    let rt = tokio::runtime::Handle::current();
    let watched: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
    std::thread::spawn(move || loop {
        if let Ok(rd) = std::fs::read_dir("/dev/input") {
            for e in rd.flatten() {
                let p = e.path();
                if !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("event")) || watched.lock().unwrap().contains(&p) {
                    continue;
                }
                let Ok(dev) = evdev::Device::open(&p) else { continue };
                if !dev.supported_keys().is_some_and(|k| k.contains(evdev::KeyCode::new(BTN_MODE))) {
                    continue;
                }
                watched.lock().unwrap().insert(p.clone());
                let (core, rt, watched) = (core.clone(), rt.clone(), watched.clone());
                std::thread::spawn(move || {
                    let mut dev = dev;
                    while let Ok(evs) = dev.fetch_events() {
                        for ev in evs {
                            if ev.event_type() == evdev::EventType::KEY && ev.code() == BTN_MODE && ev.value() == 1 {
                                let c = core.clone();
                                rt.spawn(async move { pressed(&c) });
                            }
                        }
                    }
                    watched.lock().unwrap().remove(&p);
                });
            }
        }
        std::thread::sleep(Duration::from_secs(3));
    });
}

/// Botão Guia (controle, tecla Super ou celular) com um jogo/app aberto: guarda um quadro da tela do jogo
/// (fundo da Central, para ela parecer aberta por cima do jogo), traz a Shell e abre a Central.
pub fn pressed(core: &Core) {
    if core.game_info().is_none() {
        return;
    }
    // PNG: o grim do Debian/Ubuntu vem sem suporte a JPEG
    let frame = core.cfg.runtime_dir.join("frame.png");
    let ok = core.compositor.name() == "sway"
        && std::process::Command::new("grim")
            .args(["-l", "1"])
            .arg(&frame)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
    core.compositor.show_shell();
    core.emit("input.guide", serde_json::json!({ "frame": if ok { Some(nexus_protocol::now_unix()) } else { None } }));
}
