//! Rede pelo NetworkManager (`nmcli`, sempre com argv fixo e valores validados). A Shell nunca roda comandos:
//! pede ao Core, que fala com o NetworkManager como o usuário da sessão (polkit: system/polkit/51-nexus-system.rules).
//! `NEXUS_NMCLI` troca o binário (testes).
use serde_json::{json, Value};
use std::net::Ipv4Addr;
use std::process::{Command, Stdio};
use std::time::Duration;

fn nmcli_bin() -> String {
    std::env::var("NEXUS_NMCLI").unwrap_or_else(|_| "nmcli".into())
}

/// Roda o nmcli com limite de tempo. Ok(stdout) ou Err(mensagem do nmcli).
fn nmcli(args: &[&str], secs: u64) -> Result<String, String> {
    let out = Command::new("timeout")
        .arg(secs.to_string())
        .arg(nmcli_bin())
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "O gerenciador de rede (NetworkManager) não está instalado".to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let e = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if out.status.code() == Some(124) { "O gerenciador de rede não respondeu".into() } else { friendly(&e) })
    }
}

/// Mensagens do nmcli em português simples.
pub fn friendly(e: &str) -> String {
    let l = e.to_lowercase();
    if l.contains("secrets were required") || l.contains("802-1x") || l.contains("psk") && l.contains("invalid") {
        "Senha incorreta".into()
    } else if l.contains("no network with ssid") {
        "Rede não encontrada. Chegue mais perto do roteador e tente de novo.".into()
    } else if l.contains("not authorized") || l.contains("insufficient privileges") {
        "Sem permissão para mudar a rede (falta a regra do polkit do Nexus)".into()
    } else if l.contains("timeout") || l.contains("timed out") {
        "A rede demorou demais para responder".into()
    } else if l.contains("networkmanager is not running") || l.contains("could not create nmclient") {
        "O gerenciador de rede não está rodando".into()
    } else if e.is_empty() {
        "Não foi possível concluir".into()
    } else {
        e.trim_start_matches("Error: ").to_string()
    }
}

/// Divide uma linha do modo `-t` do nmcli: campos separados por ':' com '\:' e '\\' escapados.
pub fn split_terse(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut it = line.chars();
    while let Some(c) = it.next() {
        match c {
            '\\' => {
                if let Some(n) = it.next() {
                    out.last_mut().unwrap().push(n);
                }
            }
            ':' => out.push(String::new()),
            _ => out.last_mut().unwrap().push(c),
        }
    }
    out
}

/// SSID: 1 a 32 bytes, sem caracteres de controle.
pub fn valid_ssid(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && !s.chars().any(|c| c.is_control())
}

