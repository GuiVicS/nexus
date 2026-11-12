//! Botão Guia dos controles físicos: leitura passiva (sem grab) via evdev. Com um jogo aberto,
//! a Shell não tem o foco do Gamepad API, então o Core traz o Nexus de volta à frente.
use crate::core::Core;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BTN_MODE: u16 = 0x13c;
const BTN_SELECT: u16 = 0x13a;
const BTN_START: u16 = 0x13b;

/// Quanto tempo segurar Guia (ou Select+Start) para a saída de emergência (`NEXUS_EMERGENCY_MS`, padrão 3 s).
pub fn hold_ms() -> u64 {
    std::env::var("NEXUS_EMERGENCY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(3000)
}

/// Geração do "segurando": soltar o botão invalida o cronômetro que está esperando.
static HOLD_GEN: AtomicU64 = AtomicU64::new(0);

/// Guia (ou Select+Start) apertado (`down`) ou solto. Segurar por 3 s com um jogo aberto pede para fechá-lo à força.
pub fn hold(core: &Arc<Core>, rt: &tokio::runtime::Handle, down: bool) {
    let gen = HOLD_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    if !down {
        return;
    }
    let core = core.clone();
    rt.spawn(async move {
        tokio::time::sleep(Duration::from_millis(hold_ms())).await;
        if HOLD_GEN.load(Ordering::SeqCst) == gen && core.game_info().is_some() {
            emergency(&core);
        }
    });
}

/// Saída de emergência: traz o Nexus para a frente (se ainda não estiver) e pede a confirmação na Shell.
/// Quem fecha o jogo é a Shell, depois do "sim" (`game.close` com `force`, que mata o escopo inteiro).
pub fn emergency(core: &Core) {
    if !core.guide_open.load(Ordering::SeqCst) {
        pressed(core);
    }
    let title = core.game_info().map(|g| g.title).unwrap_or_default();
    core.emit("input.emergency", serde_json::json!({ "title": title }));
}

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
                    let (mut sel, mut start) = (false, false);
                    while let Ok(evs) = dev.fetch_events() {
                        for ev in evs {
                            if ev.event_type() != evdev::EventType::KEY {
                                continue;
                            }
                            let down = ev.value() != 0;
                            let pair_before = sel && start;
                            let fire = match ev.code() {
                                BTN_MODE => {
                                    // segurar Guia por 3 s = saída de emergência (ignora o auto-repeat, valor 2)
                                    if ev.value() == 1 { hold(&core, &rt, true) } else if ev.value() == 0 { hold(&core, &rt, false) }
                                    ev.value() == 1
                                }
                                // Select+Start juntos = Guia (controles sem botão Guia): dispara ao completar o par
                                BTN_SELECT => { let f = down && !sel && start; sel = down; f }
                                BTN_START => { let f = down && !start && sel; start = down; f }
                                _ => false,
                            };
                            if ev.code() != BTN_MODE && pair_before != (sel && start) {
                                hold(&core, &rt, sel && start);
                            }
                            if fire {
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

/// Menu rápido (botão Guia, Select+Start, F1, tecla Super ou celular) — abre e fecha com a mesma tecla, como num console:
/// - Steam aberta a pedido (login/compra): esconde e volta ao Nexus;
/// - jogo/app na tela: guarda um quadro dele (fundo desfocado do menu), traz a Shell e abre o menu;
/// - menu aberto por cima do jogo: fecha e volta ao jogo;
/// - no Nexus sem jogo (ou no Início com o jogo em segundo plano): abre/fecha a Central.
pub fn pressed(core: &Core) {
    use std::sync::atomic::Ordering;
    if core.steam_ui.load(Ordering::SeqCst) {
        core.hide_steam_ui();
        return;
    }
    let sway = core.compositor.name() == "sway";
    if core.game_info().is_none() {
        core.guide_open.store(false, Ordering::SeqCst);
        core.emit("input.guide", serde_json::json!({ "toggle": true }));
        return;
    }
    if core.guide_open.swap(false, Ordering::SeqCst) {
        core.compositor.show_game();
        core.emit("input.guide", serde_json::json!({ "close": true }));
        return;
    }
    if sway && !crate::compositor::games_visible() {
        // Nexus na frente, jogo em segundo plano: só alterna a Central (sem novo quadro)
        core.emit("input.guide", serde_json::json!({ "toggle": true }));
        return;
    }
    let t0 = std::time::Instant::now();
    // PNG: o grim do Debian/Ubuntu vem sem suporte a JPEG
    let frame = core.cfg.runtime_dir.join("frame.png");
    // o grim trava com a tela bloqueada/protetor de tela: no máximo 1,5 s, senão o menu abre sem o quadro do jogo
    let ok = sway && crate::util::run_with_timeout(std::process::Command::new("grim").args(["-l", "1"]).arg(&frame), std::time::Duration::from_millis(1500));
    core.guide_open.store(true, Ordering::SeqCst);
    core.compositor.show_shell();
    let ms = t0.elapsed().as_millis() as u64;
    tracing::info!("menu rápido sobre o jogo: quadro + Shell na frente em {ms} ms");
    core.emit("input.guide", serde_json::json!({ "frame": if ok { Some(nexus_protocol::now_unix()) } else { None }, "ms": ms }));
}
