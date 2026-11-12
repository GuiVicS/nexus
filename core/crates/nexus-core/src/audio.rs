//! Áudio pelo PipeWire (`wpctl`, argv fixo, ids numéricos validados): saídas (HDMI, analógica, Bluetooth, USB),
//! volume geral e mudo, saída padrão (lembrada por perfil), teste de som (`pw-play`) e microfone.
//! `NEXUS_WPCTL` e `NEXUS_PWPLAY` trocam os binários (testes).
use crate::db::Db;
use serde_json::{json, Value};
use std::process::{Command, Stdio};

/// Serializa as mudanças (cada RPC roda numa thread própria).
pub static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn bin(env: &str, def: &str) -> String {
    std::env::var(env).unwrap_or_else(|_| def.into())
}

fn wpctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("timeout").arg("5").arg(bin("NEXUS_WPCTL", "wpctl")).args(args).env("LC_ALL", "C").stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let e = String::from_utf8_lossy(&out.stderr).to_lowercase();
        Err(if e.contains("could not connect") || (e.contains("not found") && e.contains("pipewire")) {
            "O serviço de som (PipeWire) não está rodando".into()
        } else {
            "Não foi possível mudar o som".into()
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: u32,
    pub name: String,
    pub default: bool,
    pub volume: u8,
    pub muted: bool,
}

/// Lê as seções "Sinks" e "Sources" do bloco "Audio" de `wpctl status`.
pub fn parse_status(out: &str) -> (Vec<Node>, Vec<Node>) {
    let (mut sinks, mut sources) = (vec![], vec![]);
    let mut in_audio = false;
    let mut sec = "";
    for line in out.lines() {
        if !line.starts_with(' ') && !line.starts_with('│') && !line.starts_with('├') && !line.starts_with('└') && !line.trim().is_empty() {
            in_audio = line.trim() == "Audio";
            continue;
        }
        if !in_audio {
            continue;
        }
        let t = line.trim_start_matches(|c: char| c == ' ' || c == '│' || c == '├' || c == '└' || c == '─');
        if let Some(h) = t.strip_suffix(':') {
            sec = match h.trim() {
                "Sinks" => "sink",
                "Sources" => "source",
                _ => "",
            };
            continue;
        }
        if sec.is_empty() {
            continue;
        }
        let (default, t) = match t.strip_prefix('*') {
            Some(r) => (true, r.trim_start()),
            None => (false, t),
        };
        let Some((id, rest)) = t.split_once(". ") else { continue };
        let Ok(id) = id.trim().parse::<u32>() else { continue };
        let (name, vol) = match rest.split_once("[vol:") {
            Some((n, v)) => (n.trim(), v.trim_end_matches(']').trim()),
            None => (rest.trim(), ""),
        };
        let muted = vol.contains("MUTED");
        let volume = vol.split_whitespace().next().and_then(|x| x.parse::<f32>().ok()).map(|f| (f * 100.0).round().clamp(0.0, 150.0) as u8).unwrap_or(0);
        let n = Node { id, name: name.to_string(), default, volume, muted };
        if sec == "sink" {
            sinks.push(n);
        } else {
            sources.push(n);
        }
    }
    (sinks, sources)
}

/// Propriedades de `wpctl inspect` que interessam: node.name (estável entre boots) e o tipo da saída.
pub fn parse_inspect(out: &str) -> (String, &'static str) {
    let get = |k: &str| out.lines().find_map(|l| l.trim().trim_start_matches("* ").strip_prefix(k).and_then(|r| r.trim().strip_prefix('=')).map(|v| v.trim().trim_matches('"').to_string())).unwrap_or_default();
    let node = get("node.name");
    let api = get("device.api");
    let bus = get("device.bus");
    let desc = format!("{} {}", get("device.profile.description"), get("node.description")).to_lowercase();
    let kind = if api == "bluez5" || node.starts_with("bluez") {
        "bluetooth"
    } else if node.contains("hdmi") || desc.contains("hdmi") || desc.contains("displayport") {
        "hdmi"
    } else if bus == "usb" || node.contains("usb") {
        "usb"
    } else {
        "analogica"
    };
    (node, kind)
}

fn node_json(n: &Node, with_kind: bool) -> Value {
    let (node, kind) = if with_kind { parse_inspect(&wpctl(&["inspect", &n.id.to_string()]).unwrap_or_default()) } else { (String::new(), "analogica") };
    json!({ "id": n.id, "name": n.name, "node": node, "kind": kind, "default": n.default, "volume": n.volume, "muted": n.muted })
}

pub fn status() -> Value {
    let out = match wpctl(&["status"]) {
        Ok(o) => o,
        Err(e) => return json!({ "available": false, "error": e, "sinks": [], "sources": [] }),
    };
    let (sinks, sources) = parse_status(&out);
    let def = sinks.iter().find(|s| s.default).or(sinks.first());
    let mic = sources.iter().find(|s| s.default).or(sources.first());
    json!({
        "available": true,
        "volume": def.map(|d| d.volume).unwrap_or(0),
        "muted": def.map(|d| d.muted).unwrap_or(false),
        "sinks": sinks.iter().map(|n| node_json(n, true)).collect::<Vec<_>>(),
        "sources": sources.iter().map(|n| node_json(n, false)).collect::<Vec<_>>(),
        "mic": mic.map(|m| json!({ "id": m.id, "volume": m.volume, "muted": m.muted })),
    })
}

/// Alvo: id numérico de um nó, ou a saída/entrada padrão.
fn target(id: Option<u32>, source: bool) -> String {
    match id {
        Some(i) => i.to_string(),
        None if source => "@DEFAULT_AUDIO_SOURCE@".into(),
        None => "@DEFAULT_AUDIO_SINK@".into(),
    }
}

pub fn set_volume(id: Option<u32>, source: bool, pct: u8) -> Result<(), String> {
    let v = format!("{:.2}", pct.min(100) as f32 / 100.0);
    let t = target(id, source);
    wpctl(&["set-volume", &t, &v])?;
    // mexer no volume tira o mudo (como nos consoles)
    if pct > 0 {
        wpctl(&["set-mute", &t, "0"])?;
    }
    Ok(())
}

pub fn set_mute(id: Option<u32>, source: bool, on: bool) -> Result<(), String> {
    wpctl(&["set-mute", &target(id, source), if on { "1" } else { "0" }]).map(|_| ())
}

pub fn set_default(id: u32) -> Result<(), String> {
    wpctl(&["set-default", &id.to_string()]).map(|_| ())
}

/// Lembra a saída escolhida para o perfil (pelo node.name, que não muda entre boots como o id).
pub fn remember(db: &Db, profile: &str, id: u32) {
    let (node, _) = parse_inspect(&wpctl(&["inspect", &id.to_string()]).unwrap_or_default());
    if !node.is_empty() {
        let _ = db.kv_set(&format!("audio_out:{profile}"), &node);
    }
}

/// Ao entrar no perfil: volta para a saída que ele usava, se ela estiver conectada.
pub fn apply_profile(db: &Db, profile: &str) {
    let Some(want) = db.kv_get(&format!("audio_out:{profile}")) else { return };
    let (sinks, _) = parse_status(&wpctl(&["status"]).unwrap_or_default());
    for s in sinks {
        if parse_inspect(&wpctl(&["inspect", &s.id.to_string()]).unwrap_or_default()).0 == want {
            if !s.default {
                let _ = set_default(s.id);
            }
            return;
        }
    }
}

/// Dois bipes curtos (WAV 48 kHz mono) gerados uma vez no cache.
fn test_wav() -> std::path::PathBuf {
    let p = std::env::temp_dir().join("nexus-teste-som.wav");
    if p.exists() {
        return p;
    }
    let rate = 48_000u32;
    let mut pcm: Vec<i16> = vec![];
    for (f, ms) in [(660.0f32, 220u32), (0.0, 80), (880.0, 300)] {
        let n = rate * ms / 1000;
        for i in 0..n {
            let fade = ((i.min(n - i)) as f32 / 480.0).min(1.0);
            let s = if f > 0.0 { (2.0 * std::f32::consts::PI * f * i as f32 / rate as f32).sin() * 0.35 * fade } else { 0.0 };
            pcm.push((s * i16::MAX as f32) as i16);
        }
    }
    let data_len = (pcm.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        w.extend_from_slice(&s.to_le_bytes());
    }
    let _ = std::fs::write(&p, w);
    p
}

/// Toca o teste na saída indicada (ou na padrão).
pub fn test(id: Option<u32>) -> Result<(), String> {
    let wav = test_wav();
    let mut c = Command::new("timeout");
    c.arg("5").arg(bin("NEXUS_PWPLAY", "pw-play"));
    if let Some(i) = id {
        c.arg(format!("--target={i}"));
    }
    let ok = c.arg(&wav).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err("Não foi possível tocar o som de teste".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "PipeWire 'pipewire-0' [1.6.2, u@h, cookie:1]\n └─ Clients:\n        33. WirePlumber                         [1.6.2, u@h, pid:2677]\n\nAudio\n ├─ Devices:\n │      48. Áudio interno                      [alsa]\n │  \n ├─ Sinks:\n │      54. Monitor LG (HDMI)                   [vol: 1.00]\n │  *   55. Áudio interno Estéreo analógico  [vol: 0.40 MUTED]\n │  \n ├─ Sources:\n │  *   56. Áudio interno Estéreo analógico  [vol: 0.85]\n │  \n ├─ Filters:\n │  \n └─ Streams:\n        70. Firefox\n\nVideo\n ├─ Devices:\n ├─ Sinks:\n │      99. Não é áudio  [vol: 1.00]\n";

    #[test]
    fn parses_wpctl_status() {
        let (sinks, sources) = parse_status(STATUS);
        assert_eq!(sinks.len(), 2, "só os sinks do bloco Audio: {sinks:?}");
        assert_eq!(sinks[1], Node { id: 55, name: "Áudio interno Estéreo analógico".into(), default: true, volume: 40, muted: true });
        assert_eq!((sinks[0].id, sinks[0].default, sinks[0].volume), (54, false, 100));
        assert_eq!(sources, vec![Node { id: 56, name: "Áudio interno Estéreo analógico".into(), default: true, volume: 85, muted: false }]);
    }

    #[test]
    fn output_kind() {
        assert_eq!(parse_inspect("  * node.name = \"alsa_output.pci-0000_01_00.1.hdmi-stereo\"\n    device.api = \"alsa\"\n"), ("alsa_output.pci-0000_01_00.1.hdmi-stereo".into(), "hdmi"));
        assert_eq!(parse_inspect("    node.name = \"bluez_output.AA_BB.1\"\n    device.api = \"bluez5\"\n").1, "bluetooth");
        assert_eq!(parse_inspect("    node.name = \"alsa_output.usb-Logitech.analog-stereo\"\n    device.bus = \"usb\"\n").1, "usb");
        assert_eq!(parse_inspect("    node.name = \"alsa_output.pci.analog-stereo\"\n    device.profile.description = \"Estéreo analógico\"\n").1, "analogica");
    }

    #[test]
    fn output_is_remembered_per_profile() {
        let dir = std::env::temp_dir().join(format!("nexus-audio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("NEXUS_WPCTL", concat!(env!("CARGO_MANIFEST_DIR"), "/../../../tests/e2e/fake/wpctl"));
        std::env::set_var("NEXUS_FAKE_AUDIO", &dir);
        let db = Db::memory().unwrap();
        let def = || std::fs::read_to_string(dir.join("default")).unwrap_or_default().trim().to_string();
        set_default(54).unwrap();
        remember(&db, "ana", 54); // Ana usa a TV pelo HDMI
        set_default(55).unwrap(); // outro perfil voltou para a analógica
        apply_profile(&db, "bia"); // sem preferência: não mexe
        assert_eq!(def(), "55");
        apply_profile(&db, "ana");
        assert_eq!(def(), "54", "Ana volta para o HDMI");
        let st = status();
        assert_eq!(st["sinks"][0]["kind"], "hdmi");
        assert_eq!(st["sinks"][0]["default"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sound_is_valid_wav() {
        let b = std::fs::read(test_wav()).unwrap();
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(b[40..44].try_into().unwrap()) as usize, b.len() - 44);
    }
}