/// Senha WPA: 8 a 63 caracteres imprimíveis (ou 64 hexadecimais).
pub fn valid_psk(p: &str) -> bool {
    (8..=63).contains(&p.chars().count()) && p.chars().all(|c| !c.is_control()) || p.len() == 64 && p.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn parse_wifi_list(out: &str, known: &[String]) -> Vec<Value> {
    let mut v: Vec<Value> = Vec::new();
    for l in out.lines() {
        let f = split_terse(l);
        if f.len() < 4 || f[1].is_empty() {
            continue; // redes ocultas aparecem sem nome
        }
        let ssid = f[1].clone();
        let signal: u64 = f[2].parse().unwrap_or(0);
        let active = f[0] == "*";
        let security = f[3].trim().to_string();
        // a mesma rede em vários pontos de acesso: fica a de sinal mais forte
        if let Some(e) = v.iter_mut().find(|x| x["ssid"] == ssid.as_str()) {
            if signal > e["signal"].as_u64().unwrap_or(0) {
                e["signal"] = json!(signal);
            }
            if active {
                e["active"] = json!(true);
            }
            continue;
        }
        v.push(json!({ "ssid": ssid, "signal": signal, "secure": !security.is_empty() && security != "--", "security": security, "active": active, "known": known.contains(&ssid) }));
    }
    v.sort_by(|a, b| b["active"].as_bool().cmp(&a["active"].as_bool()).then(b["signal"].as_u64().cmp(&a["signal"].as_u64())));
    v
}

pub fn parse_devices(out: &str) -> Vec<Value> {
    out.lines()
        .filter_map(|l| {
            let f = split_terse(l);
            if f.len() < 4 || !matches!(f[1].as_str(), "ethernet" | "wifi") {
                return None;
            }
            Some(json!({ "device": f[0], "type": f[1], "state": f[2], "connection": if f[3] == "--" { "" } else { &f[3] } }))
        })
        .collect()
}

/// `nmcli -t -f IP4.ADDRESS,IP4.GATEWAY,IP4.DNS,GENERAL.HWADDR device show <dev>`
pub fn parse_ip(out: &str) -> Value {
    let (mut addr, mut gw, mut dns, mut mac) = (String::new(), String::new(), Vec::<String>::new(), String::new());
    for l in out.lines() {
        let Some((k, val)) = l.split_once(':') else { continue };
        let val = val.replace("\\:", ":");
        if k.starts_with("IP4.ADDRESS") && addr.is_empty() {
            addr = val;
        } else if k == "IP4.GATEWAY" && val != "--" {
            gw = val;
        } else if k.starts_with("IP4.DNS") {
            dns.push(val);
        } else if k == "GENERAL.HWADDR" {
            mac = val;
        }
    }
    json!({ "address": addr, "gateway": gw, "dns": dns, "mac": mac })
}

fn known_wifi() -> Vec<String> {
    nmcli(&["-t", "-f", "NAME,TYPE", "connection", "show"], 8)
        .unwrap_or_default()
        .lines()
        .map(split_terse)
        .filter(|f| f.len() >= 2 && f[1] == "802-11-wireless")
        .map(|f| f[0].clone())
        .collect()
}

/// Estado geral: conectividade, interfaces, IP/DNS da conexão principal, Wi-Fi ligado.
pub fn status() -> Value {
    let conn = match nmcli(&["-t", "-f", "CONNECTIVITY", "general"], 6) {
        Ok(c) => c.trim().to_string(),
        Err(e) => return json!({ "available": false, "error": e }),
    };
    let devices = parse_devices(&nmcli(&["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"], 6).unwrap_or_default());
    let wifi_on = nmcli(&["-t", "-f", "WIFI", "radio"], 6).map(|w| w.trim() == "enabled").unwrap_or(false);
    let main = devices.iter().find(|d| d["state"] == "connected").cloned();
    let ip = main.as_ref().and_then(|d| d["device"].as_str()).and_then(|dev| nmcli(&["-t", "-f", "IP4.ADDRESS,IP4.GATEWAY,IP4.DNS,GENERAL.HWADDR", "device", "show", dev], 6).ok()).map(|o| parse_ip(&o)).unwrap_or(Value::Null);
    let method = main
        .as_ref()
        .and_then(|d| d["connection"].as_str().filter(|c| !c.is_empty()))
        .and_then(|c| nmcli(&["-t", "-f", "ipv4.method", "connection", "show", c], 6).ok())
        .and_then(|o| o.lines().next().and_then(|l| l.split_once(':')).map(|(_, m)| m.to_string()))
        .unwrap_or_default();
    let signal = main.as_ref().filter(|d| d["type"] == "wifi").and_then(|_| {
        nmcli(&["-t", "-f", "IN-USE,SIGNAL", "device", "wifi", "list", "--rescan", "no"], 6).ok()?.lines().map(split_terse).find(|f| f.first().map(String::as_str) == Some("*")).and_then(|f| f.get(1)?.parse::<u64>().ok())
    });
    json!({
        "available": true,
        "connectivity": conn,
        "online": conn == "full",
        "wifiEnabled": wifi_on,
        "hasWifi": devices.iter().any(|d| d["type"] == "wifi"),
        "devices": devices,
        "main": main,
        "ip": ip,
        "ipv4Method": method,
        "signal": signal,
    })
}

pub fn wifi_list(rescan: bool) -> Result<Value, String> {
    let out = nmcli(&["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", if rescan { "yes" } else { "auto" }], 20)?;
    Ok(json!({ "networks": parse_wifi_list(&out, &known_wifi()) }))
}

pub fn wifi_connect(ssid: &str, password: Option<&str>, hidden: bool) -> Result<(), String> {
    if !valid_ssid(ssid) {
        return Err("Nome de rede inválido".into());
    }
    let mut args: Vec<&str> = vec!["device", "wifi", "connect", ssid];
    if let Some(p) = password.filter(|p| !p.is_empty()) {
        if !valid_psk(p) {
            return Err("A senha do Wi-Fi tem de 8 a 63 caracteres".into());
        }
        args.extend(["password", p]);
    }
    if hidden {
        args.extend(["hidden", "yes"]);
    }
    nmcli(&args, 45).map(|_| ())
}

pub fn forget(name: &str) -> Result<(), String> {
    if !valid_ssid(name) {
        return Err("Nome de rede inválido".into());
    }
    nmcli(&["connection", "delete", "id", name], 15).map(|_| ())
}

pub fn set_wifi(on: bool) -> Result<(), String> {
    nmcli(&["radio", "wifi", if on { "on" } else { "off" }], 10).map(|_| ())
}

/// Teste de conexão: o NetworkManager confere a internet e o Core mede quanto demora para abrir uma conexão.
pub fn test() -> Value {
    let conn = nmcli(&["-t", "networking", "connectivity", "check"], 15).map(|c| c.trim().to_string()).unwrap_or_else(|_| "unknown".into());
    let t = std::time::Instant::now();
    let dns_ok = std::net::ToSocketAddrs::to_socket_addrs(&("deb.debian.org", 443)).map(|mut a| a.next().is_some()).unwrap_or(false);
    let reach = ["1.1.1.1:443", "8.8.8.8:443"].iter().find_map(|a| {
        let t0 = std::time::Instant::now();
        std::net::TcpStream::connect_timeout(&a.parse().ok()?, Duration::from_secs(3)).ok().map(|_| t0.elapsed().as_millis() as u64)
    });
    json!({
        "connectivity": conn,
        "online": conn == "full",
        "dns": dns_ok,
        "latencyMs": reach,
        "elapsedMs": t.elapsed().as_millis() as u64,
    })
}

/// "1.2.3.4/24" válido?
pub fn valid_cidr(s: &str) -> bool {
    let Some((ip, p)) = s.split_once('/') else { return false };
    ip.parse::<Ipv4Addr>().is_ok() && p.parse::<u8>().is_ok_and(|p| (1..=32).contains(&p))
}

/// IP fixo ou automático (DHCP) na conexão `con`.
pub fn set_ipv4(con: &str, manual: Option<(&str, &str, &[String])>) -> Result<(), String> {
    if con.is_empty() || con.len() > 64 || con.chars().any(|c| c.is_control()) {
        return Err("Conexão inválida".into());
    }
    let dns_joined;
    let args: Vec<&str> = match manual {
        Some((addr, gw, dns)) => {
            if !valid_cidr(addr) {
                return Err("Endereço inválido (ex.: 192.168.0.50/24)".into());
            }
            if gw.parse::<Ipv4Addr>().is_err() {
                return Err("Gateway inválido (ex.: 192.168.0.1)".into());
            }
            if dns.is_empty() || dns.len() > 3 || dns.iter().any(|d| d.parse::<Ipv4Addr>().is_err()) {
                return Err("DNS inválido (ex.: 1.1.1.1)".into());
            }
            dns_joined = dns.join(",");
            vec!["connection", "modify", "id", con, "ipv4.method", "manual", "ipv4.addresses", addr, "ipv4.gateway", gw, "ipv4.dns", &dns_joined]
        }
        None => vec!["connection", "modify", "id", con, "ipv4.method", "auto", "ipv4.addresses", "", "ipv4.gateway", "", "ipv4.dns", ""],
    };
    nmcli(&args, 15)?;
    nmcli(&["connection", "up", "id", con], 45).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terse_and_wifi_list() {
        assert_eq!(split_terse(r"a\:b:c\\d:"), vec!["a:b", r"c\d", ""]);
        let out = "*:Casa:70:WPA2\n:Casa:85:WPA2\n:Vizinho\\: 5G:40:WPA1 WPA2\n::90:WPA2\n:Café:55:\n";
        let l = parse_wifi_list(out, &["Café".to_string()]);
        assert_eq!(l.len(), 3, "rede oculta (sem nome) fica de fora e a repetida vira uma");
        assert_eq!((l[0]["ssid"].as_str(), l[0]["signal"].as_u64(), l[0]["active"].as_bool()), (Some("Casa"), Some(85), Some(true)));
        let cafe = l.iter().find(|n| n["ssid"] == "Café").unwrap();
        assert_eq!((cafe["secure"].as_bool(), cafe["known"].as_bool()), (Some(false), Some(true)));
        assert!(l.iter().any(|n| n["ssid"] == "Vizinho: 5G"));
    }

    #[test]
    fn devices_and_ip() {
        let d = parse_devices("enp0s3:ethernet:connected:Conexão cabeada 1\nlo:loopback:connected (externally):lo\nwlp2s0:wifi:disconnected:--\n");
        assert_eq!(d.len(), 2);
        assert_eq!(d[0]["connection"], "Conexão cabeada 1");
        assert_eq!(d[1]["connection"], "");
        let ip = parse_ip("IP4.ADDRESS[1]:10.0.2.15/24\nIP4.GATEWAY:10.0.2.2\nIP4.DNS[1]:10.0.2.3\nIP4.DNS[2]:1.1.1.1\nGENERAL.HWADDR:08\\:00\\:27\\:AA\\:BB\\:CC\n");
        assert_eq!(ip["address"], "10.0.2.15/24");
        assert_eq!(ip["gateway"], "10.0.2.2");
        assert_eq!(ip["dns"], json!(["10.0.2.3", "1.1.1.1"]));
        assert_eq!(ip["mac"], "08:00:27:AA:BB:CC");
    }

    #[test]
    fn validation() {
        assert!(valid_ssid("Casa 5G") && !valid_ssid("") && !valid_ssid(&"x".repeat(33)) && !valid_ssid("a\nb"));
        assert!(valid_psk("12345678") && !valid_psk("1234567") && valid_psk(&"a".repeat(64).replace('a', "f")) && !valid_psk(&"z".repeat(64)));
        assert!(valid_cidr("192.168.0.50/24") && !valid_cidr("192.168.0.50") && !valid_cidr("300.1.1.1/24") && !valid_cidr("1.1.1.1/33"));
        assert_eq!(friendly("Error: Connection activation failed: Secrets were required, but not provided."), "Senha incorreta");
        assert!(friendly("Error: No network with SSID 'x' found.").starts_with("Rede não encontrada"));
    }

    #[test]
    fn bad_input_never_reaches_nmcli() {
        std::env::set_var("NEXUS_NMCLI", "/nonexistent/nmcli");
        assert_eq!(wifi_connect("ok", Some("curta"), false).unwrap_err(), "A senha do Wi-Fi tem de 8 a 63 caracteres");
        assert!(set_ipv4("Casa", Some(("1.2.3.4", "1.2.3.1", &["1.1.1.1".to_string()]))).unwrap_err().contains("Endereço"));
        assert!(set_ipv4("Casa", Some(("1.2.3.4/24", "x", &["1.1.1.1".to_string()]))).unwrap_err().contains("Gateway"));
        assert!(forget("").is_err());
    }
}
