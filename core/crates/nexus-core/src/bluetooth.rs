//! Bluetooth via BlueZ (`bluetoothctl`, sempre com argv fixo e endereço validado): ligar/desligar, procurar, parear,
//! conectar, desconectar e esquecer qualquer aparelho (controle, fone, teclado…), com tipo e bateria.
//! `NEXUS_BLUETOOTHCTL` troca o binário e `NEXUS_BT_SYSFS` a pasta dos adaptadores (testes).
use serde_json::{json, Value};
use std::path::Path;

pub fn valid_mac(m: &str) -> bool {
    m.len() == 17 && m.split(':').count() == 6 && m.split(':').all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

fn ctl(args: &[&str], secs: u64) -> Option<String> {
    let bin = std::env::var("NEXUS_BLUETOOTHCTL").unwrap_or_else(|_| "bluetoothctl".into());
    let out = std::process::Command::new("timeout").arg(secs.to_string()).arg(bin).args(args).env("LC_ALL", "C").stdin(std::process::Stdio::null()).output().ok()?;
    Some(format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

/// Há adaptador? Olha o sysfs antes: sem adaptador o `bluetoothctl` fica esperando para sempre.
fn has_adapter() -> bool {
    let dir = std::env::var("NEXUS_BT_SYSFS").unwrap_or_else(|_| "/sys/class/bluetooth".into());
    Path::new(&dir).is_dir() && std::fs::read_dir(&dir).map(|mut r| r.next().is_some()).unwrap_or(false)
}

fn looks_like_pad(name: &str) -> bool {
    let n = name.to_lowercase();
    ["controller", "gamepad", "joystick", "xbox", "dualshock", "dualsense", "wireless controller", "8bitdo", "joy-con", "pro controller", "stadia", "gamesir", "ipega", "controle", "pad"].iter().any(|k| n.contains(k))
}

/// Tipo amigável pelo ícone do BlueZ (classe do aparelho) e, sem ícone, pelo nome.
pub fn kind_of(icon: &str, name: &str) -> &'static str {
    match icon {
        "input-gaming" => "controle",
        i if i.starts_with("audio-") || i == "multimedia-player" => "fone",
        "input-keyboard" => "teclado",
        "input-mouse" | "input-tablet" => "mouse",
        "phone" => "celular",
        _ => {
            let n = name.to_lowercase();
            if looks_like_pad(name) {
                "controle"
            } else if ["headset", "headphone", "buds", "fone", "speaker", "caixa", "soundbar", "airpods", "jbl"].iter().any(|k| n.contains(k)) {
                "fone"
            } else if n.contains("keyboard") || n.contains("teclado") {
                "teclado"
            } else if n.contains("mouse") {
                "mouse"
            } else {
                "outro"
            }
        }
    }
}

pub fn parse_devices(out: &str) -> Vec<(String, String)> {
    out.lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("Device ")?;
            let (mac, name) = rest.split_once(' ')?;
            valid_mac(mac).then(|| (mac.to_string(), name.trim().to_string()))
        })
        .collect()
}

#[derive(Debug, Default, PartialEq)]
pub struct Info {
    pub icon: String,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    pub battery: Option<u8>,
}

/// Lê a saída de `bluetoothctl info <mac>` ("Battery Percentage: 0x5a (90)").
pub fn parse_info(out: &str) -> Info {
    let mut i = Info::default();
    for l in out.lines().map(str::trim) {
        let Some((k, v)) = l.split_once(": ") else { continue };
        match k {
            "Icon" => i.icon = v.trim().into(),
            "Paired" => i.paired = v.trim() == "yes",
            "Connected" => i.connected = v.trim() == "yes",
            "Trusted" => i.trusted = v.trim() == "yes",
            "Battery Percentage" => i.battery = v.split_once('(').and_then(|(_, r)| r.trim_end_matches(')').parse().ok()).filter(|b: &u8| *b <= 100),
            _ => {}
        }
    }
    i
}

fn device(mac: &str, name: &str, info: &Info) -> Value {
    let kind = kind_of(&info.icon, name);
    json!({ "mac": mac, "name": name, "kind": kind, "gamepad": kind == "controle", "paired": info.paired, "connected": info.connected, "battery": info.battery })
}

/// Estado: há adaptador? ligado? aparelhos pareados com tipo, conexão e bateria (conectados primeiro).
pub fn status() -> Value {
    if !has_adapter() {
        return json!({ "available": false, "powered": false, "paired": [] });
    }
    let show = ctl(&["show"], 5).unwrap_or_default();
    let mut paired: Vec<Value> = parse_devices(&ctl(&["devices", "Paired"], 5).unwrap_or_default())
        .into_iter()
        .map(|(mac, name)| {
            let info = parse_info(&ctl(&["info", &mac], 5).unwrap_or_default());
            device(&mac, &name, &Info { paired: true, ..info })
        })
        .collect();
    paired.sort_by_key(|d| !d["connected"].as_bool().unwrap_or(false));
    json!({ "available": show.contains("Controller"), "powered": show.contains("Powered: yes"), "paired": paired })
}

fn friendly(out: &str, default: &str) -> String {
    let o = out.to_lowercase();
    if o.contains("blocked") || o.contains("rfkill") {
        "O Bluetooth está bloqueado no aparelho (chave ou modo avião)".into()
    } else if o.contains("not available") || o.contains("no default controller") {
        "Adaptador Bluetooth não encontrado".into()
    } else {
        default.into()
    }
}

