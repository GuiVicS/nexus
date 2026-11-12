//! Brilho da tela (notebooks e telas com backlight em `/sys/class/backlight`). TV e monitor por HDMI não têm: nesse
//! caso a Central rápida não mostra o controle. Quem escreve é o logind (`SetBrightness` na sessão ativa), sem root;
//! se o logind recusar, tenta escrever direto no sysfs (regra do udev dá permissão ao grupo).
//! `NEXUS_BACKLIGHT_DIR` troca a pasta e `NEXUS_BUSCTL` o programa (testes).
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn dir() -> PathBuf {
    std::env::var("NEXUS_BACKLIGHT_DIR").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/sys/class/backlight"))
}

fn read_u64(p: &std::path::Path) -> Option<u64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

/// O backlight de maior resolução (o "intel_backlight" costuma ter mais passos que o "acpi_video0").
pub fn device() -> Option<(String, u64, u64)> {
    let mut best: Option<(String, u64, u64)> = None;
    for e in std::fs::read_dir(dir()).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let (Some(max), Some(cur)) = (read_u64(&e.path().join("max_brightness")), read_u64(&e.path().join("brightness"))) else { continue };
        if max > 0 && best.as_ref().map_or(true, |b| max > b.2) {
            best = Some((name, cur, max));
        }
    }
    best
}

pub fn percent(cur: u64, max: u64) -> u8 {
    ((cur * 100 + max / 2) / max.max(1)).min(100) as u8
}

/// Nunca deixa a tela preta: o mínimo é 5 %.
pub fn raw_value(pct: u8, max: u64) -> u64 {
    (max * pct.clamp(5, 100) as u64 / 100).max(1)
}

pub fn status() -> Value {
    match device() {
        Some((name, cur, max)) => json!({ "available": true, "name": name, "percent": percent(cur, max) }),
        None => json!({ "available": false, "percent": 0 }),
    }
}

pub fn set(pct: u8) -> Result<(), String> {
    let (name, _, max) = device().ok_or("Esta tela não tem controle de brilho")?;
    // o nome vem do sysfs, mas só letras, números, _ e - passam para a linha de comando
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Nome do dispositivo de brilho inválido".into());
    }
    let v = raw_value(pct, max);
    let bus = std::env::var("NEXUS_BUSCTL").unwrap_or_else(|_| "busctl".into());
    let ok = Command::new("timeout")
        .arg("5")
        .arg(bus)
        .args(["call", "org.freedesktop.login1", "/org/freedesktop/login1/session/auto", "org.freedesktop.login1.Session", "SetBrightness", "ssu", "backlight", &name, &v.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok || std::fs::write(dir().join(&name).join("brightness"), v.to_string()).is_ok() {
        Ok(())
    } else {
        Err("Não foi possível mudar o brilho".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversoes() {
        assert_eq!(percent(0, 255), 0);
        assert_eq!(percent(128, 255), 50);
        assert_eq!(percent(255, 255), 100);
        assert_eq!(raw_value(0, 1000), 50, "mínimo de 5 %");
        assert_eq!(raw_value(50, 1000), 500);
        assert_eq!(raw_value(100, 1000), 1000);
        assert_eq!(raw_value(100, 0), 1);
    }
}
