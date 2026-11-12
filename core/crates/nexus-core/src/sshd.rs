//! Acesso por SSH (B15 e passo a passo de Ajustes): estado do servidor (instalado? ligado?), usuário da sessão, IPs,
//! porta, impressão digital da chave do aparelho e liga/desliga pelo ajudante root (`sysops.rs`).
//! Programas trocáveis: `NEXUS_SYSTEMCTL`, `NEXUS_IP`, `NEXUS_SSHKEYGEN`, `NEXUS_HOSTNAME`; `NEXUS_SSHD_CONFIG`
//! (padrão /etc/ssh/sshd_config) e `NEXUS_SSH_HOSTKEY` (padrão /etc/ssh/ssh_host_ed25519_key.pub).
use crate::sysops::{bin, run, run_sys};
use serde_json::{json, Value};

pub const INSTALL_CMD: &str = "sudo apt install openssh-server";
pub const ENABLE_CMD: &str = "sudo systemctl enable --now ssh";

/// Unidade do servidor: `ssh` (Debian/Ubuntu/Raspberry Pi OS) ou `sshd`. `None` = servidor não instalado.
pub fn unit() -> Option<String> {
    let (_, out, _) = run(&bin("NEXUS_SYSTEMCTL", "systemctl"), &["list-unit-files", "ssh.service", "sshd.service", "--no-legend"], 5);
    parse_unit(&out)
}

pub fn parse_unit(out: &str) -> Option<String> {
    out.lines().filter_map(|l| l.split_whitespace().next()).filter_map(|u| u.strip_suffix(".service")).find(|u| matches!(*u, "ssh" | "sshd")).map(String::from)
}

fn is(verb: &str, unit: &str) -> bool {
    let (_, out, _) = run(&bin("NEXUS_SYSTEMCTL", "systemctl"), &[verb, unit], 5);
    matches!(out.trim(), "active" | "enabled" | "enabled-runtime")
}

/// Endereços IPv4 da rede local (sem loopback), do `ip -4 -o addr show scope global`.
pub fn parse_ips(out: &str) -> Vec<String> {
    out.lines().filter_map(|l| l.split_whitespace().skip_while(|w| *w != "inet").nth(1)).map(|a| a.split('/').next().unwrap_or(a).to_string()).filter(|a| !a.starts_with("127.") && !a.starts_with("169.254.")).collect()
}

/// Porta do `Port N` do sshd_config (a primeira, fora de comentários); 22 se não houver.
pub fn parse_port(cfg: &str) -> u16 {
    cfg.lines().filter_map(|l| {
        let l = l.trim();
        if l.starts_with('#') {
            return None;
        }
        let mut w = l.split_whitespace();
        (w.next()?.eq_ignore_ascii_case("port")).then(|| w.next()?.parse::<u16>().ok()).flatten()
    }).next().unwrap_or(22)
}

pub fn session_user() -> String {
    for k in ["USER", "LOGNAME"] {
        if let Ok(u) = std::env::var(k) {
            if !u.is_empty() {
                return u;
            }
        }
    }
    let (_, out, _) = run("id", &["-un"], 3);
    let u = out.trim().to_string();
    if u.is_empty() { "usuario".into() } else { u }
}

fn fingerprint() -> Option<String> {
    let key = bin("NEXUS_SSH_HOSTKEY", "/etc/ssh/ssh_host_ed25519_key.pub");
    let (ok, out, _) = run(&bin("NEXUS_SSHKEYGEN", "ssh-keygen"), &["-lf", &key], 5);
    parse_fingerprint(&out).filter(|_| ok)
}

/// "256 SHA256:abc… root@host (ED25519)" → "SHA256:abc… (ED25519)"
pub fn parse_fingerprint(out: &str) -> Option<String> {
    let l = out.lines().next()?;
    let mut w = l.split_whitespace();
    w.next()?;
    let fp = w.next().filter(|f| f.starts_with("SHA256:"))?;
    let kind = l.rsplit_once(' ').map(|(_, k)| k).filter(|k| k.starts_with('('))?;
    Some(format!("{fp} {kind}"))
}

pub fn status() -> Value {
    let u = unit();
    let (active, enabled) = match &u {
        Some(u) => {
            // com ssh.socket (Ubuntu moderno) o serviço só sobe na primeira conexão: a porta aberta já conta como ligado
            let sock = is("is-active", "ssh.socket");
            (is("is-active", u) || sock, is("is-enabled", u) || is("is-enabled", "ssh.socket"))
        }
        None => (false, false),
    };
    let (_, ipout, _) = run(&bin("NEXUS_IP", "ip"), &["-4", "-o", "addr", "show", "scope", "global"], 5);
    let ips = parse_ips(&ipout);
    let port = parse_port(&std::fs::read_to_string(bin("NEXUS_SSHD_CONFIG", "/etc/ssh/sshd_config")).unwrap_or_default());
    let (_, host, _) = run(&bin("NEXUS_HOSTNAME", "hostname"), &[], 3);
    json!({
        "installed": u.is_some(),
        "active": active,
        "enabled": enabled,
        "user": session_user(),
        "host": host.trim(),
        "ips": ips,
        "port": port,
        "fingerprint": if u.is_some() { fingerprint() } else { None },
        "installCmd": INSTALL_CMD,
        "enableCmd": ENABLE_CMD,
    })
}

pub fn set(on: bool) -> Result<(), String> {
    if unit().is_none() {
        return Err(format!("O servidor SSH não está instalado. No terminal do aparelho: {INSTALL_CMD}"));
    }
    run_sys(if on { "ssh-on" } else { "ssh-off" }).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses() {
        assert_eq!(parse_unit("ssh.service enabled enabled\n"), Some("ssh".into()));
        assert_eq!(parse_unit("sshd.service disabled enabled\n"), Some("sshd".into()));
        assert_eq!(parse_unit(""), None);
        let ip = "2: enp0s3    inet 192.168.0.42/24 brd 192.168.0.255 scope global dynamic enp0s3\n3: wlan0    inet 10.0.0.7/24 scope global wlan0\n";
        assert_eq!(parse_ips(ip), vec!["192.168.0.42", "10.0.0.7"]);
        assert_eq!(parse_port("# Port 2222\nPort 2200\nPort 22\n"), 2200);
        assert_eq!(parse_port("#Port 22\n"), 22);
        assert_eq!(parse_fingerprint("256 SHA256:AbC+/12 root@tv (ED25519)\n"), Some("SHA256:AbC+/12 (ED25519)".into()));
        assert_eq!(parse_fingerprint("não existe\n"), None);
    }
}
