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
        crate::freeze::thaw(); // antes de aparecer: a Shell volta a desenhar já no primeiro quadro
        Self::ws(1)
    }
    fn show_game(&self) {
        Self::ws(2);
        crate::freeze::freeze_later(crate::freeze::FREEZE_DELAY);
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

/// Comando avulso no sway (ignora erros: fora do sway não faz nada).
pub fn sway_cmd(cmd: &str) {
    let _ = Command::new("swaymsg").arg(cmd).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

/// Diálogos X11 flutuantes (ex.: "Install" da Steam) ficam atrás da janela principal em tela cheia: traz à frente.
pub fn raise_dialogs() {
    sway_cmd(r#"[floating shell="xwayland" workspace="2"] fullscreen enable, focus"#);
}

/// Janelas abertas no sway: (app_id ou classe X11, título). Vazio fora do sway.
pub fn sway_windows() -> Vec<(String, String)> {
    let Ok(out) = Command::new("swaymsg").args(["-t", "get_tree", "-r"]).stderr(std::process::Stdio::null()).output() else { return vec![] };
    let Ok(tree) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else { return vec![] };
    let mut v = vec![];
    fn walk(n: &serde_json::Value, v: &mut Vec<(String, String)>) {
        let id = n["app_id"].as_str().or_else(|| n["window_properties"]["class"].as_str()).unwrap_or("");
        if !id.is_empty() {
            v.push((id.to_string(), n["name"].as_str().unwrap_or("").to_string()));
        }
        for k in ["nodes", "floating_nodes"] {
            if let Some(a) = n[k].as_array() {
                a.iter().for_each(|c| walk(c, v));
            }
        }
    }
    walk(&tree, &mut v);
    v
}


/// Janela "<Jogo> Configuration" do Unity (resolução/qualidade antes de abrir o jogo): só tem mouse, então na TV
/// travava o jogo. Devolve o id da janela no sway e o retângulo (x, y, largura, altura), se houver uma.
pub fn unity_config_window() -> Option<(i64, [i64; 4])> {
    let out = Command::new("swaymsg").args(["-t", "get_tree", "-r"]).stderr(std::process::Stdio::null()).output().ok()?;
    let tree = serde_json::from_slice::<serde_json::Value>(&out.stdout).ok()?;
    fn walk(n: &serde_json::Value) -> Option<(i64, [i64; 4])> {
        let name = n["name"].as_str().unwrap_or("");
        let is_win = n["app_id"].is_string() || n["window_properties"].is_object();
        let ours = n["app_id"].as_str().is_some_and(|a| a.starts_with("nexus")) || n["window_properties"]["class"].as_str().is_some_and(|c| c.starts_with("nexus"));
        if is_win && !ours && is_unity_config_title(name) {
            let r = &n["rect"];
            return Some((n["id"].as_i64()?, [r["x"].as_i64()?, r["y"].as_i64()?, r["width"].as_i64()?, r["height"].as_i64()?]));
        }
        ["nodes", "floating_nodes"].iter().filter_map(|k| n[*k].as_array()).flatten().find_map(walk)
    }
    walk(&tree)
}

/// Botão "Play!" do diálogo do Unity: fica no canto inferior direito, à esquerda do "Quit" (medido no
/// AdVenture Capitalist com a janela em tela cheia; o GTK mantém os botões presos ao canto ao esticar).
pub fn unity_play_point(rect: [i64; 4]) -> (i64, i64) {
    let [x, y, w, h] = rect;
    (x + (w - 208).max(w / 2), y + (h - 23).max(h / 2))
}

/// Clique pelo próprio sway (cursor do seat): não depende de /dev/uinput e funciona em qualquer sessão.
pub fn click_at(x: i64, y: i64) {
    sway_cmd(&format!("seat - cursor set {x} {y}"));
    sway_cmd("seat - cursor press button1");
    sway_cmd("seat - cursor release button1");
}

/// sway rodando dentro de outra sessão (teste: saídas "WL-1"/"X11-1"). Aí o teclado virtual (uinput) vai para
/// a sessão de fora, não para o jogo — então não se usa.
pub fn sway_nested() -> bool {
    let Ok(out) = Command::new("swaymsg").args(["-t", "get_outputs", "-r"]).stderr(std::process::Stdio::null()).output() else { return true };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else { return true };
    let names: Vec<&str> = v.as_array().map(|a| a.iter().filter_map(|o| o["name"].as_str()).collect()).unwrap_or_default();
    names.is_empty() || names.iter().all(|n| n.starts_with("WL-") || n.starts_with("X11-") || n.starts_with("HEADLESS-"))
}

pub fn is_unity_config_title(t: &str) -> bool {
    t.len() > " Configuration".len() && t.ends_with(" Configuration")
}

/// Diálogos da Steam que aparecem ao instalar: "Install" (540×480 a 100%) e, em alguns jogos, o contrato de
/// licença "EULA" (640×600). Janelas X11 flutuantes da Steam; o título muda com o idioma, então vale também o
/// formato (proporção) da janela.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SteamDialog {
    Install,
    Eula,
}

impl SteamDialog {
    /// (largura de referência, distância do botão de confirmar à direita, do de cancelar à direita) — botões presos
    /// ao canto inferior direito, 43 px acima da borda (medidos na VM).
    fn layout(self) -> (f64, f64, f64) {
        match self {
            SteamDialog::Install => (540.0, 330.0, 116.0),
            SteamDialog::Eula => (640.0, 219.0, 85.0),
        }
    }
}

pub fn classify_steam_dialog(title: &str, w: i64, h: i64) -> Option<SteamDialog> {
    const INSTALL: [&str; 6] = ["Install", "Instalar", "Installieren", "Installer", "Installa", "Установить"];
    if INSTALL.contains(&title) {
        return Some(SteamDialog::Install);
    }
    if title == "EULA" || title.starts_with("Contrato") || title.starts_with("License") {
        return Some(SteamDialog::Eula);
    }
    if title == "Steam" || w < 500 || h < 440 {
        return None;
    }
    // mesmo formato em qualquer zoom: Install 540×480 (1,125), EULA 640×600 (1,067)
    let near = |rw: f64, rh: f64| ((w as f64 / rw) - (h as f64 / rh)).abs() < 0.03;
    if near(540.0, 480.0) {
        Some(SteamDialog::Install)
    } else if near(640.0, 600.0) {
        Some(SteamDialog::Eula)
    } else {
        None
    }
}

/// Primeiro diálogo de instalação da Steam aberto (visível ou no scratchpad): tipo, id e retângulo.
pub fn steam_dialog() -> Option<(SteamDialog, i64, [i64; 4])> {
    let tree = sway_tree()?;
    fn walk(n: &serde_json::Value, floating: bool) -> Option<(SteamDialog, i64, [i64; 4])> {
        let class = n["window_properties"]["class"].as_str().unwrap_or("");
        if floating && is_steam_client_class(class) {
            let r = &n["rect"];
            let rect = [r["x"].as_i64()?, r["y"].as_i64()?, r["width"].as_i64()?, r["height"].as_i64()?];
            if let Some(k) = classify_steam_dialog(n["name"].as_str().unwrap_or(""), rect[2], rect[3]) {
                return Some((k, n["id"].as_i64()?, rect));
            }
        }
        if let Some(f) = n["nodes"].as_array().and_then(|a| a.iter().find_map(|c| walk(c, false))) {
            return Some(f);
        }
        n["floating_nodes"].as_array()?.iter().find_map(|c| walk(c, true))
    }
    walk(&tree, false)
}

/// Pontos (confirmar, cancelar) do diálogo, escalados se a Steam estiver com zoom.
pub fn steam_dialog_points(kind: SteamDialog, rect: [i64; 4]) -> ((i64, i64), (i64, i64)) {
    let [x, y, w, h] = rect;
    let (base, ok, cancel) = kind.layout();
    let k = w as f64 / base;
    let px = |dx: f64| x + w - (dx * k).round() as i64;
    let py = y + h - (43.0 * k).round() as i64;
    ((px(ok), py), (px(cancel), py))
}

fn sway_tree() -> Option<serde_json::Value> {
    let out = Command::new("swaymsg").args(["-t", "get_tree", "-r"]).stderr(std::process::Stdio::null()).output().ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

pub fn is_steam_client_class(c: &str) -> bool {
    c.eq_ignore_ascii_case("steam")
}

/// Janela do Nexus (Shell, overlay) ou do cliente da Steam: nada disso é "o jogo".
fn is_ours_or_steam(app_id: &str, class: &str) -> bool {
    app_id.starts_with("nexus") || class.starts_with("nexus") || is_steam_client_class(class)
}

/// (id, rótulo, está no scratchpad) de cada janela.
fn windows_full() -> Vec<(i64, String, String, bool)> {
    let Some(tree) = sway_tree() else { return vec![] };
    let mut v = vec![];
    fn walk(n: &serde_json::Value, scratch: bool, v: &mut Vec<(i64, String, String, bool)>) {
        let scratch = scratch || n["name"].as_str() == Some("__i3_scratch");
        let app = n["app_id"].as_str().unwrap_or("");
        let class = n["window_properties"]["class"].as_str().unwrap_or("");
        if (!app.is_empty() || !class.is_empty()) && n["id"].is_i64() {
            v.push((n["id"].as_i64().unwrap_or(0), app.to_string(), class.to_string(), scratch));
        }
        for k in ["nodes", "floating_nodes"] {
            if let Some(a) = n[k].as_array() {
                a.iter().for_each(|c| walk(c, scratch, v));
            }
        }
    }
    walk(&tree, false, &mut v);
    v
}

/// Há uma janela de jogo/app (nem do Nexus nem do cliente da Steam) fora do scratchpad.
pub fn game_window_present() -> bool {
    windows_full().iter().any(|(_, app, class, scratch)| !scratch && !is_ours_or_steam(app, class))
}

/// Esconde todas as janelas do cliente da Steam (scratchpad = invisível).
pub fn steam_hide_all() {
    for (id, _, class, scratch) in windows_full() {
        if is_steam_client_class(&class) && !scratch {
            sway_cmd(&format!("[con_id={id}] move scratchpad"));
        }
    }
}

/// Mostra as janelas do cliente da Steam no workspace dos jogos (só quando o usuário pediu a Steam: login em
/// modo TV, página de compra). A principal em tela cheia; diálogos flutuando por cima.
pub fn steam_show_all() {
    for (id, _, class, scratch) in windows_full() {
        if is_steam_client_class(&class) && scratch {
            sway_cmd(&format!("[con_id={id}] move to workspace number 2"));
            sway_cmd(&format!("[con_id={id}] floating disable, fullscreen enable"));
        }
    }
}

/// Tira uma janela do scratchpad para o workspace dos jogos, flutuando e focada (ex.: diálogo "Install").
pub fn peek_window(id: i64) {
    sway_cmd(&format!("[con_id={id}] move to workspace number 2"));
    sway_cmd(&format!("[con_id={id}] floating enable, fullscreen disable, move position center, focus"));
}

/// Devolve uma janela ao scratchpad.
pub fn hide_window(id: i64) {
    sway_cmd(&format!("[con_id={id}] move scratchpad"));
}

/// O workspace dos jogos (2) está na tela.
pub fn games_visible() -> bool {
    let Ok(out) = Command::new("swaymsg").args(["-t", "get_workspaces", "-r"]).stderr(std::process::Stdio::null()).output() else { return false };
    serde_json::from_slice::<serde_json::Value>(&out.stdout).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().any(|w| w["visible"].as_bool() == Some(true) && w["num"].as_i64() == Some(2))
}

/// Foca uma janela pelo id do sway.
pub fn focus_con(id: i64) {
    sway_cmd(&format!("[con_id={id}] focus"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn steam_dialogs() {
        use super::{classify_steam_dialog as c, steam_dialog_points as p, SteamDialog::*};
        // medido na VM: Install em (347,104) 540×480 → Install (557,541), Cancel (771,541);
        // EULA em (297,44) 640×600 → Accept (718,601), Cancel (852,601)
        assert_eq!(p(Install, [347, 104, 540, 480]), ((557, 541), (771, 541)));
        assert_eq!(p(Eula, [297, 44, 640, 600]), ((718, 601), (852, 601)));
        assert_eq!(c("Install", 540, 480), Some(Install));
        assert_eq!(c("Instalar", 810, 720), Some(Install));
        assert_eq!(c("Instalar Team Fortress", 540, 480), Some(Install));
        assert_eq!(c("EULA", 640, 600), Some(Eula));
        assert_eq!(c("Licença", 960, 900), Some(Eula));
        assert_eq!(c("Steam", 540, 480), None);
        assert_eq!(c("Friends List", 300, 600), None);
        assert_eq!(c("Launching...", 600, 286), None);
    }
    #[test]
    fn unity_title() {
        assert!(super::is_unity_config_title("AdVenture Capitalist Configuration"));
        assert!(!super::is_unity_config_title("Configuration"));
        assert!(!super::is_unity_config_title("Steam"));
        assert_eq!(super::unity_play_point([0, 0, 1280, 688]), (1072, 665), "ponto validado no AdVenture Capitalist");
        assert_eq!(super::unity_play_point([100, 50, 200, 30]), (200, 65), "janela pequena: não sai dela");
    }
}
