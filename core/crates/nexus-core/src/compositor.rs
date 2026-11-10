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

/// Foca uma janela pelo id do sway.
pub fn focus_con(id: i64) {
    sway_cmd(&format!("[con_id={id}] focus"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn unity_title() {
        assert!(super::is_unity_config_title("AdVenture Capitalist Configuration"));
        assert!(!super::is_unity_config_title("Configuration"));
        assert!(!super::is_unity_config_title("Steam"));
        assert_eq!(super::unity_play_point([0, 0, 1280, 688]), (1072, 665), "ponto validado no AdVenture Capitalist");
        assert_eq!(super::unity_play_point([100, 50, 200, 30]), (200, 65), "janela pequena: não sai dela");
    }
}
