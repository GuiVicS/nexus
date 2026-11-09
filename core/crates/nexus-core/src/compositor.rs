//! Controle do compositor: mostrar a Shell ou o jogo. `none` não faz nada (desenvolvimento/testes).
use std::process::Command;

pub trait Compositor: Send + Sync {
    fn show_shell(&self);
    fn show_game(&self);
    fn name(&self) -> &'static str;
}

pub struct Null;
impl Compositor for Null {
    fn show_shell(&self) {}
    fn show_game(&self) {}
    fn name(&self) -> &'static str {
        "none"
    }
}

/// sway: workspace 1 = Shell, workspace 2 = jogos (ver system/sway/nexus-kiosk.conf)
pub struct Sway;
impl Sway {
    fn ws(n: u8) {
        let _ = Command::new("swaymsg").args(["workspace", "number", &n.to_string()]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    }
}
impl Compositor for Sway {
    fn show_shell(&self) {
        Self::ws(1)
    }
    fn show_game(&self) {
        Self::ws(2)
    }
    fn name(&self) -> &'static str {
        "sway"
    }
}

pub fn from_name(name: &str) -> Box<dyn Compositor> {
    match name {
        "sway" => Box::new(Sway),
        _ => Box::new(Null),
    }
}
