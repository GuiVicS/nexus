//! Tela e vídeo pelo sway (`swaymsg`, argv fixo e valores validados): resolução/Hz, escala, rotação, VRR e HDR
//! (quando o compositor e a tela informam), mais a margem para TVs que cortam as bordas (aplicada pela Shell).
//!
//! Segurança contra tela preta: mudanças arriscadas ficam **pendentes**; o Core aplica, guarda a configuração
//! anterior e volta sozinho se ninguém confirmar em `NEXUS_DISPLAY_CONFIRM_SECS` (15 s). Só a configuração
//! confirmada é salva (por tela, pelo identificador marca/modelo/série) e reaplicada ao iniciar.
//! `NEXUS_SWAYMSG` troca o binário (testes).
use crate::db::Db;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use std::sync::Mutex;

fn swaymsg(args: &[&str]) -> Result<String, String> {
    let bin = std::env::var("NEXUS_SWAYMSG").unwrap_or_else(|_| "swaymsg".into());
    let out = Command::new("timeout").arg("5").arg(bin).args(args).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    // swaymsg devolve [{"success":false,"error":"…"}] com status 2 quando o comando é recusado
    if out.status.success() && !s.contains("\"success\": false") && !s.contains("\"success\":false") {
        Ok(s)
    } else {
        Err("A tela recusou essa configuração".into())
    }
}

