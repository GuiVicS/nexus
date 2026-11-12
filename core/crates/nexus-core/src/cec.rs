//! HDMI-CEC (B5): ligar e desligar a TV junto com o Nexus, via `cec-client` (libcec). Sem adaptador CEC (o Raspberry Pi
//! tem na porta HDMI; PCs precisam do adaptador Pulse-Eight) não há o que fazer: a tela explica isso.
//! `NEXUS_CEC_CLIENT` troca o programa (testes). Preferências no banco: `cec_wake` (ligar a TV ao abrir o Nexus) e
//! `cec_sleep` (desligar a TV ao desligar ou suspender o aparelho). Ambas começam desligadas.
use crate::db::Db;
use crate::sysops::bin;
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

pub fn client() -> String {
    bin("NEXUS_CEC_CLIENT", "cec-client")
}

pub fn installed() -> bool {
    crate::programs::which(&client()).is_some()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Adapter {
    pub port: String,
    pub kind: String,
}

/// Lê a saída de `cec-client -l`.
pub fn parse_adapters(out: &str) -> Vec<Adapter> {
    let mut v: Vec<Adapter> = vec![];
    let mut cur: Option<Adapter> = None;
    for l in out.lines() {
        let l = l.trim();
        if l.starts_with("device:") {
            if let Some(a) = cur.take() {
                v.push(a);
            }
            cur = Some(Adapter { port: String::new(), kind: String::new() });
        } else if let Some(a) = cur.as_mut() {
            if let Some(p) = l.strip_prefix("com port:") {
                a.port = p.trim().to_string();
            } else if let Some(t) = l.strip_prefix("type:") {
                a.kind = t.trim().to_string();
            }
        }
    }
    if let Some(a) = cur {
        v.push(a);
    }
    v
}

pub fn adapters() -> Vec<Adapter> {
    let (ok, out, _) = crate::sysops::run(&client(), &["-l"], 15);
    if ok { parse_adapters(&out) } else { vec![] }
}

pub fn flag(db: &Db, key: &str) -> bool {
    db.kv_get(key).is_some_and(|v| v == "1")
}

pub fn status(db: &Db) -> Value {
    let inst = installed();
    let ads = if inst { adapters() } else { vec![] };
    json!({
        "installed": inst, "installCmd": "sudo apt install cec-utils",
        "adapters": ads.iter().map(|a| json!({ "port": a.port, "kind": a.kind })).collect::<Vec<_>>(),
        "wake": flag(db, "cec_wake"), "sleep": flag(db, "cec_sleep"),
    })
}

/// Manda comandos CEC (`on 0`, `standby 0`, `as`) ao cec-client em modo "só enviar".
pub fn send(cmds: &[&str]) -> Result<(), String> {
    if !installed() {
        return Err("O cec-client não está instalado (sudo apt install cec-utils)".into());
    }
    let mut child = Command::new("timeout")
        .args(["20", &client(), "-s", "-d", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Não consegui abrir o cec-client".to_string())?;
    if let Some(mut si) = child.stdin.take() {
        for c in cmds {
            let _ = writeln!(si, "{c}");
        }
    }
    let st = child.wait().map_err(|_| "O cec-client não respondeu".to_string())?;
    if st.success() { Ok(()) } else { Err("A TV não respondeu (confira o cabo HDMI e se o CEC está ligado na TV)".into()) }
}

pub fn tv(on: bool) -> Result<(), String> {
    if on { send(&["on 0", "as"]) } else { send(&["standby 0"]) }
}

/// Ao desligar ou suspender o aparelho: desliga a TV junto, se o usuário pediu. Erros são ignorados (não atrasa o desligamento).
pub fn on_power(db: &Db, action: &str) {
    if matches!(action, "off" | "suspend") && flag(db, "cec_sleep") && installed() {
        let _ = tv(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_adapter_list() {
        let out = "libCEC version: 6.0.2\n\nFound devices: 1\n\ndevice:              1\ncom port:            /dev/ttyACM0\nvendor id:           2548\nfirmware version:    3\ntype:                Pulse-Eight USB-CEC Adapter\n";
        let a = parse_adapters(out);
        assert_eq!(a, vec![Adapter { port: "/dev/ttyACM0".into(), kind: "Pulse-Eight USB-CEC Adapter".into() }]);
        assert!(parse_adapters("Found devices: NONE\n").is_empty());
    }
}