pub fn set_power(on: bool) -> Result<(), String> {
    if !has_adapter() {
        return Err("Este aparelho não tem adaptador Bluetooth".into());
    }
    let out = ctl(&["power", if on { "on" } else { "off" }], 8).ok_or("bluetoothctl indisponível")?;
    if out.contains("succeeded") {
        Ok(())
    } else {
        Err(friendly(&out, if on { "Não foi possível ligar o Bluetooth" } else { "Não foi possível desligar o Bluetooth" }))
    }
}

/// Liga o Bluetooth, procura por `secs` segundos e devolve o que encontrou e ainda não está pareado (controles primeiro).
pub fn scan(secs: u64) -> Value {
    if !has_adapter() {
        return json!({ "devices": [] });
    }
    let _ = ctl(&["power", "on"], 5);
    let _ = ctl(&["--timeout", &secs.to_string(), "scan", "on"], secs + 3);
    let paired: Vec<String> = parse_devices(&ctl(&["devices", "Paired"], 5).unwrap_or_default()).into_iter().map(|d| d.0).collect();
    let mut found: Vec<Value> = parse_devices(&ctl(&["devices"], 5).unwrap_or_default())
        .into_iter()
        .filter(|(mac, n)| !n.is_empty() && !valid_mac(&n.replace('-', ":")) && !paired.contains(mac))
        .map(|(mac, name)| {
            let info = parse_info(&ctl(&["info", &mac], 5).unwrap_or_default());
            device(&mac, &name, &info)
        })
        .collect();
    let rank = |d: &Value| match d["kind"].as_str() {
        Some("controle") => 0,
        Some("fone") | Some("teclado") | Some("mouse") => 1,
        _ => 2,
    };
    found.sort_by_key(rank);
    json!({ "devices": found })
}

fn check(mac: &str) -> Result<(), String> {
    if valid_mac(mac) {
        Ok(())
    } else {
        Err("endereço inválido".into())
    }
}

/// Pareia, confia (para reconectar sozinho) e conecta.
pub fn pair(mac: &str) -> Result<(), String> {
    check(mac)?;
    let out = ctl(&["--timeout", "25", "pair", mac], 30).unwrap_or_default();
    if !(out.contains("Pairing successful") || out.contains("AlreadyExists")) {
        return Err(if out.contains("AuthenticationFailed") || out.contains("AuthenticationCanceled") {
            "O aparelho recusou o pareamento: coloque-o em modo de pareamento e tente de novo".into()
        } else {
            friendly(&out, "Não foi possível parear: coloque o aparelho em modo de pareamento e tente de novo")
        });
    }
    let _ = ctl(&["trust", mac], 5);
    connect(mac).map_err(|_| "Pareado, mas não conectou: ligue o aparelho ou aperte um botão dele".to_string())
}

pub fn connect(mac: &str) -> Result<(), String> {
    check(mac)?;
    let out = ctl(&["--timeout", "15", "connect", mac], 20).unwrap_or_default();
    if out.contains("Connection successful") {
        Ok(())
    } else {
        Err(friendly(&out, "Não conectou: confira se o aparelho está ligado e perto"))
    }
}

pub fn disconnect(mac: &str) -> Result<(), String> {
    check(mac)?;
    let out = ctl(&["disconnect", mac], 10).unwrap_or_default();
    if out.contains("Successful disconnected") || out.contains("not connected") {
        Ok(())
    } else {
        Err(friendly(&out, "Não foi possível desconectar"))
    }
}

pub fn remove(mac: &str) -> Result<(), String> {
    check(mac)?;
    ctl(&["remove", mac], 8).ok_or("bluetoothctl indisponível")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_and_device_parsing() {
        assert!(valid_mac("AA:bb:01:23:45:6F"));
        assert!(!valid_mac("AA:BB:01:23:45"));
        assert!(!valid_mac("AA:BB:01:23:45:6G"));
        assert!(!valid_mac("aa:bb:cc:dd:ee:ff; rm -rf"));
        let d = parse_devices("Device 01:02:03:04:05:06 Xbox Wireless Controller\nDevice zz Lixo\n[CHG] Controller x\nDevice 0A:0B:0C:0D:0E:0F 8BitDo Pro 2");
        assert_eq!(d, vec![("01:02:03:04:05:06".into(), "Xbox Wireless Controller".into()), ("0A:0B:0C:0D:0E:0F".into(), "8BitDo Pro 2".into())]);
    }

    #[test]
    fn info_kind_and_battery() {
        let i = parse_info("Device 01:02:03:04:05:06 (public)\n\tName: WH-1000XM4\n\tIcon: audio-headset\n\tPaired: yes\n\tTrusted: yes\n\tConnected: no\n\tBattery Percentage: 0x5a (90)\n");
        assert_eq!(i, Info { icon: "audio-headset".into(), paired: true, connected: false, trusted: true, battery: Some(90) });
        assert_eq!(kind_of(&i.icon, "WH-1000XM4"), "fone");
        assert_eq!(kind_of("input-gaming", "x"), "controle");
        assert_eq!(kind_of("", "DualSense Wireless Controller"), "controle");
        assert_eq!(kind_of("input-keyboard", "K380"), "teclado");
        assert_eq!(kind_of("", "Galaxy Buds2"), "fone");
        assert_eq!(kind_of("", "Impressora"), "outro");
        assert_eq!(parse_info("Battery Percentage: 0xff (255)").battery, None);
    }
}