pub fn available() -> bool {
    std::env::var_os("NEXUS_SWAYMSG").is_some() || std::env::var_os("SWAYSOCK").is_some()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Settings {
    /// "1920x1080@60.000"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vrr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hdr: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub name: String,
    pub ident: String,
    pub label: String,
    pub mode: Option<(u32, u32, u32)>, // largura, altura, mHz
    pub modes: Vec<(u32, u32, u32)>,
    pub scale: f64,
    pub transform: String,
    pub vrr: Option<bool>,
    pub hdr: Option<bool>,
}

pub fn parse_outputs(js: &str) -> Vec<Output> {
    let v: Value = serde_json::from_str(js).unwrap_or(Value::Null);
    let m = |x: &Value| Some((x["width"].as_u64()? as u32, x["height"].as_u64()? as u32, x["refresh"].as_u64().unwrap_or(0) as u32));
    v.as_array()
        .map(|a| {
            a.iter()
                .filter(|o| o["active"].as_bool().unwrap_or(false))
                .map(|o| {
                    let s = |k: &str| o[k].as_str().unwrap_or("").to_string();
                    let (make, model, serial) = (s("make"), s("model"), s("serial"));
                    let mut modes: Vec<(u32, u32, u32)> = o["modes"].as_array().map(|ms| ms.iter().filter_map(m).collect()).unwrap_or_default();
                    modes.sort_by(|a, b| (b.0 * b.1, b.2).cmp(&(a.0 * a.1, a.2)));
                    modes.dedup();
                    let known = |x: &str| !x.is_empty() && x != "Unknown";
                    let label = match (known(&make), known(&model)) {
                        (true, true) => format!("{make} {model}"),
                        (_, true) => model.clone(),
                        _ => s("name"),
                    };
                    Output {
                        name: s("name"),
                        ident: format!("{make} {model} {serial}"),
                        label,
                        mode: m(&o["current_mode"]),
                        modes,
                        scale: o["scale"].as_f64().unwrap_or(1.0),
                        transform: o["transform"].as_str().unwrap_or("normal").to_string(),
                        vrr: o["adaptive_sync_status"].as_str().map(|x| x == "enabled"),
                        hdr: o["hdr"].as_bool(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn outputs() -> Vec<Output> {
    swaymsg(&["-t", "get_outputs", "-r"]).map(|j| parse_outputs(&j)).unwrap_or_default()
}

const TRANSFORMS: &[&str] = &["normal", "90", "180", "270"];

pub fn valid_mode(m: &str) -> bool {
    let Some((wh, hz)) = m.split_once('@') else { return false };
    let Some((w, h)) = wh.split_once('x') else { return false };
    w.parse::<u32>().is_ok_and(|w| (320..=16384).contains(&w)) && h.parse::<u32>().is_ok_and(|h| (200..=16384).contains(&h)) && hz.parse::<f64>().is_ok_and(|r| (1.0..=1000.0).contains(&r))
}

/// Configuração atual de uma saída, no formato de `Settings` (para poder voltar a ela).
pub fn current(o: &Output) -> Settings {
    Settings {
        mode: o.mode.filter(|m| m.2 > 0).map(|(w, h, r)| format!("{w}x{h}@{:.3}", r as f64 / 1000.0)),
        scale: Some(o.scale),
        transform: Some(o.transform.clone()),
        vrr: o.vrr,
        hdr: o.hdr,
    }
}

pub fn validate(s: &Settings) -> Result<(), String> {
    if s.mode.as_deref().is_some_and(|m| !valid_mode(m)) {
        return Err("Resolução inválida".into());
    }
    if s.scale.is_some_and(|x| !(0.5..=3.0).contains(&x)) {
        return Err("Escala inválida".into());
    }
    if s.transform.as_deref().is_some_and(|t| !TRANSFORMS.contains(&t)) {
        return Err("Rotação inválida".into());
    }
    Ok(())
}

/// Aplica no sway (só o que vier preenchido).
pub fn apply(output: &str, s: &Settings) -> Result<(), String> {
    validate(s)?;
    if !output.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) || output.is_empty() {
        return Err("Saída de vídeo inválida".into());
    }
    let mut cmd = vec!["output".to_string(), output.to_string()];
    if let Some(m) = &s.mode {
        cmd.extend(["mode".into(), format!("{m}Hz")]);
    }
    if let Some(x) = s.scale {
        cmd.extend(["scale".into(), format!("{x}")]);
    }
    if let Some(t) = &s.transform {
        cmd.extend(["transform".into(), t.clone()]);
    }
    if let Some(v) = s.vrr {
        cmd.extend(["adaptive_sync".into(), if v { "on" } else { "off" }.into()]);
    }
    if let Some(h) = s.hdr {
        cmd.extend(["hdr".into(), if h { "on" } else { "off" }.into()]);
    }
    let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
    swaymsg(&args).map(|_| ())
}

fn key(ident: &str) -> String {
    format!("display:{ident}")
}

/// Une a configuração salva com uma mudança nova (campos ausentes ficam como estavam).
fn merge(a: &Settings, b: &Settings) -> Settings {
    Settings { mode: b.mode.clone().or(a.mode.clone()), scale: b.scale.or(a.scale), transform: b.transform.clone().or(a.transform.clone()), vrr: b.vrr.or(a.vrr), hdr: b.hdr.or(a.hdr) }
}

pub fn saved(db: &Db, ident: &str) -> Option<Settings> {
    db.kv_get(&key(ident)).and_then(|j| serde_json::from_str(&j).ok())
}

pub fn save(db: &Db, ident: &str, s: &Settings) {
    let all = merge(&saved(db, ident).unwrap_or_default(), s);
    let _ = db.kv_set(&key(ident), &serde_json::to_string(&all).unwrap_or_default());
}

/// Ao iniciar: reaplica a configuração confirmada de cada tela conectada.
pub fn restore(db: &Db) {
    for o in outputs() {
        if let Some(s) = saved(db, &o.ident) {
            if apply(&o.name, &s).is_err() {
                tracing::warn!("tela {}: configuração salva recusada; mantendo a atual", o.label);
            }
        }
    }
}

pub fn margin(db: &Db) -> u32 {
    db.kv_get("display_margin").and_then(|m| m.parse().ok()).filter(|m| *m <= 10).unwrap_or(0)
}

pub fn set_margin(db: &Db, m: u32) -> Result<(), String> {
    if m > 10 {
        return Err("Margem inválida".into());
    }
    db.kv_set("display_margin", &m.to_string()).map_err(|e| e.to_string())
}

/// Mudança esperando confirmação.
#[derive(Debug, Clone)]
pub struct Pending {
    pub token: u64,
    pub output: String,
    pub ident: String,
    pub previous: Settings,
    pub next: Settings,
}

pub static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static NEXT_TOKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn confirm_secs() -> u64 {
    std::env::var("NEXUS_DISPLAY_CONFIRM_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(15)
}

/// Aplica uma mudança arriscada e deixa pendente. Devolve o token para confirmar.
pub fn try_set(output: &str, next: Settings) -> Result<u64, String> {
    validate(&next)?;
    let o = outputs().into_iter().find(|o| o.name == output).ok_or("Tela não encontrada")?;
    let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    // já havia outra pendente: volta para a anterior dela antes (não empilha)
    let previous = match p.take() {
        Some(old) if old.output == output => old.previous,
        _ => current(&o),
    };
    apply(output, &next)?;
    // contador pequeno: o token vai e volta pelo JSON da Shell (inteiros acima de 2^53 perdem precisão no JS)
    let token = NEXT_TOKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    *p = Some(Pending { token, output: output.into(), ident: o.ident, previous, next });
    Ok(token)
}

/// Confirma: grava. Devolve false se o token não é o pendente (já voltou sozinho).
pub fn confirm(db: &Db, token: u64) -> bool {
    let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    match p.take() {
        Some(x) if x.token == token => {
            save(db, &x.ident, &x.next);
            true
        }
        other => {
            *p = other;
            false
        }
    }
}

/// Desfaz a pendente com esse token (ou qualquer uma, se `token` for None). Devolve true se desfez algo.
pub fn revert(token: Option<u64>) -> bool {
    let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    match p.take() {
        Some(x) if token.is_none_or(|t| t == x.token) => {
            let _ = apply(&x.output, &x.previous);
            true
        }
        other => {
            *p = other;
            false
        }
    }
}

pub fn status(db: &Db) -> Value {
    if !available() {
        return json!({ "available": false, "outputs": [], "margin": margin(db) });
    }
    let pending = PENDING.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|p| p.token);
    let outs: Vec<Value> = outputs()
        .iter()
        .map(|o| {
            json!({
                "name": o.name,
                "label": o.label,
                "width": o.mode.map(|m| m.0), "height": o.mode.map(|m| m.1), "refresh": o.mode.map(|m| m.2),
                "modes": o.modes.iter().map(|(w, h, r)| json!({ "width": w, "height": h, "refresh": r })).collect::<Vec<_>>(),
                "scale": o.scale,
                "transform": o.transform,
                "vrr": o.vrr,
                "hdr": o.hdr,
            })
        })
        .collect();
    json!({ "available": true, "outputs": outs, "margin": margin(db), "pending": pending, "confirmSecs": confirm_secs() })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTS: &str = r#"[{"name":"HDMI-A-1","make":"Samsung Electric Company","model":"SAMSUNG","serial":"0x01000E00","active":true,"scale":1.0,"transform":"normal","adaptive_sync_status":"disabled","current_mode":{"width":1920,"height":1080,"refresh":60000},"modes":[{"width":1280,"height":720,"refresh":60000},{"width":1920,"height":1080,"refresh":60000},{"width":1920,"height":1080,"refresh":50000},{"width":3840,"height":2160,"refresh":30000},{"width":1920,"height":1080,"refresh":60000}]},{"name":"DP-1","active":false}]"#;

    #[test]
    fn parses_outputs() {
        let o = parse_outputs(OUTS);
        assert_eq!(o.len(), 1, "só as ativas");
        let h = &o[0];
        assert_eq!((h.name.as_str(), h.label.as_str()), ("HDMI-A-1", "Samsung Electric Company SAMSUNG"));
        assert_eq!(h.modes, vec![(3840, 2160, 30000), (1920, 1080, 60000), (1920, 1080, 50000), (1280, 720, 60000)], "maior primeiro, sem repetidos");
        assert_eq!((h.vrr, h.hdr), (Some(false), None));
        assert_eq!(current(h).mode.as_deref(), Some("1920x1080@60.000"));
    }

    #[test]
    fn validates() {
        assert!(valid_mode("1920x1080@60.000") && valid_mode("1280x720@59.94"));
        assert!(!valid_mode("1920x1080") && !valid_mode("9x9@60") && !valid_mode("1920x1080@60;reboot"));
        assert!(validate(&Settings { transform: Some("45".into()), ..Default::default() }).is_err());
        assert!(validate(&Settings { scale: Some(9.0), ..Default::default() }).is_err());
        assert!(apply("HDMI-A-1;exit", &Settings::default()).is_err());
    }

    #[test]
    fn merge_keeps_old_fields() {
        let a = Settings { mode: Some("1920x1080@60.000".into()), scale: Some(1.0), ..Default::default() };
        let b = Settings { scale: Some(1.5), ..Default::default() };
        assert_eq!(merge(&a, &b), Settings { mode: Some("1920x1080@60.000".into()), scale: Some(1.5), ..Default::default() });
    }
}
